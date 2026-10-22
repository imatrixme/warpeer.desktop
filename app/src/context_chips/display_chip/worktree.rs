use std::sync::Arc;

use warpui::elements::{Dismiss, DropShadow};
use warpui::text_layout::ClipConfig;

#[path = "worktree/create_dialog.rs"]
mod create_dialog;
#[path = "worktree/create_dialog_components.rs"]
mod create_dialog_components;
#[path = "worktree/create_operation.rs"]
mod create_operation;

use create_dialog::{CreateWorktreeDialog, CreateWorktreeDialogEvent};

use super::*;
use crate::view_components::DismissibleToast;
use crate::workspace::ToastStack;
use crate::worktree_agents::{
    ExecutionHostKey, WorktreeAgentCoordinator, WorktreeMenuEntry, decode_menu_entries,
};
const WORKTREE_MENU_WIDTH: f32 = 400.;

#[derive(Clone, Debug)]
pub(crate) struct WorktreeChip {
    menu_open: bool,
    dialog_open: bool,
    can_create: bool,
    entries: Vec<WorktreeMenuEntry>,
    menu: ViewHandle<DisplayChipMenu>,
    create_dialog: ViewHandle<CreateWorktreeDialog>,
}

impl WorktreeChip {
    pub(super) fn new(
        on_click_values: &[String],
        config: &DisplayChipConfig,
        ctx: &mut ViewContext<DisplayChip>,
    ) -> Self {
        let entries = decode_menu_entries(on_click_values);
        let items = menu_items_from_entries(entries.clone());
        let can_create = config
            .session_context
            .as_ref()
            .is_some_and(|session| matches!(session.session.session_type(), SessionType::Local));
        let menu = ctx.add_typed_action_view(move |ctx| {
            DisplayChipMenu::new(
                items,
                can_create.then(|| FixedFooter::new(Arc::new(NewWorktreeMenuItem))),
                ChipMenuType::Branches,
                ctx,
            )
            .with_search_placeholder("Search worktrees...", ctx)
            .with_fixed_footer_after_items()
            .embedded()
        });
        ctx.subscribe_to_view(&menu, |display, _, event, ctx| match event {
            PromptDisplayMenuEvent::MenuAction(event) => {
                if event.action_item.as_any().is::<NewWorktreeMenuItem>() {
                    display.open_create_worktree_dialog(ctx);
                    return;
                }
                let Some(item) = event
                    .action_item
                    .as_any()
                    .downcast_ref::<WorktreeMenuItem>()
                else {
                    log::warn!("Worktree menu action contained an unexpected item");
                    return;
                };
                if item.entry.can_navigate() {
                    ctx.emit(PromptDisplayChipEvent::TryExecuteCommand(
                        PromptChipShellCommand::NavigateToWorktree {
                            dir_name: item.entry.path.clone(),
                        },
                    ));
                } else if item.entry.prunable {
                    ctx.emit(PromptDisplayChipEvent::TryExecuteCommand(
                        PromptChipShellCommand::Echo {
                            message: "This worktree is no longer available.",
                        },
                    ));
                }
                display.close_worktree_menu(ctx);
            }
            PromptDisplayMenuEvent::MenuSecondaryAction(_) => {}
            PromptDisplayMenuEvent::CloseMenu => {
                display.close_worktree_menu(ctx);
                ctx.emit(PromptDisplayChipEvent::ToggleMenu { open: false });
            }
        });
        let create_dialog = ctx.add_typed_action_view(CreateWorktreeDialog::new);
        ctx.subscribe_to_view(&create_dialog, |display, _, event, ctx| match event {
            CreateWorktreeDialogEvent::Dismissed => display.close_create_worktree_dialog(ctx),
            CreateWorktreeDialogEvent::Created(entry) => {
                display.finish_create_worktree(entry.clone(), ctx)
            }
        });
        let terminal_view_id = config.terminal_view_id;
        let coordinator = WorktreeAgentCoordinator::handle(ctx);
        ctx.subscribe_to_model(&coordinator, move |display, coordinator, _, ctx| {
            let entries = coordinator.read(ctx, |coordinator, _| {
                coordinator.menu_entries_for_terminal(terminal_view_id)
            });
            if let DisplayChipKind::Worktrees(worktrees) = &mut display.display_chip_kind {
                worktrees.update_entries(entries, ctx);
            }
        });
        Self {
            menu_open: false,
            dialog_open: false,
            can_create,
            entries,
            menu,
            create_dialog,
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.menu_open || self.dialog_open
    }

    pub(super) fn close(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = false;
        self.dialog_open = false;
        self.menu.update(ctx, |menu, _| menu.reset_selected_index());
        self.create_dialog
            .update(ctx, |dialog, ctx| dialog.hide(ctx));
        ctx.notify();
    }

    pub(super) fn focus_if_open(&self, ctx: &mut ViewContext<DisplayChip>) -> bool {
        if self.dialog_open {
            ctx.focus(&self.create_dialog);
            true
        } else if self.menu_open {
            ctx.focus(&self.menu);
            true
        } else {
            false
        }
    }

    pub(super) fn toggle(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = !self.menu_open;
        if self.menu_open {
            ctx.focus(&self.menu);
        } else {
            self.menu.update(ctx, |menu, _| menu.reset_selected_index());
        }
        ctx.emit(PromptDisplayChipEvent::ToggleMenu {
            open: self.menu_open,
        });
        ctx.notify();
    }

    fn update_entries(
        &mut self,
        entries: Vec<WorktreeMenuEntry>,
        ctx: &mut ViewContext<DisplayChip>,
    ) {
        self.entries = entries.clone();
        self.menu.update(ctx, |menu, ctx| {
            menu.update_menu_items(menu_items_from_entries(entries), ctx);
        });
        ctx.notify();
    }

    pub(super) fn render(&self, display: &DisplayChip, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let color = if display.is_in_agent_view {
            agent_view_chip_color(appearance)
        } else {
            appearance.theme().ansi_fg_green()
        };
        let is_interactive =
            !display.is_shared_session_viewer && !display.is_cli_agent_session_active(app);
        let chip_text = display.text.clone();
        let is_in_agent_view = display.is_in_agent_view;
        let menu_open = self.menu_open;
        let hover = Hoverable::new(display.mouse_state.clone(), move |state| {
            let hovered = state.is_hovered() && is_interactive && !menu_open;
            let mut config =
                UdiChipConfig::new_with_icon(Icon::Dataflow02, color, chip_text.clone())
                    .with_hovered(hovered);
            if is_in_agent_view {
                config = config.for_agent_view();
            }
            let mut stack = Stack::new().with_child(render_udi_chip(config, appearance));
            if state.is_hovered() && is_interactive && !menu_open {
                stack.add_positioned_overlay_child(
                    appearance
                        .ui_builder()
                        .tool_tip("Switch Git worktree".to_string())
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
        if self.dialog_open {
            let positioning = display.menu_positioning_provider.menu_position(app);
            let (parent_anchor, child_anchor) = DisplayChip::positioning_to_anchors(positioning);
            let offset = match positioning {
                MenuPositioning::BelowInputBox => vec2f(0., 6.),
                MenuPositioning::AboveInputBox => vec2f(0., -6.),
            };
            stack.add_positioned_overlay_child(
                ChildView::new(&self.create_dialog).finish(),
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
        let theme = Appearance::as_ref(app).theme();
        let card = ConstrainedBox::new(
            Container::new(ChildView::new(&self.menu).finish())
                .with_background(theme.surface_2())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                .with_drop_shadow(DropShadow::default())
                .finish(),
        )
        .with_width(WORKTREE_MENU_WIDTH)
        .finish();
        Dismiss::new(card)
            .on_dismiss(|ctx, _| ctx.dispatch_typed_action(DisplayChipAction::CloseMenu))
            .finish()
    }
}

impl DisplayChip {
    fn open_create_worktree_dialog(&mut self, ctx: &mut ViewContext<Self>) {
        let DisplayChipKind::Worktrees(worktrees) = &mut self.display_chip_kind else {
            return;
        };
        if !worktrees.can_create {
            return;
        }
        worktrees.menu_open = false;
        worktrees.dialog_open = true;
        worktrees
            .menu
            .update(ctx, |menu, _| menu.reset_selected_index());
        let entries = worktrees.entries.clone();
        let dialog = worktrees.create_dialog.clone();
        dialog.update(ctx, |dialog, ctx| dialog.show(entries, ctx));
        ctx.focus(&dialog);
        ctx.emit(PromptDisplayChipEvent::ToggleMenu { open: false });
        ctx.notify();
    }

    fn close_create_worktree_dialog(&mut self, ctx: &mut ViewContext<Self>) {
        let DisplayChipKind::Worktrees(worktrees) = &mut self.display_chip_kind else {
            return;
        };
        worktrees.dialog_open = false;
        worktrees
            .create_dialog
            .update(ctx, |dialog, ctx| dialog.hide(ctx));
        ctx.notify();
    }

    fn finish_create_worktree(&mut self, entry: WorktreeMenuEntry, ctx: &mut ViewContext<Self>) {
        let terminal_view_id = self.terminal_view_id;
        let path = entry.path.clone();
        let entries = {
            let DisplayChipKind::Worktrees(worktrees) = &mut self.display_chip_kind else {
                return;
            };
            worktrees.dialog_open = false;
            worktrees.entries.push(entry);
            worktrees.entries.sort_by(|left, right| {
                right
                    .is_current
                    .cmp(&left.is_current)
                    .then_with(|| left.path.cmp(&right.path))
            });
            let entries = worktrees.entries.clone();
            worktrees.update_entries(entries.clone(), ctx);
            entries
        };
        WorktreeAgentCoordinator::handle(ctx).update(ctx, |coordinator, ctx| {
            coordinator.publish_catalog(terminal_view_id, ExecutionHostKey::Local, &entries, ctx);
        });
        ctx.emit(PromptDisplayChipEvent::TryExecuteCommand(
            PromptChipShellCommand::NavigateToWorktree { dir_name: path },
        ));
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(
                DismissibleToast::success("Worktree created".to_string()),
                window_id,
                ctx,
            );
        });
        ctx.notify();
    }
}

#[derive(Clone, Debug)]
struct WorktreeMenuItem {
    entry: WorktreeMenuEntry,
}

impl GenericMenuItem for WorktreeMenuItem {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> String {
        self.entry.compact_name()
    }

    fn icon(&self, _app: &AppContext) -> Option<Icon> {
        Some(Icon::Dataflow02)
    }

    fn action_data(&self) -> String {
        self.entry.path.clone()
    }

    fn right_side_element(&self, _index: usize, app: &AppContext) -> Option<Box<dyn Element>> {
        let appearance = Appearance::as_ref(app);
        Some(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(render_tag(self.entry.checkout_type_label(), appearance))
                .with_child(
                    Container::new(render_tag(&self.entry.git_state_label(), appearance))
                        .with_margin_left(4.)
                        .finish(),
                )
                .finish(),
        )
    }
}

#[derive(Clone, Debug)]
struct NewWorktreeMenuItem;

impl GenericMenuItem for NewWorktreeMenuItem {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> String {
        "New worktree...".to_string()
    }

    fn icon(&self, _app: &AppContext) -> Option<Icon> {
        Some(Icon::Plus)
    }

    fn action_data(&self) -> String {
        String::new()
    }
}

fn render_tag(label: &str, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    ConstrainedBox::new(
        Container::new(
            Text::new_inline(label.to_string(), appearance.ui_font_family(), 9.)
                .with_color(theme.sub_text_color(theme.surface_3()).into())
                .with_clip(ClipConfig::ellipsis())
                .soft_wrap(false)
                .finish(),
        )
        .with_background(theme.surface_3().with_opacity(70))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_horizontal_padding(5.)
        .with_vertical_padding(1.)
        .finish(),
    )
    .with_max_width(140.)
    .finish()
}

fn menu_items_from_entries(entries: Vec<WorktreeMenuEntry>) -> Vec<WorktreeMenuItem> {
    entries
        .into_iter()
        .map(|entry| WorktreeMenuItem { entry })
        .collect()
}

#[cfg(test)]
fn menu_items(values: &[String]) -> Vec<WorktreeMenuItem> {
    menu_items_from_entries(decode_menu_entries(values))
}

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod tests;
