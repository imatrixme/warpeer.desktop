use warp_terminal::shell::ShellType;

use super::{TmuxAttachTarget, herdr_attach_command, herdr_focus_command, tmux_attach_command};

#[test]
fn herdr_agent_target_is_shell_quoted() {
    assert_eq!(
        herdr_attach_command("agent; touch /tmp/proof", ShellType::Bash),
        "herdr agent get 'agent; touch /tmp/proof' >/dev/null && exec herdr agent attach 'agent; touch /tmp/proof'"
    );
}

#[test]
fn herdr_focus_does_not_start_a_nested_runtime() {
    assert_eq!(
        herdr_focus_command("agent; touch /tmp/proof", ShellType::Bash),
        "herdr agent get 'agent; touch /tmp/proof' >/dev/null && herdr agent focus 'agent; touch /tmp/proof'"
    );
}

#[test]
fn tmux_attach_revalidates_and_quotes_every_target_field() {
    let command = tmux_attach_command(
        &TmuxAttachTarget {
            socket: Some("/tmp/tmux; touch /tmp/socket-proof".to_string()),
            session: "dev; touch /tmp/session-proof".to_string(),
            window: "@1; touch /tmp/window-proof".to_string(),
            pane: "%2; touch /tmp/pane-proof".to_string(),
        },
        ShellType::Bash,
    );

    assert_eq!(
        command,
        "tmux -S '/tmp/tmux; touch /tmp/socket-proof' display-message -p -t '%2; touch /tmp/pane-proof' '#{pane_id}' >/dev/null && exec tmux -S '/tmp/tmux; touch /tmp/socket-proof' select-window -t '@1; touch /tmp/window-proof' \\; select-pane -t '%2; touch /tmp/pane-proof' \\; attach-session -t 'dev; touch /tmp/session-proof'"
    );
}
