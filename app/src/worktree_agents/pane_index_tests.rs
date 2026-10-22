use super::*;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey};
use crate::worktree_agents::providers::AgentProvider;

fn worktree(path: &str) -> WorktreeKey {
    WorktreeKey::new(
        RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
        path,
    )
}

fn agent(worktree: WorktreeKey, session_id: &str) -> AgentKey {
    AgentKey {
        worktree,
        provider: AgentProvider::ClaudeCode,
        session_id: session_id.to_string(),
    }
}

#[test]
fn correlation_is_invalidated_when_pane_changes_worktrees() {
    let mut index = PaneRouteIndex::default();
    let terminal = EntityId::new();
    let first = worktree("/repo/one");
    let first_agent = agent(first.clone(), "one");
    let correlation = index.publish_worktree(terminal, first);
    index.bind_agent(terminal, first_agent.clone()).unwrap();

    index.publish_worktree(terminal, worktree("/repo/two"));

    assert!(!index.validate_agent(&first_agent, terminal, correlation));
}

#[test]
fn worktree_lookup_never_returns_the_source_pane() {
    let mut index = PaneRouteIndex::default();
    let source = EntityId::new();
    let other = EntityId::new();
    let target = worktree("/repo/one");
    index.publish_worktree(source, target.clone());
    assert_eq!(index.pane_for_worktree(source, &target), None);

    index.publish_worktree(other, target.clone());
    assert_eq!(index.pane_for_worktree(source, &target), Some(other));
}

#[test]
fn token_mismatch_cannot_validate_an_existing_pane_route() {
    let mut index = PaneRouteIndex::default();
    let first_terminal = EntityId::new();
    let second_terminal = EntityId::new();
    let target = worktree("/repo/one");
    let key = agent(target.clone(), "one");
    let first = index.publish_worktree(first_terminal, target.clone());
    let second = index.publish_worktree(second_terminal, target);
    index.bind_agent(first_terminal, key.clone()).unwrap();

    assert!(index.validate_agent(&key, first_terminal, first));
    assert!(!index.validate_agent(&key, first_terminal, second));
}

#[test]
fn agent_binding_survives_transition_to_its_sibling_worktree() {
    let mut index = PaneRouteIndex::default();
    let terminal = EntityId::new();
    let current = worktree("/repo/current");
    let target = worktree("/repo/target");
    let key = agent(target.clone(), "target-agent");
    index.publish_worktree(terminal, current);

    let correlation = index.bind_agent(terminal, key.clone()).unwrap();
    index.publish_worktree(terminal, target);

    assert!(index.validate_agent(&key, terminal, correlation));
}

#[test]
fn agent_binding_rejects_an_unrelated_repository() {
    let mut index = PaneRouteIndex::default();
    let terminal = EntityId::new();
    index.publish_worktree(terminal, worktree("/repo/current"));
    let unrelated = AgentKey {
        worktree: WorktreeKey::new(
            RepositoryKey::new(ExecutionHostKey::Local, "/other/.git"),
            "/other/worktree",
        ),
        provider: AgentProvider::ClaudeCode,
        session_id: "other-agent".to_string(),
    };

    assert_eq!(index.bind_agent(terminal, unrelated), None);
}
