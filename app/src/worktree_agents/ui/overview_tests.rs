use super::*;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};
use crate::worktree_agents::observation::{
    AgentKey, AgentLifecycle, AgentMetrics, AgentRelation, BoundedStatusLabel, SourceAuthority,
    SourceHealth,
};
use crate::worktree_agents::projections::{AgentActivationKind, AgentRouteProjection};
use crate::worktree_agents::routing::RouteConfidence;
use crate::worktree_agents::runtimes::AgentRuntimeHint;

fn agent(session_id: &str, parent: Option<&str>, role: Option<&str>) -> AgentProjection {
    let worktree = WorktreeKey::new(
        RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
        "/repo",
    );
    AgentProjection {
        key: AgentKey {
            worktree,
            provider: AgentProvider::Codex,
            session_id: session_id.to_string(),
        },
        provider: AgentProvider::Codex,
        lifecycle: AgentLifecycle::Working,
        authority: SourceAuthority::ProviderSnapshot,
        source_health: SourceHealth::Healthy,
        runtime: AgentRuntimeHint::Unknown,
        relation: parent.map(|parent_session_id| AgentRelation {
            parent_session_id: parent_session_id.to_string(),
            role: role.and_then(BoundedStatusLabel::new),
        }),
        label: None,
        metrics: AgentMetrics::default(),
        observed_at_ms: 1,
        expires_at_ms: 100,
        current_pane: false,
        route: Some(AgentRouteProjection {
            kind: AgentActivationKind::ResumeAgent,
            confidence: RouteConfidence::Exact,
            generation: 1,
        }),
    }
}

#[test]
fn subagents_follow_their_parent_with_nested_depth() {
    let rows = agent_rows(vec![
        agent("grandchild", Some("child"), Some("test")),
        agent("sibling", Some("parent"), Some("review")),
        agent("parent", None, None),
        agent("child", Some("parent"), Some("implementation")),
    ]);
    let identities = rows
        .iter()
        .map(|(agent, depth)| (agent.key.session_id.as_str(), *depth))
        .collect::<Vec<_>>();

    assert_eq!(
        identities,
        vec![
            ("parent", 0),
            ("sibling", 1),
            ("child", 1),
            ("grandchild", 2),
        ]
    );
}

#[test]
fn orphaned_subagents_remain_accessible_as_primary_rows() {
    let rows = agent_rows(vec![agent("child", Some("missing"), Some("review"))]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.key.session_id, "child");
    assert_eq!(rows[0].1, 0);
}
