use super::*;

#[test]
fn current_tmux_socket_is_queried_in_addition_to_default() {
    assert_eq!(
        candidate_sockets(Some("/tmp/tmux-501/custom,123,0")),
        vec![None, Some("/tmp/tmux-501/custom".to_string())]
    );
}

#[test]
fn empty_tmux_environment_keeps_only_the_default_server() {
    assert_eq!(candidate_sockets(Some("")), vec![None]);
}

#[test]
fn pane_merge_deduplicates_the_same_socket_and_pane() {
    let record = TmuxPaneRecord {
        socket: Some("/tmp/tmux.sock".to_string()),
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
        pane_pid: 100,
        cwd: "/work".to_string(),
        command: "zsh".to_string(),
        attached: false,
    };
    let mut panes = vec![record.clone()];

    merge_panes(&mut panes, vec![record]);

    assert_eq!(panes.len(), 1);
}

#[test]
fn parses_exact_tmux_target_fields() {
    let panes =
        parse_panes(b"/tmp/tmux.sock\x1fdev one\x1f@3\x1f%7\x1f120\x1f/work/tree\x1fzsh\x1f1\n")
            .unwrap();

    assert_eq!(
        panes,
        vec![TmuxPaneRecord {
            socket: Some("/tmp/tmux.sock".to_string()),
            session: "dev one".to_string(),
            window: "@3".to_string(),
            pane: "%7".to_string(),
            pane_pid: 120,
            cwd: "/work/tree".to_string(),
            command: "zsh".to_string(),
            attached: true,
        }]
    );
}

#[test]
fn correlates_descendant_process_to_owning_pane() {
    let snapshot = TmuxProcessSnapshot {
        panes: vec![TmuxPaneRecord {
            socket: None,
            session: "dev".to_string(),
            window: "@1".to_string(),
            pane: "%2".to_string(),
            pane_pid: 100,
            cwd: "/work".to_string(),
            command: "zsh".to_string(),
            attached: false,
        }],
        processes: vec![
            ProcessRecord {
                pid: 100,
                parent_pid: 1,
                command: "zsh".to_string(),
            },
            ProcessRecord {
                pid: 110,
                parent_pid: 100,
                command: "claude".to_string(),
            },
            ProcessRecord {
                pid: 111,
                parent_pid: 110,
                command: "node".to_string(),
            },
        ],
        rollout_paths: HashMap::new(),
        session_ids: HashMap::new(),
    };

    assert_eq!(pane_for_process(&snapshot, 111), snapshot.panes.first());
    assert_eq!(pane_for_process(&snapshot, 999), None);
}

#[test]
fn ancestry_walk_is_bounded_on_cycles() {
    let snapshot = TmuxProcessSnapshot {
        panes: vec![TmuxPaneRecord {
            socket: None,
            session: "dev".to_string(),
            window: "@1".to_string(),
            pane: "%2".to_string(),
            pane_pid: 100,
            cwd: "/work".to_string(),
            command: "zsh".to_string(),
            attached: false,
        }],
        processes: vec![
            ProcessRecord {
                pid: 200,
                parent_pid: 201,
                command: "claude".to_string(),
            },
            ProcessRecord {
                pid: 201,
                parent_pid: 200,
                command: "node".to_string(),
            },
        ],
        rollout_paths: HashMap::new(),
        session_ids: HashMap::new(),
    };

    assert_eq!(pane_for_process(&snapshot, 200), None);
}

