use super::*;

#[test]
fn parses_current_claude_agent_shape_without_transcript_fields() {
    let agents = parse_snapshot(
        br#"[
          {
            "pid": 42,
            "cwd": "/work/repo",
            "kind": "background",
            "startedAt": 1784989094589,
            "sessionId": "14b62dfb-c56b-47c4-ad13-3f2963268463",
            "name": "manager",
            "status": "busy",
            "state": "working",
            "summary": "must not be retained"
          }
        ]"#,
    )
    .unwrap();

    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].session_id, "14b62dfb-c56b-47c4-ad13-3f2963268463");
    assert_eq!(agents[0].cwd, "/work/repo");
    assert_eq!(agents[0].lifecycle, AgentLifecycle::Working);
    assert_eq!(agents[0].label.as_deref(), Some("manager"));
    assert_eq!(agents[0].pid, Some(42));
    assert_eq!(agents[0].sequence, Some(1784989094589));
}

#[test]
fn maps_status_when_state_is_absent() {
    let agents =
        parse_snapshot(br#"[{"cwd":"/work","sessionId":"session-1","status":"blocked"}]"#).unwrap();

    assert_eq!(agents[0].lifecycle, AgentLifecycle::NeedsInput);
}

#[test]
fn maps_starting_and_thinking_lifecycle_states() {
    let agents = parse_snapshot(
        br#"[
          {"cwd":"/work","sessionId":"session-1","state":"starting"},
          {"cwd":"/work","sessionId":"session-2","status":"thinking"}
        ]"#,
    )
    .unwrap();

    assert_eq!(agents[0].lifecycle, AgentLifecycle::Starting);
    assert_eq!(agents[1].lifecycle, AgentLifecycle::Thinking);
}

#[test]
fn rejects_invalid_or_identityless_records() {
    assert!(parse_snapshot(b"not json").is_none());
    assert!(
        parse_snapshot(br#"[{"cwd":"/work","sessionId":""}]"#)
            .unwrap()
            .is_empty()
    );
}
