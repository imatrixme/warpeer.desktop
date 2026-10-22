use super::*;

const SNAPSHOT: &str = r#"
{
  "result": {
    "snapshot": {
      "version": "0.8.0",
      "protocol": 19,
      "agents": [
        {
          "terminal_id": "term-agent",
          "workspace_id": "w3",
          "tab_id": "w3:t2",
          "pane_id": "w3:p4",
          "agent_status": "blocked",
          "agent": "claude-code",
          "display_agent": "Claude Code",
          "name": "Reviewer",
          "cwd": "/repo/feature",
          "foreground_cwd": "/repo/feature/src",
          "agent_session": {
            "source": "pane_report",
            "agent": "claude-code",
            "kind": "id",
            "value": "claude-session-1"
          },
          "revision": 4,
          "state_change_seq": 9,
          "focused": false
        }
      ],
      "panes": [
        {
          "terminal_id": "term-agent",
          "workspace_id": "w3",
          "tab_id": "w3:t2",
          "pane_id": "w3:p4",
          "agent_status": "working",
          "agent": "claude-code",
          "cwd": "/repo/feature",
          "revision": 3,
          "focused": false
        },
        {
          "terminal_id": "term-shell",
          "workspace_id": "w3",
          "tab_id": "w3:t1",
          "pane_id": "w3:p1",
          "agent_status": "unknown",
          "cwd": "/repo",
          "revision": 1,
          "focused": true
        }
      ]
    }
  }
}
"#;

#[test]
fn structured_agent_inventory_wins_over_pane_fallback() {
    let snapshot = parse_snapshot(SNAPSHOT.as_bytes()).unwrap();
    assert_eq!(snapshot.version, "0.8.0");
    assert_eq!(snapshot.protocol, 19);
    assert!(snapshot.supports_exact_agent_routes());
    assert_eq!(snapshot.agents.len(), 1);

    let agent = &snapshot.agents[0];
    assert_eq!(agent.provider, AgentProvider::ClaudeCode);
    assert_eq!(agent.provider_session_id, "claude-session-1");
    assert_eq!(agent.cwd.as_deref(), Some("/repo/feature/src"));
    assert_eq!(agent.lifecycle, AgentLifecycle::NeedsInput);
    assert_eq!(agent.sequence, 9);
    assert_eq!(agent.terminal_id, "term-agent");
}

#[test]
fn older_protocol_remains_observable_without_exact_agent_routes() {
    let snapshot = parse_snapshot(
        SNAPSHOT
            .replace("\"protocol\": 19", "\"protocol\": 18")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(snapshot.agents.len(), 1);
    assert!(!snapshot.supports_exact_agent_routes());
}

#[test]
fn pane_report_can_supply_a_bounded_observation_without_terminal_output() {
    let snapshot = parse_snapshot(
        br#"{
          "result": {"snapshot": {
            "version": "0.8.0", "protocol": 19, "agents": [],
            "panes": [{
              "terminal_id": "term-codex", "workspace_id": "w1",
              "tab_id": "w1:t1", "pane_id": "w1:p1", "agent_status": "working",
              "agent": "codex", "cwd": "/repo", "revision": 7, "focused": true,
              "screen": "sensitive terminal output that is ignored"
            }]
          }}
        }"#,
    )
    .unwrap();
    assert_eq!(snapshot.agents[0].provider, AgentProvider::Codex);
    assert_eq!(snapshot.agents[0].lifecycle, AgentLifecycle::Working);
    assert_eq!(snapshot.agents[0].provider_session_id, "herdr:term-codex");
}

#[test]
fn unknown_shell_panes_are_not_reported_as_agents() {
    let snapshot = parse_snapshot(
        br#"{
          "result": {"snapshot": {
            "version": "0.8.0", "protocol": 19, "agents": [],
            "panes": [{
              "terminal_id": "term-shell", "workspace_id": "w1",
              "tab_id": "w1:t1", "pane_id": "w1:p1", "agent_status": "unknown",
              "cwd": "/repo", "revision": 1, "focused": true
            }]
          }}
        }"#,
    )
    .unwrap();
    assert!(snapshot.agents.is_empty());
}

#[test]
fn provider_names_preserve_other_supported_cli_agents() {
    use crate::terminal::CLIAgent;

    assert_eq!(
        provider_from_name("Gemini CLI"),
        AgentProvider::Cli(CLIAgent::Gemini)
    );
    assert_eq!(
        provider_from_name("OpenCode"),
        AgentProvider::Cli(CLIAgent::OpenCode)
    );
}
