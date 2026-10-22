use super::*;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey};
use crate::worktree_agents::observation::{AgentLifecycle, ObservationStore};
use crate::worktree_agents::runtimes::AgentRuntimeHint;

fn worktree() -> WorktreeKey {
    WorktreeKey::new(
        RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
        "/repo",
    )
}

fn inferred(process_id: u32, session_id: Option<&str>) -> InferredTmuxAgent {
    InferredTmuxAgent {
        provider: AgentProvider::Codex,
        process_id,
        session_id: session_id.map(str::to_string),
        rollout_path: None,
        pane: TmuxPaneRecord {
            socket: Some("/tmp/tmux.sock".to_string()),
            session: "agents".to_string(),
            window: "@1".to_string(),
            pane: "%1".to_string(),
            pane_pid: 10,
            cwd: "/repo".to_string(),
            command: "node".to_string(),
            attached: true,
        },
    }
}

fn observation(
    authority: SourceAuthority,
    source_id: &'static str,
    label: Option<&str>,
    token_usage: Option<u64>,
    observed_at_ms: u64,
) -> AgentObservation {
    let label = label.and_then(BoundedStatusLabel::new);
    AgentObservation {
        key: AgentKey {
            worktree: worktree(),
            provider: AgentProvider::Codex,
            session_id: "session-1".to_string(),
        },
        lifecycle: AgentLifecycle::Working,
        authority,
        source_id,
        sequence: None,
        observed_at_ms,
        expires_at_ms: observed_at_ms + 100,
        runtime: AgentRuntimeHint::Tmux,
        relation: None,
        label: label.clone(),
        metrics: AgentMetrics {
            session_title: label,
            token_usage,
            ..Default::default()
        },
    }
}

#[test]
fn process_identity_survives_a_transient_session_probe_miss() {
    let mut collector = AgentSessionInfoCollector::default();
    let exact = collector.key_for_tmux_process(worktree(), &inferred(42, Some("session-1")));
    let recovered = collector.key_for_tmux_process(worktree(), &inferred(42, None));

    assert_eq!(exact, recovered);
}

#[test]
fn tmux_pane_identity_survives_a_wrapper_process_change() {
    let mut collector = AgentSessionInfoCollector::default();
    let exact = collector.key_for_tmux_process(worktree(), &inferred(42, Some("session-1")));
    let recovered = collector.key_for_tmux_process(worktree(), &inferred(84, None));

    assert_eq!(exact, recovered);
}

#[test]
fn process_fallback_keeps_the_last_structured_title_and_metrics() {
    let mut collector = AgentSessionInfoCollector::default();
    let mut observations = ObservationStore::default();
    let structured = collector.collect(observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Fix tmux attachment"),
        Some(12_000),
        10,
    ));
    let key = structured.key.clone();
    observations.apply(structured);
    observations.apply(collector.collect(observation(
        SourceAuthority::ProcessInference,
        "process",
        None,
        None,
        20,
    )));

    observations.remove_source(&key, "structured");

    let fallback = observations.get(&key).unwrap();
    assert_eq!(
        fallback
            .metrics
            .session_title
            .as_ref()
            .map(|title| title.as_str()),
        Some("Fix tmux attachment")
    );
    assert_eq!(fallback.metrics.token_usage, Some(12_000));
}

#[test]
fn process_fallback_keeps_the_structured_parent_relation() {
    let mut collector = AgentSessionInfoCollector::default();
    let mut structured = observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Review implementation"),
        None,
        10,
    );
    structured.relation = Some(AgentRelation {
        parent_session_id: "parent-session".to_string(),
        role: BoundedStatusLabel::new("review"),
    });
    collector.collect(structured);

    let fallback = collector.collect(observation(
        SourceAuthority::ProcessInference,
        "process",
        None,
        None,
        20,
    ));

    let relation = fallback.relation.unwrap();
    assert_eq!(relation.parent_session_id, "parent-session");
    assert_eq!(
        relation.role.as_ref().map(|role| role.as_str()),
        Some("review")
    );
}

#[test]
fn metric_history_ignores_nearby_duplicates() {
    let mut collector = AgentSessionInfoCollector::default();
    collector.collect(observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Task"),
        Some(100),
        10,
    ));
    collector.collect(observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Task"),
        Some(100),
        20,
    ));

    assert_eq!(
        collector
            .sessions
            .values()
            .next()
            .unwrap()
            .metric_samples
            .len(),
        1
    );
}

#[test]
fn lower_authority_snapshot_does_not_replace_structured_metadata() {
    let mut collector = AgentSessionInfoCollector::default();
    collector.collect(observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Fix agent metadata stability"),
        Some(42_000),
        10,
    ));

    let fallback = collector.collect(observation(
        SourceAuthority::ProviderSnapshot,
        "snapshot",
        Some("Codex"),
        None,
        20,
    ));

    assert_eq!(
        fallback
            .metrics
            .session_title
            .as_ref()
            .map(|title| title.as_str()),
        Some("Fix agent metadata stability")
    );
    assert_eq!(fallback.metrics.token_usage, Some(42_000));
}

#[test]
fn process_fallback_reuses_a_recent_structured_lifecycle() {
    let mut collector = AgentSessionInfoCollector::default();
    let mut structured = observation(
        SourceAuthority::StructuredEvent,
        "structured",
        Some("Task"),
        Some(100),
        10,
    );
    structured.lifecycle = AgentLifecycle::NeedsInput;
    collector.collect(structured);

    let mut process = observation(SourceAuthority::ProcessInference, "process", None, None, 20);
    process.lifecycle = AgentLifecycle::Detected;

    assert_eq!(
        collector.collect(process).lifecycle,
        AgentLifecycle::NeedsInput
    );
}
