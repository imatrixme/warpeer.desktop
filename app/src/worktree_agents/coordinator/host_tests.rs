use super::*;

#[test]
fn inferred_process_keeps_its_exact_tmux_target() {
    let inferred = InferredTmuxAgent {
        provider: AgentProvider::Codex,
        process_id: 42,
        session_id: None,
        rollout_path: None,
        pane: TmuxPaneRecord {
            socket: Some("/tmp/tmux.sock".to_string()),
            session: "agents".to_string(),
            window: "@3".to_string(),
            pane: "%7".to_string(),
            pane_pid: 100,
            cwd: "/repo/worktree".to_string(),
            command: "codex".to_string(),
            attached: false,
        },
    };

    assert_eq!(
        inferred_tmux_route(&inferred),
        AgentRoute::Tmux {
            socket: Some("/tmp/tmux.sock".to_string()),
            session: "agents".to_string(),
            window: "@3".to_string(),
            pane: "%7".to_string(),
        }
    );
}
