use std::path::PathBuf;

use warp_terminal::shell::ShellType;
use warpui::integration::TestStep;
use warpui::{TypedActionView as _, async_assert};

use super::view_getters::{single_terminal_view_for_tab, workspace_view};
use crate::workspace::WorkspaceAction;
use crate::worktree_agents::{AgentProvider, NativeResumeTarget, WorktreeAgentLaunch};

pub fn open_claude_resume_in_new_tab(session_id: &'static str) -> TestStep {
    TestStep::new("Open exact Claude resume in a new worktree tab").with_action(
        move |app, window_id, _| {
            let worktree = PathBuf::from(std::env::var_os("HOME").expect("HOME must be set"))
                .join("worktree-agent-control-center");
            workspace_view(app, window_id).update(app, |workspace, ctx| {
                workspace.handle_action(
                    &WorkspaceAction::OpenWarpeerTerminal {
                        launch: Box::new(WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
                            provider: AgentProvider::ClaudeCode,
                            session_id: session_id.to_string(),
                            worktree,
                        })),
                        agent_key: None,
                    },
                    ctx,
                );
            });
        },
    )
}

pub fn assert_exact_resume_was_executed(tab_index: usize, session_id: &'static str) -> TestStep {
    TestStep::new("Assert the exact resume command ran in the new tab").add_named_assertion(
        "resume command exists in terminal history",
        move |app, window_id| {
            let worktree = PathBuf::from(std::env::var_os("HOME").expect("HOME must be set"))
                .join("worktree-agent-control-center");
            let expected = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
                provider: AgentProvider::ClaudeCode,
                session_id: session_id.to_string(),
                worktree,
            })
            .command(ShellType::Zsh)
            .expect("Claude resume should render a command");
            let terminal = single_terminal_view_for_tab(app, window_id, tab_index);
            terminal.read(app, |terminal, _| {
                let model = terminal.model.lock();
                let commands: Vec<_> = model
                    .block_list()
                    .blocks()
                    .iter()
                    .map(|block| block.command_to_string())
                    .collect();
                async_assert!(
                    commands.iter().any(|command| command == &expected),
                    "resume command was not executed: {commands:?}"
                )
            })
        },
    )
}
