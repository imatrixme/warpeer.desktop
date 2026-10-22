use warp_completer::completer::GeneratorContext as _;

use super::*;
use crate::context_chips::runtime_sessions::RuntimeKind;
use crate::menu::SubMenu;
use crate::terminal::model::session::command_executor::shell_quote_arg;
#[cfg(feature = "warpeer")]
use crate::worktree_agents::{WorktreeAgentCoordinator, WorktreeAgentLaunch, herdr_focus_command};

#[cfg(feature = "warpeer")]
#[path = "runtime_controls/tmux.rs"]
mod tmux;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeOperation {
    NewTab,
    PreviousTab,
    NextTab,
    SplitRight,
    SplitDown,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    ToggleZoom,
    Detach,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeControlAction {
    Execute {
        runtime: RuntimeKind,
        session_name: String,
        operation: RuntimeOperation,
    },
    DetachHerdr,
}

impl RuntimeOperation {
    fn command(self, runtime: RuntimeKind, session_name: &str, shell: ShellType) -> String {
        let session = shell_quote_arg(session_name, shell);
        match (runtime, self) {
            (RuntimeKind::Tmux, Self::NewTab) => format!("tmux new-window -t {session}:"),
            (RuntimeKind::Tmux, Self::PreviousTab) => {
                format!("tmux previous-window -t {session}")
            }
            (RuntimeKind::Tmux, Self::NextTab) => format!("tmux next-window -t {session}"),
            (RuntimeKind::Tmux, Self::SplitRight) => {
                format!("tmux split-window -h -t {session}:")
            }
            (RuntimeKind::Tmux, Self::SplitDown) => {
                format!("tmux split-window -v -t {session}:")
            }
            (RuntimeKind::Tmux, Self::ToggleZoom) => {
                format!("tmux resize-pane -Z -t {session}:")
            }
            (RuntimeKind::Tmux, Self::Detach) => {
                format!("tmux detach-client -s {session}")
            }
            (RuntimeKind::Herdr, Self::NewTab) => {
                format!("herdr --session {session} tab create --focus")
            }
            (RuntimeKind::Herdr, Self::SplitRight) => {
                format!("herdr --session {session} pane split --current --direction right --focus")
            }
            (RuntimeKind::Herdr, Self::SplitDown) => {
                format!("herdr --session {session} pane split --current --direction down --focus")
            }
            (RuntimeKind::Herdr, Self::FocusLeft) => {
                format!("herdr --session {session} pane focus --current --direction left")
            }
            (RuntimeKind::Herdr, Self::FocusRight) => {
                format!("herdr --session {session} pane focus --current --direction right")
            }
            (RuntimeKind::Herdr, Self::FocusUp) => {
                format!("herdr --session {session} pane focus --current --direction up")
            }
            (RuntimeKind::Herdr, Self::FocusDown) => {
                format!("herdr --session {session} pane focus --current --direction down")
            }
            (RuntimeKind::Herdr, Self::ToggleZoom) => {
                format!("herdr --session {session} pane zoom --current --toggle")
            }
            (RuntimeKind::Herdr, Self::Detach)
            | (RuntimeKind::Herdr, Self::PreviousTab | Self::NextTab)
            | (
                RuntimeKind::Tmux,
                Self::FocusLeft | Self::FocusRight | Self::FocusUp | Self::FocusDown,
            ) => {
                unreachable!()
            }
        }
    }
}

impl TerminalView {
    #[cfg(feature = "warpeer")]
    pub(super) fn reset_pane(&mut self, ctx: &mut ViewContext<Self>) {
        if !crate::product_profile::is_terminal_only() {
            return;
        }
        let coordinator = WorktreeAgentCoordinator::handle(ctx);
        let working_directory = coordinator
            .read(ctx, |coordinator, _| {
                coordinator.attached_agent_worktree_path(self.view_id)
            })
            .or_else(|| self.active_session_path_if_local(ctx));
        coordinator.update(ctx, |coordinator, ctx| {
            coordinator.detach_activated_agent(self.view_id, ctx);
        });
        ctx.emit(Event::Pane(PaneEvent::ResetTerminal { working_directory }));
    }

