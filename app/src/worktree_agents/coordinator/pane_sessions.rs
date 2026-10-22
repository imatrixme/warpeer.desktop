use std::path::Path;

use warpui::{EntityId, ModelContext, ModelHandle, SingletonEntity as _};

use super::activation::PANE_ACTIVATION_SOURCE_ID;
use super::{PendingPaneAgent, WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent};
use crate::terminal::CLIAgent;
use crate::terminal::cli_agent_sessions::{
    CLIAgentSession, CLIAgentSessionStatus, CLIAgentSessionsModel, CLIAgentSessionsModelEvent,
};
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};
use crate::worktree_agents::observation::{
    AgentKey, AgentLifecycle, AgentMetrics, AgentObservation, BoundedStatusLabel, SourceAuthority,
};
use crate::worktree_agents::providers::AgentProvider;
use crate::worktree_agents::routing::{AgentRoute, RouteConfidence};
use crate::worktree_agents::runtimes::AgentRuntimeHint;

impl WorktreeAgentCoordinator {
    pub(super) fn handle_cli_agent_event(
        &mut self,
        sessions: ModelHandle<CLIAgentSessionsModel>,
        event: &CLIAgentSessionsModelEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        if !self.collection_enabled {
            return;
        }
        let terminal_view_id = event.terminal_view_id();
        if matches!(event, CLIAgentSessionsModelEvent::Ended { .. }) {
            self.remove_pane_agent(terminal_view_id);
            self.pending_agents.remove(&terminal_view_id);
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
            return;
        }

        let pending = sessions.read(ctx, |sessions, _| {
            sessions
                .session(terminal_view_id)
                .map(|session| PendingPaneAgent::from_session(terminal_view_id, session))
        });
        let Some(pending) = pending else {
            return;
        };
        self.pending_agents.insert(terminal_view_id, pending);
        self.materialize_pending_agent(terminal_view_id);
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
    }

    pub(super) fn restore_cli_agent_sessions(&mut self, ctx: &mut ModelContext<Self>) {
        let terminal_view_ids: Vec<_> = self.pane_worktrees.keys().copied().collect();
        let sessions = CLIAgentSessionsModel::handle(ctx);
        let pending = sessions.read(ctx, |sessions, _| {
            terminal_view_ids
                .into_iter()
                .filter_map(|terminal_view_id| {
                    sessions.session(terminal_view_id).map(|session| {
                        (
                            terminal_view_id,
                            PendingPaneAgent::from_session(terminal_view_id, session),
                        )
                    })
                })
                .collect::<Vec<_>>()
        });
        for (terminal_view_id, pending) in pending {
            self.pending_agents.insert(terminal_view_id, pending);
            self.materialize_pending_agent(terminal_view_id);
        }
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
    }

    pub(super) fn materialize_pending_agent(&mut self, terminal_view_id: EntityId) {
        let Some(pending) = self.pending_agents.get(&terminal_view_id).cloned() else {
            return;
        };
        let Some(current_worktree) = self.pane_worktrees.get(&terminal_view_id).cloned() else {
            return;
        };
        if !can_observe_agents(&current_worktree, None) {
            self.remove_pane_agent(terminal_view_id);
            return;
        }
        let worktree = self
            .worktree_for_cwd(current_worktree.repository(), pending.cwd.as_deref())
            .unwrap_or(current_worktree);
        self.remove_pane_agent(terminal_view_id);

        let key = AgentKey {
            worktree,
            provider: pending.provider,
            session_id: pending.session_id,
        };
        let now_ms = self.now_ms();
        let source_id = source_id_for_authority(pending.authority);
        self.apply_observation(AgentObservation {
            key: key.clone(),
            lifecycle: pending.lifecycle,
            authority: pending.authority,
            source_id,
            sequence: None,
            observed_at_ms: now_ms,
            expires_at_ms: now_ms.saturating_add(300_000),
            runtime: AgentRuntimeHint::NormalPty,
            relation: None,
            label: BoundedStatusLabel::new(lifecycle_label(pending.lifecycle)),
            metrics: pending.metrics,
        });
        let Some(correlation) = self.pane_index.bind_agent(terminal_view_id, key.clone()) else {
            return;
        };
        self.routes.publish(
            key.clone(),
            source_id,
            AgentRoute::ExistingPane {
                terminal_view_id,
                correlation,
            },
            RouteConfidence::Exact,
            now_ms,
            now_ms.saturating_add(300_000),
        );
        self.pane_agent_keys.insert(terminal_view_id, key);
    }

