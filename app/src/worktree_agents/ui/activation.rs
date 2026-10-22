use warpui::{SingletonEntity as _, ViewContext};

use super::{HeaderActionOutcome, WorktreeAgentHeaderView};
use crate::terminal::view::{TerminalAction, TerminalView};
use crate::view_components::DismissibleToast;
use crate::workspace::{ToastStack, WorkspaceAction};
use crate::worktree_agents::CurrentPaneLaunchResult;
use crate::worktree_agents::coordinator::AgentActivationDecision;
use crate::worktree_agents::observation::AgentKey;

pub(super) fn activate(
    view: &mut WorktreeAgentHeaderView,
    key: &AgentKey,
    generation: u64,
    ctx: &mut ViewContext<WorktreeAgentHeaderView>,
) -> HeaderActionOutcome {
    let decision = view.coordinator.read(ctx, |coordinator, _| {
        coordinator.resolve_activation(view.terminal_view_id, key, generation)
    });
    match decision {
        AgentActivationDecision::FocusPane(terminal_view_id) => {
            ctx.dispatch_typed_action(&WorkspaceAction::FocusTerminalViewInWorkspace {
                terminal_view_id,
            });
            show_success(key, "Focused", ctx);
            view.dismiss(ctx);
            HeaderActionOutcome::Activated
        }
        AgentActivationDecision::Launch(launch) => {
            let result = view.terminal_view.upgrade(ctx).map_or(
                CurrentPaneLaunchResult::Unsupported,
                |terminal_view| {
                    terminal_view.update(ctx, |terminal_view, ctx| {
                        terminal_view.try_launch_worktree_agent(&launch, ctx)
                    })
                },
            );
            match result {
                CurrentPaneLaunchResult::Launched => {
                    view.coordinator.update(ctx, |coordinator, ctx| {
                        coordinator.bind_activated_agent(view.terminal_view_id, key.clone(), ctx);
                    });
                    show_success(key, "Attached", ctx);
                    view.dismiss(ctx);
                    HeaderActionOutcome::Activated
                }
                CurrentPaneLaunchResult::Unsupported => {
                    show_error(
                        "Current pane cannot switch to this session without interrupting its command",
                        ctx,
                    );
                    HeaderActionOutcome::ObserveOnly
                }
            }
        }
        AgentActivationDecision::AlreadyFocused => {
            show_notice("This agent is already in the current pane", ctx);
            view.dismiss(ctx);
            HeaderActionOutcome::AlreadyFocused
        }
        AgentActivationDecision::Stale => {
            view.coordinator.update(ctx, |coordinator, ctx| {
                coordinator.refresh_sources(ctx);
            });
            show_error("Agent route expired. Refreshing sessions.", ctx);
            HeaderActionOutcome::Stale
        }
        AgentActivationDecision::ObserveOnly => {
            show_notice("This session is observable but has no exact route", ctx);
            HeaderActionOutcome::ObserveOnly
        }
    }
}

pub(super) fn detach(
    view: &mut WorktreeAgentHeaderView,
    key: &AgentKey,
    ctx: &mut ViewContext<WorktreeAgentHeaderView>,
) {
    let terminal_view_id = view.coordinator.read(ctx, |coordinator, _| {
        coordinator.attached_pane_for_agent(key)
    });
    let Some(terminal_view_id) = terminal_view_id else {
        show_error("This agent is no longer attached to a pane", ctx);
        return;
    };
    let window_ids = ctx.window_ids().collect::<Vec<_>>();
    let target_window = window_ids.into_iter().find(|window_id| {
        ctx.views_of_type::<TerminalView>(*window_id)
            .is_some_and(|views| {
                views
                    .iter()
                    .any(|terminal| terminal.id() == terminal_view_id)
            })
    });
    let Some(window_id) = target_window else {
        show_error("The attached pane is no longer available", ctx);
        return;
    };
    ctx.dispatch_typed_action_for_view(window_id, terminal_view_id, &TerminalAction::ResetPane);
    show_success(key, "Detached", ctx);
    view.dismiss(ctx);
}

fn show_success(
    key: &AgentKey,
    action: &'static str,
    ctx: &mut ViewContext<WorktreeAgentHeaderView>,
) {
    let worktree = key
        .worktree
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("worktree");
    add_toast(
        DismissibleToast::success(format!("{action} {} in {worktree}", provider_name(key))),
        ctx,
    );
}

fn provider_name(key: &AgentKey) -> &'static str {
    key.provider.display_name()
}

pub(super) fn show_notice(message: &str, ctx: &mut ViewContext<WorktreeAgentHeaderView>) {
    add_toast(DismissibleToast::default(message.to_string()), ctx);
}

fn show_error(message: &str, ctx: &mut ViewContext<WorktreeAgentHeaderView>) {
    add_toast(DismissibleToast::error(message.to_string()), ctx);
}

fn add_toast(
    toast: DismissibleToast<WorkspaceAction>,
    ctx: &mut ViewContext<WorktreeAgentHeaderView>,
) {
    let window_id = ctx.window_id();
    ToastStack::handle(ctx).update(ctx, |toast_stack, ctx| {
        toast_stack.add_ephemeral_toast(toast, window_id, ctx);
    });
}
