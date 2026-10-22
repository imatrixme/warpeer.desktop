use std::collections::HashSet;
use std::sync::Arc;

use warpui::elements::{Dismiss, DropShadow};

use super::*;
use crate::terminal::history::{History, HistoryEntry, HistoryEvent};
use crate::terminal::model::session::SessionId;

const SSH_MENU_WIDTH: f32 = 360.;
const MAX_RECENT_SSH_COMMANDS: usize = 20;

#[derive(Clone, Debug)]
pub(crate) struct SshConnectionsChip {
    menu_open: bool,
    menu: ViewHandle<DisplayChipMenu>,
    session_id: Option<SessionId>,
}

impl SshConnectionsChip {
    pub(super) fn new(config: &DisplayChipConfig, ctx: &mut ViewContext<DisplayChip>) -> Self {
        let session_id = config
            .session_context
            .as_ref()
            .map(|session_context| session_context.session.id());
        let initial_items = ssh_history_items(session_id, ctx);
        let menu = ctx.add_typed_action_view(move |ctx| {
            DisplayChipMenu::new(initial_items, None, ChipMenuType::Branches, ctx)
                .with_search_placeholder("Search SSH connections...", ctx)
                .embedded()
        });

        ctx.subscribe_to_view(&menu, |display, _, event, ctx| match event {
            PromptDisplayMenuEvent::MenuAction(event) => {
                let Some(item) = event.action_item.as_any().downcast_ref::<SshHistoryItem>() else {
                    log::warn!("SSH history menu action contained an unexpected item");
                    return;
                };
                ctx.emit(PromptDisplayChipEvent::TryExecuteCommand(
                    PromptChipShellCommand::SshConnect {
                        command: item.command().to_string(),
                    },
                ));
                if let DisplayChipKind::SshConnections(ssh) = &mut display.display_chip_kind {
                    ssh.close(ctx);
                }
            }
            PromptDisplayMenuEvent::MenuSecondaryAction(_) => {}
            PromptDisplayMenuEvent::CloseMenu => {
                if let DisplayChipKind::SshConnections(ssh) = &mut display.display_chip_kind {
                    ssh.close(ctx);
                }
                ctx.emit(PromptDisplayChipEvent::ToggleMenu { open: false });
            }
        });

        let history = History::handle(ctx);
        ctx.subscribe_to_model(&history, |display, _, event, ctx| match event {
            HistoryEvent::Initialized(session_id) => {
                if let DisplayChipKind::SshConnections(ssh) = &mut display.display_chip_kind
                    && ssh.session_id == Some(*session_id)
                {
                    ssh.refresh(ctx);
                }
            }
        });

        Self {
            menu_open: false,
            menu,
            session_id,
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.menu_open
    }

    pub(super) fn close(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = false;
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
        &mut self,
        session_context: Option<SessionContext>,
        ctx: &mut ViewContext<DisplayChip>,
    ) {
        self.session_id = session_context.map(|session_context| session_context.session.id());
        self.refresh(ctx);
    }

    pub(super) fn toggle(&mut self, ctx: &mut ViewContext<DisplayChip>) {
        self.menu_open = !self.menu_open;
        if self.menu_open {
            self.refresh(ctx);
            ctx.focus(&self.menu);
        } else {
            self.menu.update(ctx, |menu, _| menu.reset_selected_index());
        }
        ctx.emit(PromptDisplayChipEvent::ToggleMenu {
            open: self.menu_open,
        });
        ctx.notify();
    }

    fn refresh(&self, ctx: &mut ViewContext<DisplayChip>) {
        let items = ssh_history_items(self.session_id, ctx);
        self.menu.update(ctx, |menu, ctx| {
            menu.update_menu_items(items, ctx);
        });
    }

    pub(super) fn render(&self, display: &DisplayChip, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let color = if display.is_in_agent_view {
            agent_view_chip_color(appearance)
        } else {
            appearance.theme().ansi_fg_blue()
        };
        let is_interactive =
            !display.is_shared_session_viewer && !display.is_cli_agent_session_active(app);
        let chip_text = display.text.clone();
        let is_in_agent_view = display.is_in_agent_view;
        let menu_open = self.menu_open;
        let hover = Hoverable::new(display.mouse_state.clone(), move |state| {
            let hovered = state.is_hovered() && is_interactive && !menu_open;
            let mut config = UdiChipConfig::new_with_icon(Icon::Globe, color, chip_text.clone())
                .with_hovered(hovered);
            if is_in_agent_view {
                config = config.for_agent_view();
            }
            let mut stack = Stack::new().with_child(render_udi_chip(config, appearance));
            if state.is_hovered() && is_interactive && !menu_open {
                stack.add_positioned_overlay_child(
                    appearance
                        .ui_builder()
                        .tool_tip("Connect with a recent SSH command".to_string())
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
        let theme = Appearance::as_ref(app).theme();
        let card = ConstrainedBox::new(
            Container::new(ChildView::new(&self.menu).finish())
                .with_background(theme.surface_2())
                .with_corner_radius(CornerRadius::with_all(Radius::Pixels(6.)))
                .with_drop_shadow(DropShadow::default())
                .finish(),
        )
        .with_width(SSH_MENU_WIDTH)
        .finish();

        Dismiss::new(card)
            .on_dismiss(|ctx, _| ctx.dispatch_typed_action(DisplayChipAction::CloseMenu))
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SshHistoryItem {
    command: String,
}

impl SshHistoryItem {
    fn command(&self) -> &str {
        &self.command
    }
}

impl GenericMenuItem for SshHistoryItem {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> String {
        self.command.clone()
    }

    fn icon(&self, _app: &AppContext) -> Option<Icon> {
        Some(Icon::Globe)
    }

    fn action_data(&self) -> String {
        self.command.clone()
    }
}

fn ssh_history_items(session_id: Option<SessionId>, app: &AppContext) -> Vec<SshHistoryItem> {
    let Some(session_id) = session_id else {
        return Vec::new();
    };
    let history = History::as_ref(app);
    if !history.is_queryable(&session_id) {
        return Vec::new();
    }
    history
        .commands_shared(session_id)
        .map(|entries| recent_ssh_commands(&entries))
        .unwrap_or_default()
}

fn recent_ssh_commands(entries: &[Arc<HistoryEntry>]) -> Vec<SshHistoryItem> {
    let mut seen = HashSet::new();
    entries
        .iter()
        .rev()
        .filter_map(|entry| normalized_ssh_command(&entry.command))
        .filter(|command| seen.insert(command.clone()))
        .take(MAX_RECENT_SSH_COMMANDS)
        .map(|command| SshHistoryItem { command })
        .collect()
}

fn normalized_ssh_command(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() || command.contains(['\n', '\r']) {
        return None;
    }
    let tokens = shell_words::split(command).ok()?;
    let mut executable_index = tokens
        .iter()
        .position(|token| !is_shell_assignment(token))?;
    if tokens.get(executable_index).map(String::as_str) == Some("command") {
        executable_index += 1;
    }
    let executable = tokens.get(executable_index)?;
    (executable.rsplit('/').next() == Some("ssh")).then(|| command.to_string())
}

fn is_shell_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

#[cfg(test)]
#[path = "ssh_tests.rs"]
mod tests;
