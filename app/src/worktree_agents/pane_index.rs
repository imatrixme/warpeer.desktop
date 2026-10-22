use std::collections::HashMap;

use warpui::EntityId;

use super::identity::WorktreeKey;
use super::observation::AgentKey;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct LaunchCorrelation(pub(crate) u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PaneRouteFacts {
    worktree: WorktreeKey,
    agent: Option<AgentKey>,
    correlation: LaunchCorrelation,
}

#[derive(Default)]
pub(crate) struct PaneRouteIndex {
    panes: HashMap<EntityId, PaneRouteFacts>,
    agents: HashMap<AgentKey, EntityId>,
    next_correlation: u64,
}

impl PaneRouteIndex {
    pub(crate) fn publish_worktree(
        &mut self,
        terminal_view_id: EntityId,
        worktree: WorktreeKey,
    ) -> LaunchCorrelation {
        if let Some(current) = self.panes.get_mut(&terminal_view_id) {
            if current.worktree == worktree {
                return current.correlation;
            }
            if current
                .agent
                .as_ref()
                .is_some_and(|agent| agent.worktree == worktree)
            {
                current.worktree = worktree;
                return current.correlation;
            }
        }
        self.remove(terminal_view_id);
        self.next_correlation = self.next_correlation.wrapping_add(1).max(1);
        let correlation = LaunchCorrelation(self.next_correlation);
        self.panes.insert(
            terminal_view_id,
            PaneRouteFacts {
                worktree,
                agent: None,
                correlation,
            },
        );
        correlation
    }

    pub(crate) fn bind_agent(
        &mut self,
        terminal_view_id: EntityId,
        key: AgentKey,
    ) -> Option<LaunchCorrelation> {
        let facts = self.panes.get_mut(&terminal_view_id)?;
        if facts.worktree.repository() != key.worktree.repository() {
            return None;
        }
        if let Some(previous) = facts.agent.replace(key.clone()) {
            self.agents.remove(&previous);
        }
        self.agents.insert(key, terminal_view_id);
        Some(facts.correlation)
    }

    pub(crate) fn validate_agent(
        &self,
        key: &AgentKey,
        terminal_view_id: EntityId,
        correlation: LaunchCorrelation,
    ) -> bool {
        self.agents.get(key) == Some(&terminal_view_id)
            && self.panes.get(&terminal_view_id).is_some_and(|facts| {
                facts.agent.as_ref() == Some(key) && facts.correlation == correlation
            })
    }

    pub(crate) fn pane_for_worktree(
        &self,
        source_terminal_view_id: EntityId,
        worktree: &WorktreeKey,
    ) -> Option<EntityId> {
        self.panes.iter().find_map(|(terminal_view_id, facts)| {
            (*terminal_view_id != source_terminal_view_id && facts.worktree == *worktree)
                .then_some(*terminal_view_id)
        })
    }

    pub(crate) fn unbind_agent(&mut self, terminal_view_id: EntityId) -> bool {
        let Some(facts) = self.panes.get_mut(&terminal_view_id) else {
            return false;
        };
        let Some(agent) = facts.agent.take() else {
            return false;
        };
        self.agents.remove(&agent);
        true
    }

    pub(crate) fn remove(&mut self, terminal_view_id: EntityId) -> bool {
        let Some(facts) = self.panes.remove(&terminal_view_id) else {
            return false;
        };
        if let Some(agent) = facts.agent {
            self.agents.remove(&agent);
        }
        true
    }
}

#[cfg(test)]
#[path = "pane_index_tests.rs"]
mod tests;
