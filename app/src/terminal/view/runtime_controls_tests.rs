use super::*;

#[test]
fn runtime_control_commands_quote_session_names() {
    assert_eq!(
        RuntimeOperation::SplitRight.command(RuntimeKind::Tmux, "work tree", ShellType::Bash),
        "tmux split-window -h -t 'work tree':"
    );
    assert_eq!(
        RuntimeOperation::ToggleZoom.command(RuntimeKind::Herdr, "work;tree", ShellType::Bash,),
        "herdr --session 'work;tree' pane zoom --current --toggle"
    );
}