    #[cfg(feature = "warpeer")]
    pub(super) fn try_switch_worktree_runtime(
        &mut self,
        launch: &WorktreeAgentLaunch,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        match launch {
            WorktreeAgentLaunch::AttachTmux { target, .. } => {
                let Some(source_session) = self.active_tmux_session(ctx) else {
                    return false;
                };
                let target = tmux::TmuxSwitchTarget {
                    socket: target.socket.clone(),
                    source_session,
                    target_session: target.session.clone(),
                    target_window: target.window.clone(),
                    target_pane: target.pane.clone(),
                };
                ctx.spawn(
                    async move { tmux::switch_client(target).await },
                    |_, result, _| {
                        if let Err(error) = result {
                            log::warn!("Failed to switch tmux client: {error:#}");
                        }
                    },
                );
                true
            }
            WorktreeAgentLaunch::AttachHerdr { target, .. }
                if self.active_runtime(ctx) == Some(RuntimeKind::Herdr) =>
            {
                let Some(session_context) = self.input.as_ref(ctx).completion_session_context(ctx)
                else {
                    return false;
                };
                let session_id = session_context.session.id();
                let env_vars = self
                    .sessions
                    .as_ref(ctx)
                    .get_env_vars_for_session(session_id);
                let command =
                    herdr_focus_command(target, session_context.session.shell().shell_type());
                ctx.spawn(
                    async move {
                        session_context
                            .execute_command_at_pwd(&command, env_vars)
                            .await
                    },
                    |_, result, _| match result {
                        Ok(output) if output.success() => {}
                        Ok(output) => log::warn!(
                            "Failed to switch resumable agent runtime, exit code {:?}",
                            output.exit_code()
                        ),
                        Err(error) => {
                            log::warn!("Failed to switch resumable agent runtime: {error:#}")
                        }
                    },
                );
                true
            }
            WorktreeAgentLaunch::OpenDirectory(_)
            | WorktreeAgentLaunch::AttachHerdr { .. }
            | WorktreeAgentLaunch::NativeResume(_) => false,
        }
    }

    #[cfg(feature = "warpeer")]
    fn active_runtime(&self, ctx: &AppContext) -> Option<RuntimeKind> {
        self.active_runtime_command(ctx).map(|(runtime, _)| runtime)
    }

    #[cfg(feature = "warpeer")]
    fn active_tmux_session(&self, ctx: &AppContext) -> Option<String> {
        let (runtime, command) = self.active_runtime_command(ctx)?;
        (runtime == RuntimeKind::Tmux)
            .then(|| tmux::attached_session_from_command(&command))
            .flatten()
    }

    #[cfg(feature = "warpeer")]
    fn active_runtime_command(&self, ctx: &AppContext) -> Option<(RuntimeKind, String)> {
        let model = self.model.lock();
        let active_block = model.block_list().active_block();
        if !active_block.is_active_and_long_running() {
            return None;
        }
        let executable = active_block.top_level_command(self.sessions.as_ref(ctx))?;
        Some((
            RuntimeKind::from_executable(&executable)?,
            active_block.command_to_string(),
        ))
    }

