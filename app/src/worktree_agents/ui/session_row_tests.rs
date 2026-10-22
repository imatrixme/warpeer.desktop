use super::{session_short_id, *};

#[test]
fn session_short_id_is_bounded_and_sanitized() {
    assert_eq!(session_short_id("abc-1234/secret"), "#abc-1234");
    assert_eq!(session_short_id("///"), "session");
}

#[test]
fn route_labels_withhold_actions_for_non_exact_routes() {
    let inferred = AgentRouteProjection {
        kind: AgentActivationKind::FocusPane,
        confidence: RouteConfidence::Inferred,
        generation: 7,
    };
    assert_eq!(
        action_label(
            AgentProvider::ClaudeCode,
            SourceHealth::Healthy,
            true,
            false,
            Some(inferred),
            None,
        ),
        "Observe only"
    );
    let exact = AgentRouteProjection {
        confidence: RouteConfidence::Exact,
        ..inferred
    };
    assert_eq!(
        action_label(
            AgentProvider::ClaudeCode,
            SourceHealth::Healthy,
            true,
            false,
            Some(exact),
            None,
        ),
        "Focus attached pane"
    );
}

#[test]
fn activation_outcome_overrides_the_planned_route_label() {
    let exact = AgentRouteProjection {
        kind: AgentActivationKind::FocusPane,
        confidence: RouteConfidence::Exact,
        generation: 3,
    };
    assert_eq!(
        action_label(
            AgentProvider::ClaudeCode,
            SourceHealth::Healthy,
            true,
            false,
            Some(exact),
            Some(HeaderActionOutcome::Stale),
        ),
        "Route expired"
    );
}

#[test]
fn action_labels_explain_unavailable_resume_paths() {
    assert_eq!(
        action_label(
            AgentProvider::ClaudeCode,
            SourceHealth::Unavailable,
            true,
            false,
            None,
            None,
        ),
        "Provider unavailable"
    );
    assert_eq!(
        action_label(
            AgentProvider::Codex,
            SourceHealth::Healthy,
            false,
            false,
            None,
            None,
        ),
        "Worktree unavailable"
    );
    assert_eq!(
        action_label(
            AgentProvider::Unknown,
            SourceHealth::Healthy,
            true,
            false,
            None,
            None,
        ),
        "Resume unsupported"
    );
}

#[test]
fn current_pane_label_overrides_the_route_action() {
    assert_eq!(
        action_label(
            AgentProvider::Codex,
            SourceHealth::Healthy,
            true,
            true,
            Some(AgentRouteProjection {
                kind: AgentActivationKind::FocusPane,
                confidence: RouteConfidence::Exact,
                generation: 4,
            }),
            None,
        ),
        "Current pane"
    );
}

#[test]
fn source_and_route_labels_expose_degraded_state_without_raw_payloads() {
    assert_eq!(
        source_name(SourceAuthority::ProviderSnapshot, SourceHealth::Degraded),
        "source degraded"
    );
    assert_eq!(
        route_confidence_name(Some(AgentRouteProjection {
            kind: AgentActivationKind::ObserveOnly,
            confidence: RouteConfidence::Stale,
            generation: 9,
        })),
        "stale route"
    );
}
