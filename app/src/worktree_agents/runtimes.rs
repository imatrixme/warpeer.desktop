use warp_terminal::shell::ShellType;

use crate::terminal::model::session::shell_quote_arg;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AgentRuntimeHint {
    NormalPty,
    Tmux,
    Herdr,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TmuxAttachTarget {
    pub(crate) socket: Option<String>,
    pub(crate) session: String,
    pub(crate) window: String,
    pub(crate) pane: String,
}

pub(crate) fn herdr_attach_command(target: &str, shell_type: ShellType) -> String {
    let target = shell_quote_arg(target, shell_type);
    format!("herdr agent get {target} >/dev/null && exec herdr agent attach {target}")
}

pub(crate) fn herdr_focus_command(target: &str, shell_type: ShellType) -> String {
    let target = shell_quote_arg(target, shell_type);
    format!("herdr agent get {target} >/dev/null && herdr agent focus {target}")
}

pub(crate) fn tmux_attach_command(target: &TmuxAttachTarget, shell_type: ShellType) -> String {
    let prefix = target
        .socket
        .as_deref()
        .map(|socket| format!("tmux -S {}", shell_quote_arg(socket, shell_type)))
        .unwrap_or_else(|| "tmux".to_string());
    let pane = shell_quote_arg(&target.pane, shell_type);
    let window = shell_quote_arg(&target.window, shell_type);
    let session = shell_quote_arg(&target.session, shell_type);
    format!(
        "{prefix} display-message -p -t {pane} '#{{pane_id}}' >/dev/null && exec {prefix} select-window -t {window} \\; select-pane -t {pane} \\; attach-session -t {session}"
    )
}

#[cfg(test)]
#[path = "runtimes_tests.rs"]
mod tests;
