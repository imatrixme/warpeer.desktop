use super::*;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};

fn key() -> AgentKey {
    AgentKey {
        worktree: WorktreeKey::new(
            RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
            "/repo",
        ),
        provider: AgentProvider::Codex,
        session_id: "thread-1".to_string(),
    }
}

fn observation(
    lifecycle: AgentLifecycle,
    authority: SourceAuthority,
    sequence: Option<u64>,
    observed_at_ms: u64,
) -> AgentObservation {
    AgentObservation {
        key: key(),
        lifecycle,
        authority,
        source_id: match authority {
            SourceAuthority::ProcessInference => "process",
            SourceAuthority::ProviderSnapshot => "snapshot",
            SourceAuthority::StructuredEvent => "event",
        },
        sequence,
        observed_at_ms,
        expires_at_ms: observed_at_ms + 100,
        runtime: AgentRuntimeHint::NormalPty,
        relation: None,
        label: None,
        metrics: AgentMetrics::default(),
    }
}

#[test]
fn structured_observation_outranks_fresh_process_inference() {
    let mut store = ObservationStore::default();
    assert!(store.apply(observation(
        AgentLifecycle::Thinking,
        SourceAuthority::StructuredEvent,
        Some(2),
        20,
    )));
    assert!(!store.apply(observation(
        AgentLifecycle::Working,
        SourceAuthority::ProcessInference,
        None,
        21,
    )));
    assert_eq!(
        store.get(&key()).unwrap().lifecycle,
        AgentLifecycle::Thinking
    );
}

#[test]
fn late_sequence_and_duplicate_are_suppressed() {
    let mut store = ObservationStore::default();
    let current = observation(
        AgentLifecycle::Working,
        SourceAuthority::StructuredEvent,
        Some(4),
        20,
    );
    assert!(store.apply(current.clone()));
    assert!(!store.apply(current));
    assert!(!store.apply(observation(
        AgentLifecycle::Thinking,
        SourceAuthority::StructuredEvent,
        Some(3),
        21,
    )));
}

#[test]
fn equal_sequence_snapshot_refreshes_freshness_without_changing_state() {
    let mut store = ObservationStore::default();
    let first = observation(
        AgentLifecycle::Working,
        SourceAuthority::ProviderSnapshot,
        Some(4),
        20,
    );
    assert!(store.apply(first));
    let refreshed = observation(
        AgentLifecycle::Working,
        SourceAuthority::ProviderSnapshot,
        Some(4),
        80,
    );
    assert!(!store.apply(refreshed));
    assert_eq!(store.get(&key()).unwrap().expires_at_ms, 180);
}

#[test]
fn unsequenced_process_snapshot_refreshes_freshness_without_repainting() {
    let mut store = ObservationStore::default();
    let first = observation(
        AgentLifecycle::Detected,
        SourceAuthority::ProcessInference,
        None,
        20,
    );
    assert!(store.apply(first));
    let refreshed = observation(
        AgentLifecycle::Detected,
        SourceAuthority::ProcessInference,
        None,
        80,
    );

    assert!(!store.apply(refreshed));
    assert_eq!(store.get(&key()).unwrap().expires_at_ms, 180);
}

#[test]
fn tombstone_rejects_late_reopen() {
    let mut store = ObservationStore::default();
    assert!(store.apply(observation(
        AgentLifecycle::Completed,
        SourceAuthority::StructuredEvent,
        Some(8),
        50,
    )));
    assert!(!store.apply(observation(
        AgentLifecycle::Working,
        SourceAuthority::ProviderSnapshot,
        Some(7),
        49,
    )));
    assert_eq!(
        store.get(&key()).unwrap().lifecycle,
        AgentLifecycle::Completed
    );
}

#[test]
fn expired_observation_becomes_stale() {
    let mut store = ObservationStore::default();
    store.apply(observation(
        AgentLifecycle::Working,
        SourceAuthority::ProviderSnapshot,
        None,
        10,
    ));

    assert_eq!(store.mark_expired(110), 1);
    assert_eq!(store.get(&key()).unwrap().lifecycle, AgentLifecycle::Stale);
}

#[test]
fn removing_one_source_reveals_the_other_healthy_source() {
    let mut store = ObservationStore::default();
    let mut snapshot = observation(
        AgentLifecycle::Working,
        SourceAuthority::ProviderSnapshot,
        Some(2),
        20,
    );
    snapshot.source_id = "herdr";
    assert!(store.apply(snapshot));

    let event = observation(
        AgentLifecycle::NeedsInput,
        SourceAuthority::StructuredEvent,
        Some(4),
        30,
    );
    assert!(store.apply(event));
    assert_eq!(
        store.get(&key()).unwrap().lifecycle,
        AgentLifecycle::NeedsInput
    );

    assert!(store.remove_source(&key(), "event"));
    assert_eq!(
        store.get(&key()).unwrap().lifecycle,
        AgentLifecycle::Working
    );
}

#[test]
fn status_labels_are_sanitized_and_bounded() {
    let label = BoundedStatusLabel::new(format!("thinking\n{}", "x".repeat(100))).unwrap();

    assert!(!label.as_str().contains('\n'));
    assert_eq!(label.as_str().chars().count(), 80);
}