#[test]
fn process_inference_deduplicates_provider_per_pane() {
    let pane = TmuxPaneRecord {
        socket: None,
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
        pane_pid: 100,
        cwd: "/work".to_string(),
        command: "zsh".to_string(),
        attached: false,
    };
    let snapshot = TmuxProcessSnapshot {
        panes: vec![pane.clone()],
        processes: vec![
            ProcessRecord {
                pid: 100,
                parent_pid: 1,
                command: "zsh".to_string(),
            },
            ProcessRecord {
                pid: 110,
                parent_pid: 100,
                command: "/opt/homebrew/bin/claude".to_string(),
            },
            ProcessRecord {
                pid: 111,
                parent_pid: 110,
                command: "claude".to_string(),
            },
        ],
        rollout_paths: HashMap::new(),
        session_ids: HashMap::new(),
    };

    assert_eq!(
        infer_agents(&snapshot),
        vec![InferredTmuxAgent {
            provider: AgentProvider::ClaudeCode,
            process_id: 110,
            session_id: None,
            rollout_path: None,
            pane,
        }]
    );
}

#[test]
fn codex_resume_and_rollout_identity_are_preserved() {
    let pane = TmuxPaneRecord {
        socket: None,
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
        pane_pid: 100,
        cwd: "/work".to_string(),
        command: "zsh".to_string(),
        attached: true,
    };
    let snapshot = TmuxProcessSnapshot {
        panes: vec![pane.clone()],
        processes: vec![
            ProcessRecord {
                pid: 100,
                parent_pid: 1,
                command: "zsh".to_string(),
            },
            ProcessRecord {
                pid: 110,
                parent_pid: 100,
                command: "codex resume 0198-thread".to_string(),
            },
        ],
        rollout_paths: HashMap::from([(110, "/tmp/rollout-0198-thread.jsonl".to_string())]),
        session_ids: HashMap::new(),
    };

    assert_eq!(
        infer_agents(&snapshot),
        vec![InferredTmuxAgent {
            provider: AgentProvider::Codex,
            process_id: 110,
            session_id: Some("0198-thread".to_string()),
            rollout_path: Some("/tmp/rollout-0198-thread.jsonl".to_string()),
            pane,
        }]
    );
}

#[test]
fn recognizes_other_supported_cli_agent_processes() {
    use crate::terminal::CLIAgent;

    assert_eq!(
        provider_from_command("/opt/homebrew/bin/opencode"),
        Some(AgentProvider::Cli(CLIAgent::OpenCode))
    );
    assert_eq!(
        provider_from_command("node /usr/local/bin/gemini"),
        Some(AgentProvider::Cli(CLIAgent::Gemini))
    );
}

#[test]
fn parses_codex_artifacts_per_process() {
    let artifacts = parse_codex_artifacts(
        b"p110\nfcwd\nn/tmp/other\nn/Users/me/.codex/sessions/rollout-thread.jsonl\nn/Users/me/.codex/thread-writer-locks/thread.lock\np120\nn/tmp/rollout-second.jsonl\n",
    );

    assert_eq!(
        artifacts.rollout_paths,
        HashMap::from([
            (
                110,
                "/Users/me/.codex/sessions/rollout-thread.jsonl".to_string()
            ),
            (120, "/tmp/rollout-second.jsonl".to_string()),
        ])
    );
    assert_eq!(
        artifacts.session_ids,
        HashMap::from([(110, "thread".to_string())])
    );
}

#[test]
fn writer_lock_correlates_codex_without_resume_argument() {
    let pane = TmuxPaneRecord {
        socket: None,
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
        pane_pid: 100,
        cwd: "/work".to_string(),
        command: "zsh".to_string(),
        attached: true,
    };
    let snapshot = TmuxProcessSnapshot {
        panes: vec![pane.clone()],
        processes: vec![
            ProcessRecord {
                pid: 100,
                parent_pid: 1,
                command: "zsh".to_string(),
            },
            ProcessRecord {
                pid: 110,
                parent_pid: 100,
                command: "codex --yolo".to_string(),
            },
        ],
        rollout_paths: HashMap::new(),
        session_ids: HashMap::from([(110, "thread".to_string())]),
    };

    assert_eq!(
        infer_agents(&snapshot),
        vec![InferredTmuxAgent {
            provider: AgentProvider::Codex,
            process_id: 110,
            session_id: Some("thread".to_string()),
            rollout_path: None,
            pane,
        }]
    );
}