    fn worktree_for_cwd(
        &self,
        repository: &RepositoryKey,
        cwd: Option<&str>,
    ) -> Option<WorktreeKey> {
        let cwd = Path::new(cwd?);
        self.worktree_metadata
            .keys()
            .filter(|key| key.repository() == repository && cwd.starts_with(key.path()))
            .max_by_key(|key| key.path().components().count())
            .cloned()
    }

    pub(super) fn remove_pane_agent(&mut self, terminal_view_id: EntityId) -> bool {
        if let Some(key) = self.pane_agent_keys.remove(&terminal_view_id) {
            self.pane_index.unbind_agent(terminal_view_id);
            for source_id in [
                "warp-cli-agent-plugin",
                "warp-cli-agent-snapshot",
                "warp-cli-agent-detection",
                PANE_ACTIVATION_SOURCE_ID,
            ] {
                self.observations.remove_source(&key, source_id);
                self.routes.remove_source(&key, source_id);
            }
            true
        } else {
            false
        }
    }
}

pub(super) fn can_observe_agents(
    worktree: &WorktreeKey,
    authenticated_remote_host: Option<&ExecutionHostKey>,
) -> bool {
    match worktree.repository().host() {
        ExecutionHostKey::Local => true,
        remote @ ExecutionHostKey::Remote(_) => authenticated_remote_host == Some(remote),
    }
}

impl PendingPaneAgent {
    fn from_session(terminal_view_id: EntityId, session: &CLIAgentSession) -> Self {
        let session_id = session
            .session_context
            .session_id
            .clone()
            .unwrap_or_else(|| format!("pane-{terminal_view_id:?}"));
        Self {
            provider: provider_from_cli_agent(session.agent),
            session_id,
            cwd: session.session_context.cwd.clone(),
            lifecycle: lifecycle_from_cli_status(&session.status),
            authority: if session.supports_rich_status() {
                SourceAuthority::StructuredEvent
            } else {
                SourceAuthority::ProcessInference
            },
            metrics: if session.supports_rich_status() {
                AgentMetrics::with_session_title(
                    session
                        .session_context
                        .display_title()
                        .and_then(BoundedStatusLabel::new),
                )
            } else {
                AgentMetrics::default()
            },
        }
    }
}

pub(super) fn provider_from_cli_agent(agent: CLIAgent) -> AgentProvider {
    AgentProvider::from_cli_agent(agent)
}

pub(super) fn lifecycle_from_cli_status(status: &CLIAgentSessionStatus) -> AgentLifecycle {
    match status {
        CLIAgentSessionStatus::InProgress => AgentLifecycle::Working,
        CLIAgentSessionStatus::Success => AgentLifecycle::ReadyForReview,
        CLIAgentSessionStatus::Failed { .. } => AgentLifecycle::Failed,
        CLIAgentSessionStatus::Blocked { .. } => AgentLifecycle::NeedsInput,
        CLIAgentSessionStatus::Cancelled => AgentLifecycle::Stopped,
    }
}

pub(super) fn lifecycle_label(lifecycle: AgentLifecycle) -> &'static str {
    match lifecycle {
        AgentLifecycle::Detected => "State unknown",
        AgentLifecycle::Starting => "Starting",
        AgentLifecycle::Thinking => "Thinking",
        AgentLifecycle::Working => "Working",
        AgentLifecycle::NeedsInput => "Needs input",
        AgentLifecycle::ReadyForReview => "Awaiting input",
        AgentLifecycle::Completed => "Completed",
        AgentLifecycle::Failed => "Failed",
        AgentLifecycle::Stopped => "Stopped",
        AgentLifecycle::Stale => "Stale",
    }
}

fn source_id_for_authority(authority: SourceAuthority) -> &'static str {
    match authority {
        SourceAuthority::StructuredEvent => "warp-cli-agent-plugin",
        SourceAuthority::ProviderSnapshot => "warp-cli-agent-snapshot",
        SourceAuthority::ProcessInference => "warp-cli-agent-detection",
    }
}
