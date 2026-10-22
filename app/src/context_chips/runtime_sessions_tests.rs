use warp_terminal::shell::ShellType;

use super::*;
use crate::context_chips::{
    ChipAvailability, ChipRuntimeCapabilities, ExternalCommandsAvailability,
};

#[test]
fn parses_tmux_session_rows() {
    let sessions = parse_tmux_sessions("dev|2|1\nidle|1|0\n");

    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].session_name(), "dev");
    assert_eq!(sessions[0].name(), "dev - 2 windows, attached");
    assert_eq!(sessions[1].name(), "idle - 1 window, detached");
}

#[test]
fn parses_herdr_session_json() {
    let sessions =
        parse_herdr_sessions(r#"{"sessions":[{"default":true,"name":"default","running":true}]}"#);

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_name(), "default");
    assert_eq!(sessions[0].name(), "default - running, default");
}

#[test]
fn extracts_attached_runtime_session_names() {
    assert_eq!(
        RuntimeKind::Tmux.session_name_from_command("tmux attach-session -t 'work tree'"),
        Some("work tree".to_string())
    );
    assert_eq!(
        RuntimeKind::Herdr.session_name_from_command("herdr session attach default"),
        Some("default".to_string())
    );
    assert_eq!(
        RuntimeKind::Herdr.session_name_from_command("herdr --session=work pane list"),
        Some("work".to_string())
    );
}

#[test]
fn runtime_chips_are_available_in_remote_sessions() {
    let capabilities = ChipRuntimeCapabilities {
        session_id: None,
        session_is_local: Some(false),
        external_commands: ExternalCommandsAvailability::Unknown,
    };

    assert_eq!(
        context_chip().availability(&capabilities),
        ChipAvailability::Enabled
    );
}

#[test]
fn all_filter_labels_each_session_with_its_runtime() {
    let mut sessions = parse_tmux_sessions("dev|2|1\n");
    sessions.extend(parse_herdr_sessions(
        r#"{"sessions":[{"name":"default","running":true}]}"#,
    ));

    let filtered = filter_runtime_sessions(&sessions, RuntimeSessionFilter::All);

    assert_eq!(filtered[0].name(), "tmux - dev - 2 windows, attached");
    assert_eq!(filtered[1].name(), "Herdr - default - running");
}

#[test]
fn runtime_filter_only_keeps_matching_sessions() {
    let mut sessions = parse_tmux_sessions("dev|2|1\n");
    sessions.extend(parse_herdr_sessions(
        r#"{"sessions":[{"name":"default","running":true}]}"#,
    ));

    let filtered = filter_runtime_sessions(&sessions, RuntimeSessionFilter::Herdr);

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].runtime(), RuntimeKind::Herdr);
    assert_eq!(filtered[0].name(), "default - running");
}

#[test]
fn tabs_only_include_all_when_both_runtimes_are_available() {
    assert_eq!(
        runtime_filters_for(&[RuntimeKind::Tmux, RuntimeKind::Herdr]),
        vec![
            RuntimeSessionFilter::All,
            RuntimeSessionFilter::Tmux,
            RuntimeSessionFilter::Herdr,
        ]
    );
    assert_eq!(
        runtime_filters_for(&[RuntimeKind::Tmux]),
        vec![RuntimeSessionFilter::Tmux]
    );
}

#[test]
fn runtime_termination_commands_quote_session_names() {
    let tmux = parse_tmux_sessions("work tree|1|0\n").remove(0);
    assert_eq!(
        tmux.termination_command(ShellType::Bash),
        "tmux kill-session -t 'work tree'"
    );

    let running_herdr =
        parse_herdr_sessions(r#"{"sessions":[{"name":"work;tree","running":true}]}"#).remove(0);
    assert_eq!(
        running_herdr.termination_command(ShellType::Bash),
        "herdr session stop --json 'work;tree' && herdr session delete --json 'work;tree'"
    );

    let stopped_herdr =
        parse_herdr_sessions(r#"{"sessions":[{"name":"idle","running":false}]}"#).remove(0);
    assert_eq!(
        stopped_herdr.termination_command(ShellType::Bash),
        "herdr session delete --json 'idle'"
    );
}
