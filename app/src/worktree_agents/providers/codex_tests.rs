use serde_json::json;

use super::*;

#[test]
fn parses_active_thread_without_retaining_preview_or_turns() {
    let agent = parse_thread_read(json!({
        "thread": {
            "id": "0198-thread",
            "cwd": "/work/project",
            "updatedAt": 1700000100,
            "preview": "must not be retained",
            "name": "dashboard",
            "status": {"type": "active", "activeFlags": []},
            "source": {"type": "appServer"},
            "turns": [{"items": [{"type": "userMessage", "text": "secret"}]}]
        }
    }))
    .unwrap();

    assert_eq!(agent.session_id, "0198-thread");
    assert_eq!(agent.cwd, "/work/project");
    assert_eq!(agent.lifecycle, AgentLifecycle::Working);
    assert_eq!(agent.label.as_deref(), Some("dashboard"));
    assert_eq!(agent.sequence, Some(1700000100));
}

#[test]
fn maps_waiting_and_idle_states() {
    let waiting = parse_thread_read(json!({
        "thread": {
            "id": "waiting",
            "cwd": "/work",
            "status": {"type": "active", "activeFlags": ["waitingOnApproval"]}
        }
    }))
    .unwrap();
    let idle = parse_thread_read(json!({
        "thread": {
            "id": "idle",
            "cwd": "/work",
            "status": {"type": "idle"}
        }
    }))
    .unwrap();

    assert_eq!(waiting.lifecycle, AgentLifecycle::NeedsInput);
    assert_eq!(idle.lifecycle, AgentLifecycle::ReadyForReview);
}

#[test]
fn rejects_identityless_thread() {
    assert!(
        parse_thread_read(json!({
            "thread": {"id": "", "cwd": "/work", "status": {"type": "idle"}}
        }))
        .is_none()
    );
}

#[test]
fn cached_app_server_threads_are_reused_until_refresh_is_due() {
    let base = Instant::now();
    let mut state = CodexSessionCollectorState {
        threads: vec![RawThread {
            id: "session-1".to_string(),
            cwd: "/work".to_string(),
            parent_thread_id: None,
            agent_nickname: None,
            agent_role: None,
            name: Some("Task".to_string()),
            title: None,
            first_user_message: String::new(),
            preview: String::new(),
            path: Some("/tmp/rollout-session-1.jsonl".to_string()),
            updated_at: Some(1),
            status: json!({"type": "active", "activeFlags": []}),
        }],
        last_refresh: Some(base),
        last_attempt: Some(base),
        ..Default::default()
    };
    let hints = CodexActiveHints {
        session_ids: HashSet::from(["session-1".to_string()]),
        rollout_paths: HashSet::from(["/tmp/rollout-session-1.jsonl".to_string()]),
    };

    assert!(!state.should_refresh(base + Duration::from_secs(20), &hints));
    assert!(state.should_refresh(base + Duration::from_secs(31), &hints));

    state.last_attempt = Some(base);
    state.last_refresh = Some(base);
    let unknown = CodexActiveHints {
        session_ids: HashSet::from(["session-2".to_string()]),
        rollout_paths: HashSet::new(),
    };
    assert!(state.should_refresh(base + Duration::from_secs(13), &unknown));
}

#[test]
fn stable_thread_title_wins_over_preview_and_rollout_fallbacks() {
    let agent = parse_thread_read(json!({
        "thread": {
            "id": "stable-title",
            "cwd": "/work/project",
            "title": "Fix right-click lockup",
            "firstUserMessage": "Older first message",
            "preview": "Changing preview",
            "status": {"type": "idle"}
        }
    }))
    .unwrap();

    assert_eq!(agent.label.as_deref(), Some("Fix right-click lockup"));
}

#[test]
fn first_user_message_is_used_when_codex_has_no_title() {
    let agent = parse_thread_read(json!({
        "thread": {
            "id": "first-message",
            "cwd": "/work/project",
            "firstUserMessage": "Initial task sentence",
            "preview": "Later preview",
            "status": {"type": "idle"}
        }
    }))
    .unwrap();

    assert_eq!(agent.label.as_deref(), Some("Initial task sentence"));
}

#[test]
fn preserves_structured_subagent_parent_and_role() {
    let agent = parse_thread_read(json!({
        "thread": {
            "id": "child-thread",
            "cwd": "/work/project",
            "parentThreadId": "parent-thread",
            "agentNickname": "Atlas",
            "agentRole": "implementation",
            "status": {"type": "active", "activeFlags": []}
        }
    }))
    .unwrap();

    let relation = agent.relation.unwrap();
    assert_eq!(relation.parent_session_id, "parent-thread");
    assert_eq!(
        relation.role.as_ref().map(|role| role.as_str()),
        Some("implementation")
    );
}
