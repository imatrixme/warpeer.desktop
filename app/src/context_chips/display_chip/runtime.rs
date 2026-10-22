use warp_completer::completer::GeneratorContext as _;
use warpui::elements::{Dismiss, DropShadow, Fill as UiFill};
use warpui::ui_components::segmented_control::{
    LabelConfig, RenderableOptionConfig, SegmentedControl, SegmentedControlEvent,
};

use super::runtime_session_kill_dialog::{RuntimeSessionKillDialog, RuntimeSessionKillDialogEvent};
use super::*;
use crate::context_chips::runtime_sessions::{
    RuntimeKind, RuntimeSessionFetcher, RuntimeSessionFetcherEvent, RuntimeSessionFilter,
    RuntimeSessionItem, filter_runtime_sessions, runtime_filters_for,
};
use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;

const RUNTIME_MENU_WIDTH: f32 = 360.;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeSessionsChip {
    menu_open: bool,
    menu: ViewHandle<DisplayChipMenu>,
    filter_control: ViewHandle<SegmentedControl<RuntimeSessionFilter>>,
    fetcher: ModelHandle<RuntimeSessionFetcher>,
    kill_dialog: ViewHandle<RuntimeSessionKillDialog>,
    sessions: Vec<RuntimeSessionItem>,
    selected_filter: RuntimeSessionFilter,
    kill_dialog_open: bool,
}