    pub(super) fn runtime_context_menu_item(
        &self,
        ctx: &AppContext,
    ) -> Option<MenuItem<TerminalAction>> {
        if !crate::product_profile::is_terminal_only() {
            return None;
        }

        let (runtime, session_name) = {
            let model = self.model.lock();
            let active_block = model.block_list().active_block();
            if !active_block.is_active_and_long_running() {
                return None;
            }
            let executable = active_block.top_level_command(self.sessions.as_ref(ctx))?;
            let runtime = RuntimeKind::from_executable(&executable)?;
            let session_name = runtime.session_name_from_command(&active_block.command_to_string());
            (runtime, session_name)
        };

        let session_name = session_name
            .or_else(|| (runtime == RuntimeKind::Herdr).then(|| "default".to_string()))?;
        let items = match runtime {
            RuntimeKind::Tmux => tmux_menu_items(&session_name),
            RuntimeKind::Herdr => herdr_menu_items(&session_name),
        };
        Some(MenuItem::Submenu {
            fields: MenuItemFields::new_submenu(runtime.label()),
            menu: SubMenu::new(items),
        })
    }

    pub(super) fn handle_runtime_control(
        &mut self,
        action: &RuntimeControlAction,
        ctx: &mut ViewContext<Self>,
    ) {
        let RuntimeControlAction::Execute {
            runtime,
            session_name,
            operation,
        } = action
        else {
            self.write_user_bytes_to_pty(vec![escape_sequences::C0::ETX], ctx);
            return;
        };

        let Some(session_context) = self.input.as_ref(ctx).completion_session_context(ctx) else {
            return;
        };
        let command = operation.command(
            *runtime,
            session_name,
            session_context.session.shell().shell_type(),
        );
        ctx.spawn(
            async move { session_context.execute_command_at_pwd(&command, None).await },
            |_, result, _| match result {
                Ok(output) if output.success() => {}
                Ok(output) => log::warn!(
                    "Resumable terminal runtime command failed with exit code {:?}",
                    output.exit_code()
                ),
                Err(error) => {
                    log::warn!("Failed to control resumable terminal runtime: {error:#}")
                }
            },
        );
    }
}

fn runtime_action(
    label: &str,
    runtime: RuntimeKind,
    session_name: &str,
    operation: RuntimeOperation,
) -> MenuItem<TerminalAction> {
    MenuItemFields::new(label)
        .with_on_select_action(TerminalAction::RuntimeControl(
            RuntimeControlAction::Execute {
                runtime,
                session_name: session_name.to_string(),
                operation,
            },
        ))
        .into_item()
}

fn tmux_menu_items(session_name: &str) -> Vec<MenuItem<TerminalAction>> {
    let runtime = RuntimeKind::Tmux;
    [
        ("New window", RuntimeOperation::NewTab),
        ("Previous window", RuntimeOperation::PreviousTab),
        ("Next window", RuntimeOperation::NextTab),
        ("Split right", RuntimeOperation::SplitRight),
        ("Split down", RuntimeOperation::SplitDown),
        ("Toggle pane zoom", RuntimeOperation::ToggleZoom),
        ("Detach", RuntimeOperation::Detach),
    ]
    .into_iter()
    .map(|(label, operation)| runtime_action(label, runtime, session_name, operation))
    .collect()
}

fn herdr_menu_items(session_name: &str) -> Vec<MenuItem<TerminalAction>> {
    let runtime = RuntimeKind::Herdr;
    let mut items = [
        ("New tab", RuntimeOperation::NewTab),
        ("Split right", RuntimeOperation::SplitRight),
        ("Split down", RuntimeOperation::SplitDown),
        ("Focus left", RuntimeOperation::FocusLeft),
        ("Focus right", RuntimeOperation::FocusRight),
        ("Focus up", RuntimeOperation::FocusUp),
        ("Focus down", RuntimeOperation::FocusDown),
        ("Toggle pane zoom", RuntimeOperation::ToggleZoom),
    ]
    .into_iter()
    .map(|(label, operation)| runtime_action(label, runtime, session_name, operation))
    .collect::<Vec<_>>();
    items.push(
        MenuItemFields::new("Detach")
            .with_on_select_action(TerminalAction::RuntimeControl(
                RuntimeControlAction::DetachHerdr,
            ))
            .into_item(),
    );
    items
}

#[cfg(test)]
#[path = "runtime_controls_tests.rs"]
mod tests;
