use super::*;
use crate::ui_components::icons::Icon;
use crate::worktree_agents::catalog::WorktreeDirtyState;
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};
use crate::worktree_agents::providers::AgentProvider;

#[test]
fn provider_icons_use_existing_brand_assets() {
    assert_eq!(provider_icon(AgentProvider::ClaudeCode), Icon::ClaudeLogo);
    assert_eq!(provider_icon(AgentProvider::Codex), Icon::OpenAILogo);
    assert_eq!(provider_icon(AgentProvider::Herdr), Icon::Herdr);
}

#[test]
fn only_live_exact_agent_routes_are_visible() {
    let pane_route = Some(AgentRouteProjection {
        kind: AgentActivationKind::FocusPane,
        confidence: RouteConfidence::Exact,
        generation: 1,
    });
    let resume_route = Some(AgentRouteProjection {
        kind: AgentActivationKind::ResumeAgent,
        confidence: RouteConfidence::Exact,
        generation: 2,
    });

    assert!(should_show_agent(
        AgentLifecycle::Working,
        pane_route,
        false
    ));
    assert!(!should_show_agent(
        AgentLifecycle::Completed,
        pane_route,
        false
    ));
    assert!(!should_show_agent(AgentLifecycle::Stale, pane_route, false));
    assert!(!should_show_agent(
        AgentLifecycle::Working,
        resume_route,
        false
    ));
    assert!(should_show_agent(
        AgentLifecycle::Working,
        resume_route,
        true
    ));
    assert!(!should_show_agent(AgentLifecycle::Working, None, true));
}

#[test]
fn hovered_worktree_scopes_the_overview_panel() {
    let repository = RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git");
    let projections = vec![
        projection(repository.clone(), "/repo/main", true),
        projection(repository, "/repo/feature", false),
    ];

    let selected = selected_worktree_projections(&projections, Some(&projections[1].key));
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].key, projections[1].key);

    let selected = selected_worktree_projections(&projections, None);
    assert_eq!(selected[0].key, projections[0].key);
}

fn projection(repository: RepositoryKey, path: &str, current: bool) -> WorktreeProjection {
    WorktreeProjection {
        key: WorktreeKey::new(repository, path),
        branch: None,
        detached_head: None,
        available: true,
        dirty: WorktreeDirtyState::Clean,
        current,
        agents: Vec::new(),
    }
}