impl RuntimeSessionsChip {
    pub(super) fn new(config: &DisplayChipConfig, ctx: &mut ViewContext<DisplayChip>) -> Self {
        let fetcher =
            ctx.add_model(|ctx| RuntimeSessionFetcher::new(config.session_context.clone(), ctx));
        let filter_control = ctx.add_typed_action_view(|ctx| {
            SegmentedControl::new(
                vec![
                    RuntimeSessionFilter::All,
                    RuntimeSessionFilter::Tmux,
                    RuntimeSessionFilter::Herdr,
                ],
                |filter, is_selected, app| {
                    let appearance = Appearance::as_ref(app);
                    let theme = appearance.theme();
                    let text_color = theme.main_text_color(theme.surface_2()).into_solid();
                    Some(RenderableOptionConfig {
                        icon_path: "",
                        icon_color: text_color,
                        label: Some(LabelConfig {
                            label: filter.label().into(),
                            width_override: Some(50.),
                            color: text_color,
                        }),
                        tooltip: None,
                        background: if is_selected {
                            UiFill::Solid(theme.surface_3().into())
                        } else {
                            UiFill::None
                        },
                    })
                },
                RuntimeSessionFilter::All,
                runtime_filter_styles(ctx),
            )
        });
        let filter_control_for_menu = filter_control.clone();
        let fetcher_for_menu = fetcher.clone();
        let menu = ctx.add_typed_action_view(move |ctx| {
            DisplayChipMenu::new(
                Vec::<RuntimeSessionItem>::new(),
                None,
                ChipMenuType::Branches,
                ctx,
            )
            .with_search_placeholder("Search sessions...", ctx)
            .with_after_search_renderer(move |app| {
                if fetcher_for_menu.as_ref(app).available_runtimes().len() <= 1 {
                    return Empty::new().finish();
                }
                Container::new(ChildView::new(&filter_control_for_menu).finish())
                    .with_horizontal_padding(8.)
                    .with_vertical_padding(8.)
                    .finish()
            })
            .embedded()
        });
        let kill_dialog = ctx.add_typed_action_view(RuntimeSessionKillDialog::new);

        ctx.subscribe_to_model(&fetcher, |display, fetcher, event, ctx| {
            match event {
                RuntimeSessionFetcherEvent::SessionsUpdated => {
                    let (available_runtimes, sessions) = fetcher.read(ctx, |fetcher, _| {
                        (
                            fetcher.available_runtimes().to_vec(),
                            fetcher.cached_sessions().to_vec(),
                        )
                    });
                    if let DisplayChipKind::RuntimeSessions(runtime) =
                        &mut display.display_chip_kind
                    {
                        runtime.update_sessions(available_runtimes, sessions, ctx);
                    }
                }
            }
            ctx.notify();
        });

        ctx.subscribe_to_view(&filter_control, |display, _, event, ctx| {
            let SegmentedControlEvent::OptionSelected(filter) = event;
            if let DisplayChipKind::RuntimeSessions(runtime) = &mut display.display_chip_kind {
                runtime.set_filter(*filter, ctx);
            }
        });

        ctx.subscribe_to_view(&kill_dialog, |display, _, event, ctx| match event {
            RuntimeSessionKillDialogEvent::Confirm(session) => {
                display.kill_runtime_session(session.clone(), ctx)
            }
            RuntimeSessionKillDialogEvent::Cancel => {
                if let DisplayChipKind::RuntimeSessions(runtime) = &mut display.display_chip_kind {
                    runtime.close_kill_dialog(ctx);
                }
            }
        });

        ctx.subscribe_to_view(&menu, |display, _, event, ctx| match event {
            PromptDisplayMenuEvent::MenuAction(event) => {
                let Some(session) = event
                    .action_item
                    .as_any()
                    .downcast_ref::<RuntimeSessionItem>()
                else {
                    log::warn!("Runtime session menu action contained an unexpected item");
                    return;
                };
                let command = match session.runtime() {
                    RuntimeKind::Tmux => PromptChipShellCommand::AttachTmuxSession {
                        session_name: session.session_name().to_string(),
                    },
                    RuntimeKind::Herdr => PromptChipShellCommand::AttachHerdrSession {
                        session_name: session.session_name().to_string(),
                    },
                };
                ctx.emit(PromptDisplayChipEvent::TryExecuteCommand(command));
                display.close_runtime_session_menu(ctx);
            }
            PromptDisplayMenuEvent::MenuSecondaryAction(event) => {
                let Some(session) = event
                    .action_item
                    .as_any()
                    .downcast_ref::<RuntimeSessionItem>()
                else {
                    log::warn!("Runtime session secondary action contained an unexpected item");
                    return;
                };
                if let DisplayChipKind::RuntimeSessions(runtime) = &mut display.display_chip_kind {
                    runtime.open_kill_dialog(session.clone(), ctx);
                }
            }
            PromptDisplayMenuEvent::CloseMenu => {
                display.close_runtime_session_menu(ctx);
                ctx.emit(PromptDisplayChipEvent::ToggleMenu { open: false });
            }
        });

        Self {
            menu_open: false,
            menu,
            filter_control,
            fetcher,
            kill_dialog,
            sessions: Vec::new(),
            selected_filter: RuntimeSessionFilter::All,
            kill_dialog_open: false,
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.menu_open
    }

    pub(super) fn close(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = false;
        self.kill_dialog_open = false;
        self.kill_dialog
            .update(ctx, |dialog, _| dialog.clear_session());
        self.menu.update(ctx, |menu, _| menu.reset_selected_index());
        ctx.notify();
    }

    pub(super) fn focus_if_open(&self, ctx: &mut ViewContext<DisplayChip>) -> bool {
        if self.menu_open {
            ctx.focus(&self.menu);
            true
        } else {
            false
        }
    }

    pub(super) fn update_session_context(
        &self,
        session_context: Option<SessionContext>,
        ctx: &mut ViewContext<DisplayChip>,
    ) {
        self.fetcher.update(ctx, |fetcher, ctx| {
            fetcher.update_session_context(session_context, ctx);
        });
    }

    pub(super) fn toggle(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = !self.menu_open;
        if self.menu_open {
            self.fetcher
                .update(ctx, |fetcher, ctx| fetcher.refetch(ctx));
            ctx.focus(&self.menu);
        } else {
            self.kill_dialog_open = false;
            self.kill_dialog
                .update(ctx, |dialog, _| dialog.clear_session());
            self.menu.update(ctx, |menu, _| menu.reset_selected_index());
        }
        ctx.emit(PromptDisplayChipEvent::ToggleMenu {
            open: self.menu_open,
        });
        ctx.notify();
    }

    fn update_sessions(
        &mut self,
        available_runtimes: Vec<RuntimeKind>,
        sessions: Vec<RuntimeSessionItem>,
        ctx: &mut ViewContext<DisplayChip>,
    ) {
        let filters = runtime_filters_for(&available_runtimes);
        let selected_filter = if filters.contains(&self.selected_filter) {
            self.selected_filter
        } else {
            filters[0]
        };
        self.sessions = sessions;
        self.selected_filter = selected_filter;
        self.filter_control.update(ctx, |control, ctx| {
            control.update_options(filters, ctx);
            control.set_selected_option(selected_filter, ctx);
        });
        self.update_filtered_menu(ctx);
    }

    fn set_filter(&mut self, filter: RuntimeSessionFilter, ctx: &mut ViewContext<DisplayChip>) {
        self.selected_filter = filter;
        self.update_filtered_menu(ctx);
        ctx.focus(&self.menu);
        ctx.notify();
    }

    fn update_filtered_menu(&self, ctx: &mut ViewContext<DisplayChip>) {
        let sessions = filter_runtime_sessions(&self.sessions, self.selected_filter);
        self.menu.update(ctx, |menu, ctx| {
            menu.update_menu_items(sessions, ctx);
        });
    }

    fn open_kill_dialog(
        &mut self,
        session: RuntimeSessionItem,
        ctx: &mut ViewContext<DisplayChip>,
    ) {
        self.kill_dialog_open = true;
        self.kill_dialog
            .update(ctx, |dialog, _| dialog.set_session(session));
        ctx.focus(&self.kill_dialog);
        ctx.notify();
    }

    fn close_kill_dialog(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.kill_dialog_open = false;
        self.kill_dialog
            .update(ctx, |dialog, _| dialog.clear_session());
        ctx.focus(&self.menu);
        ctx.notify();
    }

    pub(super) fn render(&self, display: &DisplayChip, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let color = if display.is_in_agent_view {
            agent_view_chip_color(appearance)
        } else {
            appearance.theme().ansi_fg_cyan()
        };
        let is_interactive =
            !display.is_shared_session_viewer && !display.is_cli_agent_session_active(app);
        let chip_text = display.text.clone();
        let is_in_agent_view = display.is_in_agent_view;
        let menu_open = self.menu_open;
        let hover = Hoverable::new(display.mouse_state.clone(), move |state| {
            let hovered = state.is_hovered() && is_interactive && !menu_open;
            let mut config =
                UdiChipConfig::new_with_icon(Icon::TerminalInput, color, chip_text.clone())
                    .with_hovered(hovered);
            if is_in_agent_view {
                config = config.for_agent_view();
            }
            let mut stack = Stack::new().with_child(render_udi_chip(config, appearance));
            if state.is_hovered() && is_interactive && !menu_open {
                stack.add_positioned_overlay_child(
                    appearance
                        .ui_builder()
                        .tool_tip("Attach terminal session".to_string())
                        .build()
                        .finish(),
                    udi_tooltip_positioning(),
                );
            }
            stack.finish()
        });
        let hover = if is_interactive {
            hover
                .on_click(|ctx, _, _| ctx.dispatch_typed_action(DisplayChipAction::ToggleMenu))
                .with_cursor(Cursor::PointingHand)
                .finish()
        } else {
            hover.finish()
        };

        let mut stack = Stack::new().with_child(hover);
        if self.menu_open {
            let positioning = display.menu_positioning_provider.menu_position(app);
            let (parent_anchor, child_anchor) = DisplayChip::positioning_to_anchors(positioning);
            let offset = match positioning {
                MenuPositioning::BelowInputBox => vec2f(0., 4.),
                MenuPositioning::AboveInputBox => vec2f(0., -4.),
            };
            stack.add_positioned_overlay_child(
                self.render_menu(app),
                OffsetPositioning::offset_from_parent(
                    offset,
                    ParentOffsetBounds::WindowByPosition,
                    parent_anchor,
                    child_anchor,
                ),
            );
        }
        stack.finish()
    }

    fn render_menu(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        if self.kill_dialog_open {
            return ChildView::new(&self.kill_dialog).finish();
        }
        let card = ConstrainedBox::new(
            Container::new(ChildView::new(&self.menu).finish())
                .with_background(theme.surface_2())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                .with_drop_shadow(DropShadow::default())
                .finish(),
        )
        .with_width(RUNTIME_MENU_WIDTH)
        .finish();

        Dismiss::new(card)
            .on_dismiss(|ctx, _| ctx.dispatch_typed_action(DisplayChipAction::CloseMenu))
            .finish()
    }
}

impl DisplayChip {
    fn kill_runtime_session(&mut self, session: RuntimeSessionItem, ctx: &mut ViewContext<Self>) {
        let runtime_kind = session.runtime();
        let menu = {
            let DisplayChipKind::RuntimeSessions(runtime) = &mut self.display_chip_kind else {
                return;
            };
            runtime.kill_dialog_open = false;
            runtime
                .kill_dialog
                .update(ctx, |dialog, _| dialog.clear_session());
            runtime.menu.clone()
        };

        let Some(session_context) = self.session_context.clone() else {
            show_runtime_session_toast(runtime_kind, false, ctx.window_id(), ctx);
            ctx.focus(&menu);
            ctx.notify();
            return;
        };
        let command = session.termination_command(session_context.session.shell().shell_type());
        let window_id = ctx.window_id();
        ctx.spawn(
            async move { session_context.execute_command_at_pwd(&command, None).await },
            move |display, result, ctx| {
                let success = match result {
                    Ok(output) if output.success() => true,
                    Ok(output) => {
                        log::error!(
                            "Failed to close {} session, exit code {:?}",
                            runtime_kind.label(),
                            output.exit_code()
                        );
                        false
                    }
                    Err(error) => {
                        log::error!(
                            "Failed to close {} session: {error:#}",
                            runtime_kind.label()
                        );
                        false
                    }
                };

                if let DisplayChipKind::RuntimeSessions(runtime) = &mut display.display_chip_kind {
                    runtime
                        .fetcher
                        .update(ctx, |fetcher, ctx| fetcher.refetch(ctx));
                    ctx.focus(&runtime.menu);
                }
                show_runtime_session_toast(runtime_kind, success, window_id, ctx);
                ctx.notify();
            },
        );
    }
}

fn show_runtime_session_toast(
    runtime: RuntimeKind,
    success: bool,
    window_id: warpui::WindowId,
    ctx: &mut ViewContext<DisplayChip>,
) {
    let message = if success {
        format!("Closed {} session", runtime.label())
    } else {
        format!("Could not close {} session", runtime.label())
    };
    ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
        let toast = if success {
            DismissibleToast::success(message)
        } else {
            DismissibleToast::error(message)
        };
        toast_stack.add_ephemeral_toast(toast, window_id, ctx);
    });
}

fn runtime_filter_styles(app: &AppContext) -> UiComponentStyles {
    let appearance = Appearance::as_ref(app);
    let theme = appearance.theme();
    UiComponentStyles {
        font_family_id: Some(appearance.ui_font_family()),
        font_size: Some(12.),
        background: Some(UiFill::Solid(theme.surface_1().into())),
        border_radius: Some(CornerRadius::with_all(Radius::Pixels(4.))),
        border_width: Some(1.),
        border_color: Some(UiFill::Solid(theme.surface_3().into())),
        ..Default::default()
    }
}
