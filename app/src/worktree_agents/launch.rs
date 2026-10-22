use std::path::{Path, PathBuf};

use warp_terminal::shell::ShellType;

use super::providers::AgentProvider;
use super::runtimes::{TmuxAttachTarget, herdr_attach_command, tmux_attach_command};
use crate::terminal::model::session::shell_quote_arg;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeResumeTarget {
    pub(crate) provider: AgentProvider,
    pub(crate) session_id: String,
    pub(crate) worktree: PathBuf,
}

impl NativeResumeTarget {
    pub(crate) fn argv(&self) -> Option<Vec<String>> {
        match self.provider {
            AgentProvider::ClaudeCode => Some(vec![
                "claude".to_string(),
                "--resume".to_string(),
                self.session_id.clone(),
            ]),
            AgentProvider::Codex => Some(vec![
                "codex".to_string(),
                "resume".to_string(),
                self.session_id.clone(),
            ]),
            AgentProvider::Herdr | AgentProvider::Cli(_) | AgentProvider::Unknown => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeAgentLaunch {
    OpenDirectory(PathBuf),
    AttachTmux {
        worktree: PathBuf,
        target: TmuxAttachTarget,
    },
    AttachHerdr {
        worktree: PathBuf,
        target: String,
    },
    NativeResume(NativeResumeTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurrentPaneLaunchResult {
    Launched,
    Unsupported,
}

impl WorktreeAgentLaunch {
    pub(crate) fn worktree(&self) -> &Path {
        match self {
            Self::OpenDirectory(worktree)
            | Self::AttachTmux { worktree, .. }
            | Self::AttachHerdr { worktree, .. } => worktree,
            Self::NativeResume(target) => &target.worktree,
        }
    }

    pub(crate) fn command(&self, shell_type: ShellType) -> Option<String> {
        match self {
            Self::OpenDirectory(_) => None,
            Self::AttachTmux { target, .. } => Some(tmux_attach_command(target, shell_type)),
            Self::AttachHerdr { target, .. } => Some(herdr_attach_command(target, shell_type)),
            Self::NativeResume(target) => native_resume_command(target, shell_type),
        }
    }

    pub(crate) fn prefers_current_pane(&self) -> bool {
        matches!(
            self,
            Self::AttachTmux { .. } | Self::AttachHerdr { .. } | Self::NativeResume(_)
        )
    }
}

fn native_resume_command(target: &NativeResumeTarget, shell_type: ShellType) -> Option<String> {
    let argv = target.argv()?;
    let program = argv.first()?;
    let command = argv
        .iter()
        .map(|argument| shell_quote_arg(argument, shell_type))
        .collect::<Vec<_>>()
        .join(" ");
    let diagnostic = shell_quote_arg(
        &format!("Warpeer: {program} is unavailable; exact session was not resumed."),
        shell_type,
    );
    let worktree = shell_quote_arg(&target.worktree.to_string_lossy(), shell_type);
    Some(format!(
        "cd {worktree} && command -v {program} >/dev/null 2>&1 && exec {command}; printf '%s\\n' {diagnostic} >&2; false"
    ))
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
