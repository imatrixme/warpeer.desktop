use super::*;

#[test]
fn cli_status_mapping_does_not_use_provider_payload_text() {
    assert_eq!(
        lifecycle_from_cli_status(&CLIAgentSessionStatus::Blocked {
            message: Some("sensitive tool input".to_string()),
        }),
        AgentLifecycle::NeedsInput
    );
    assert_eq!(lifecycle_label(AgentLifecycle::NeedsInput), "Needs input");
}

#[test]
fn supported_cli_agents_map_to_provider_neutral_ids() {
    assert_eq!(
        provider_from_cli_agent(CLIAgent::Claude),
        AgentProvider::ClaudeCode
    );
    assert_eq!(
        provider_from_cli_agent(CLIAgent::Codex),
        AgentProvider::Codex
    );
    assert_eq!(
        provider_from_cli_agent(CLIAgent::Gemini),
        AgentProvider::Cli(CLIAgent::Gemini)
    );
}

#[test]
fn remote_worktrees_do_not_accept_agent_observations() {
    let remote = WorktreeKey::new(
        RepositoryKey::new(ExecutionHostKey::Remote("host".to_string()), "/repo/.git"),
        "/repo",
    );
    let local = WorktreeKey::new(
        RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
        "/repo",
    );
    let authenticated_host = ExecutionHostKey::Remote("host".to_string());
    let wrong_host = ExecutionHostKey::Remote("other".to_string());
    assert!(!can_observe_agents(&remote, None));
    assert!(!can_observe_agents(&remote, Some(&wrong_host)));
    assert!(can_observe_agents(&remote, Some(&authenticated_host)));
    assert!(can_observe_agents(&local, None));
}
