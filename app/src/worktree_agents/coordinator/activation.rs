use std::path::{Path, PathBuf};

use warpui::{EntityId, ModelContext};

use super::{AgentActivationDecision, WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent};
use crate::worktree_agents::identity::ExecutionHostKey;
use crate::worktree_agents::launch::WorktreeAgentLaunch;
use crate::worktree_agents::navigation::{WorktreeNavigationDecision, decide};
use crate::worktree_agents::observation::AgentKey;
use crate::worktree_agents::routing::AgentRoute;
use crate::worktree_agents::runtimes::TmuxAttachTarget;

pub(super) const PANE_ACTIVATION_SOURCE_ID: &str = "warpeer-pane-activation";

impl WorktreeAgentCoordinator {
    pub(crate) fn attached_pane_for_agent(&self, key: &AgentKey) -> Option<EntityId> {
        let record = self.routes.get(key)?;
        if record.confidence != crate::worktree_agents::routing::RouteConfidence::Exact
            || record.expires_at_ms <= self.now_ms()
        {
            return None;
        }
        let AgentRoute::ExistingPane {
            terminal_view_id,
            correlation,
        } = &record.route
        else {
            return None;
        };
        self.pane_index
            .validate_agent(key, *terminal_view_id, *correlation)
            .then_some(*terminal_view_id)
    }

    pub(crate) fn attached_agent_worktree_path(
        &self,
        terminal_view_id: EntityId,
    ) -> Option<PathBuf> {
        self.pane_agent_keys
            .get(&terminal_view_id)
            .map(|key| key.worktree.path().to_path_buf())
    }

    pub(crate) fn detach_activated_agent(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let Some(key) = self.pane_agent_keys.remove(&terminal_view_id) else {
            return false;
        };
        self.pane_index.unbind_agent(terminal_view_id);
        for source_id in [
            PANE_ACTIVATION_SOURCE_ID,
            "warp-cli-agent-plugin",
            "warp-cli-agent-snapshot",
            "warp-cli-agent-detection",
        ] {
            self.routes.remove_source(&key, source_id);
        }
        if self.hovered_agent.as_ref() == Some(&key) {
            self.hovered_agent = None;
        }
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
        true
    }

    pub(crate) fn bind_activated_agent(
        &mut self,
        terminal_view_id: EntityId,
        key: AgentKey,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        if self
            .pane_worktrees
            .get(&terminal_view_id)
            .is_some_and(|current| current.repository() != key.worktree.repository())
        {
            return false;
        }
        self.remove_pane_agent(terminal_view_id);
        if !self.pane_worktrees.contains_key(&terminal_view_id) {
            self.pane_worktrees
                .insert(terminal_view_id, key.worktree.clone());
            self.pane_index
                .publish_worktree(terminal_view_id, key.worktree.clone());
        }
        let Some(correlation) = self.pane_index.bind_agent(terminal_view_id, key.clone()) else {
            return false;
        };
        let now_ms = self.now_ms();
        self.routes.publish(
            key.clone(),
            PANE_ACTIVATION_SOURCE_ID,
            AgentRoute::ExistingPane {
                terminal_view_id,
                correlation,
            },
            crate::worktree_agents::routing::RouteConfidence::Exact,
            now_ms,
            u64::MAX,
        );
        self.pane_agent_keys.insert(terminal_view_id, key);
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
        true
    }

    pub(crate) fn resolve_activation(
        &self,
        source_terminal_view_id: EntityId,
        key: &AgentKey,
        generation: u64,
    ) -> AgentActivationDecision {
        let now_ms = self.now_ms();
        let Some(record) = self.routes.resolve_exact_record(key, generation, now_ms) else {
            return AgentActivationDecision::Stale;
        };
        match &record.route {
            AgentRoute::ExistingPane {
                terminal_view_id,
                correlation,
            } if !self
                .pane_index
                .validate_agent(key, *terminal_view_id, *correlation) =>
            {
                AgentActivationDecision::Stale
            }
            AgentRoute::ExistingPane {
                terminal_view_id, ..
            } if *terminal_view_id == source_terminal_view_id => {
                AgentActivationDecision::AlreadyFocused
            }
            AgentRoute::ExistingPane {
                terminal_view_id, ..
            } => AgentActivationDecision::FocusPane(*terminal_view_id),
            AgentRoute::Herdr { agent, .. } => {
                AgentActivationDecision::Launch(WorktreeAgentLaunch::AttachHerdr {
                    worktree: key.worktree.path().to_path_buf(),
                    target: agent.clone(),
                })
            }
            AgentRoute::Tmux {
                socket,
                session,
                window,
                pane,
            } => AgentActivationDecision::Launch(WorktreeAgentLaunch::AttachTmux {
                worktree: key.worktree.path().to_path_buf(),
                target: TmuxAttachTarget {
                    socket: socket.clone(),
                    session: session.clone(),
                    window: window.clone(),
                    pane: pane.clone(),
                },
            }),
            AgentRoute::NativeResume(target)
                if matches!(key.worktree.repository().host(), ExecutionHostKey::Local)
                    && target.worktree.is_dir() =>
            {
                AgentActivationDecision::Launch(WorktreeAgentLaunch::NativeResume(target.clone()))
            }
            AgentRoute::NativeResume(_) => AgentActivationDecision::Stale,
            AgentRoute::ObserveOnly => AgentActivationDecision::ObserveOnly,
        }
    }

    pub(crate) fn resolve_worktree_navigation(
        &self,
        source_terminal_view_id: EntityId,
        target_path: &Path,
        current_pane_is_safe: bool,
    ) -> WorktreeNavigationDecision {
        let Some(source_worktree) = self.pane_worktrees.get(&source_terminal_view_id) else {
            return WorktreeNavigationDecision::Stale;
        };
        let target = self.worktree_metadata.iter().find(|(key, _)| {
            key.repository() == source_worktree.repository() && key.path() == target_path
        });
        let Some((target_key, metadata)) = target else {
            return WorktreeNavigationDecision::Stale;
        };
        let existing_pane = self
            .pane_index
            .pane_for_worktree(source_terminal_view_id, target_key);
        let target_exists = metadata.available
            && (!matches!(source_worktree.repository().host(), ExecutionHostKey::Local)
                || target_path.is_dir());
        decide(
            source_worktree.repository().host(),
            target_path,
            existing_pane,
            current_pane_is_safe,
            target_exists,
        )
    }
}
