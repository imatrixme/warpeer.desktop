use super::status::lifecycle_priority;
use crate::worktree_agents::observation::AgentLifecycle;
use crate::worktree_agents::projections::{AgentActivationKind, AgentProjection};
use crate::worktree_agents::routing::RouteConfidence;
use crate::worktree_agents::settings::WorktreeAgentSortKey;

pub(super) fn sort_agents(
    agents: &mut [AgentProjection],
    key: WorktreeAgentSortKey,
    descending: bool,
) {
    agents.sort_by(|left, right| {
        let ordering = match key {
            WorktreeAgentSortKey::Auto => auto_priority(left)
                .cmp(&auto_priority(right))
                .then_with(|| right.observed_at_ms.cmp(&left.observed_at_ms)),
            WorktreeAgentSortKey::Status => {
                lifecycle_priority(left.lifecycle).cmp(&lifecycle_priority(right.lifecycle))
            }
            WorktreeAgentSortKey::LastActive => left.observed_at_ms.cmp(&right.observed_at_ms),
            WorktreeAgentSortKey::TokenUsage => compare_optional(
                left.metrics.token_usage,
                right.metrics.token_usage,
                descending,
            ),
            WorktreeAgentSortKey::ContextRemaining => compare_optional(
                left.metrics.context_remaining_percent,
                right.metrics.context_remaining_percent,
                descending,
            ),
            WorktreeAgentSortKey::SessionName => session_title(left).cmp(&session_title(right)),
        };
        let ordering = if key != WorktreeAgentSortKey::Auto
            && !matches!(
                key,
                WorktreeAgentSortKey::TokenUsage | WorktreeAgentSortKey::ContextRemaining
            )
            && descending
        {
            ordering.reverse()
        } else {
            ordering
        };
        ordering.then_with(|| left.key.session_id.cmp(&right.key.session_id))
    });
}

fn compare_optional<T: Ord>(
    left: Option<T>,
    right: Option<T>,
    descending: bool,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) if descending => right.cmp(&left),
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn auto_priority(agent: &AgentProjection) -> u8 {
    if agent.current_pane {
        return 0;
    }
    if agent.route.is_some_and(|route| {
        route.kind == AgentActivationKind::FocusPane && route.confidence == RouteConfidence::Exact
    }) {
        return 1;
    }
    match agent.lifecycle {
        AgentLifecycle::NeedsInput => 2,
        AgentLifecycle::Starting | AgentLifecycle::Thinking | AgentLifecycle::Working => 3,
        AgentLifecycle::ReadyForReview => 4,
        AgentLifecycle::Detected => 5,
        AgentLifecycle::Completed | AgentLifecycle::Failed | AgentLifecycle::Stopped => 6,
        AgentLifecycle::Stale => 7,
    }
}

fn session_title(agent: &AgentProjection) -> String {
    agent
        .metrics
        .session_title
        .as_ref()
        .or(agent.label.as_ref())
        .map(|label| label.as_str().to_lowercase())
        .unwrap_or_else(|| agent.key.session_id.to_lowercase())
}

#[cfg(test)]
#[path = "sorting_tests.rs"]
mod tests;
