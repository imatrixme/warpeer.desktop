use super::*;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};
use crate::worktree_agents::launch::NativeResumeTarget;
use crate::worktree_agents::observation::AgentKey;
use crate::worktree_agents::pane_index::LaunchCorrelation;
use crate::worktree_agents::providers::AgentProvider;

fn key() -> AgentKey {
    AgentKey {
        worktree: WorktreeKey::new(
            RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git"),
            "/repo",
        ),
        provider: AgentProvider::ClaudeCode,
        session_id: "session-1".to_string(),
    }
}

fn pane_route() -> AgentRoute {
    AgentRoute::ExistingPane {
        terminal_view_id: warpui::EntityId::new(),
        correlation: LaunchCorrelation(1),
    }
}

#[test]
fn fresh_exact_route_rejects_inferred_replacement() {
    let mut registry = RouteRegistry::default();
    let route = pane_route();
    let generation = registry
        .publish(
            key(),
            "pane",
            route.clone(),
            RouteConfidence::Exact,
            10,
            100,
        )
        .unwrap();

    assert_eq!(
        registry.publish(
            key(),
            "inference",
            AgentRoute::ObserveOnly,
            RouteConfidence::Inferred,
            20,
            120,
        ),
        None
    );
    assert_eq!(registry.resolve_exact(&key(), generation, 30), Some(&route));
}

#[test]
fn stale_generation_or_expired_route_cannot_execute() {
    let mut registry = RouteRegistry::default();
    let first = registry
        .publish(key(), "pane", pane_route(), RouteConfidence::Exact, 10, 100)
        .unwrap();
    let second = registry
        .publish(
            key(),
            "pane",
            AgentRoute::ObserveOnly,
            RouteConfidence::Exact,
            20,
            200,
        )
        .unwrap();

    assert_eq!(registry.resolve_exact(&key(), first, 30), None);
    assert_eq!(registry.resolve_exact(&key(), second, 200), None);
}

#[test]
fn inferred_route_can_render_but_cannot_execute() {
    let mut registry = RouteRegistry::default();
    let route = pane_route();
    let generation = registry
        .publish(
            key(),
            "inference",
            route.clone(),
            RouteConfidence::Inferred,
            10,
            100,
        )
        .unwrap();

    assert_eq!(
        registry.get(&key()).map(|record| &record.route),
        Some(&route)
    );
    assert_eq!(registry.resolve_exact(&key(), generation, 20), None);
}

#[test]
fn removing_one_route_source_reveals_the_other_source() {
    let mut registry = RouteRegistry::default();
    let inferred = pane_route();
    assert!(
        registry
            .publish(
                key(),
                "inference",
                inferred.clone(),
                RouteConfidence::Inferred,
                10,
                200,
            )
            .is_some()
    );
    let exact = AgentRoute::ObserveOnly;
    let exact_generation = registry
        .publish(
            key(),
            "herdr",
            exact.clone(),
            RouteConfidence::Exact,
            20,
            200,
        )
        .unwrap();
    assert_eq!(
        registry.resolve_exact(&key(), exact_generation, 30),
        Some(&exact)
    );

    assert!(registry.remove_source(&key(), "herdr"));
    assert_eq!(registry.get(&key()).unwrap().route, inferred);
}

#[test]
fn identical_route_refresh_keeps_generation_stable() {
    let mut registry = RouteRegistry::default();
    let route = pane_route();
    let generation = registry
        .publish(
            key(),
            "pane",
            route.clone(),
            RouteConfidence::Exact,
            10,
            100,
        )
        .unwrap();
    assert_eq!(
        registry.publish(
            key(),
            "pane",
            route.clone(),
            RouteConfidence::Exact,
            20,
            200,
        ),
        None
    );
    assert_eq!(
        registry.resolve_exact(&key(), generation, 150),
        Some(&route)
    );
}

#[test]
fn existing_warpeer_pane_outranks_runtime_attach_route() {
    let mut registry = RouteRegistry::default();
    let pane = pane_route();
    registry.publish(key(), "pane", pane.clone(), RouteConfidence::Exact, 10, 200);
    registry.publish(
        key(),
        "herdr",
        AgentRoute::Herdr {
            session: "term-1".to_string(),
            workspace: Some("w1".to_string()),
            tab: "w1:t1".to_string(),
            pane: "w1:p1".to_string(),
            agent: "term-1".to_string(),
        },
        RouteConfidence::Exact,
        20,
        200,
    );

    assert_eq!(registry.get(&key()).unwrap().route, pane);
}

#[test]
fn removing_pane_activation_reveals_runtime_attach_route() {
    let mut registry = RouteRegistry::default();
    let tmux = AgentRoute::Tmux {
        socket: Some("/tmp/tmux.sock".to_string()),
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
    };
    registry.publish(key(), "tmux", tmux.clone(), RouteConfidence::Exact, 10, 200);
    registry.publish(
        key(),
        "warpeer-pane-activation",
        pane_route(),
        RouteConfidence::Exact,
        20,
        u64::MAX,
    );

    assert!(registry.remove_source(&key(), "warpeer-pane-activation"));
    assert_eq!(registry.get(&key()).unwrap().route, tmux);
}

#[test]
fn runtime_attach_outranks_native_resume_fallback() {
    let mut registry = RouteRegistry::default();
    registry.publish(
        key(),
        "resume",
        AgentRoute::NativeResume(NativeResumeTarget {
            provider: AgentProvider::ClaudeCode,
            session_id: "session-1".to_string(),
            worktree: "/repo".into(),
        }),
        RouteConfidence::Exact,
        10,
        200,
    );
    let tmux = AgentRoute::Tmux {
        socket: Some("/tmp/tmux.sock".to_string()),
        session: "dev".to_string(),
        window: "@1".to_string(),
        pane: "%2".to_string(),
    };
    registry.publish(key(), "tmux", tmux.clone(), RouteConfidence::Exact, 20, 200);

    assert_eq!(registry.get(&key()).unwrap().route, tmux);
}
