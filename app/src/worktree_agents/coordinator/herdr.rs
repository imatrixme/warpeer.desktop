use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use warpui::ModelContext;
use warpui::r#async::Timer;

use super::{WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent};
use crate::worktree_agents::identity::{ExecutionHostKey, WorktreeKey};
use crate::worktree_agents::observation::{
    AgentKey, AgentObservation, BoundedStatusLabel, SourceAuthority, SourceHealth,
};
use crate::worktree_agents::providers::herdr::{
    HerdrAgentRecord, HerdrSnapshot, HerdrSnapshotError, query_snapshot,
};
use crate::worktree_agents::routing::{AgentRoute, RouteConfidence};
use crate::worktree_agents::runtimes::AgentRuntimeHint;
use crate::worktree_agents::sources::{
    ObservationSourceSpec, SourceCapabilities, SourceDiagnostic, SourceScheduler,
};

const SOURCE_ID: &str = "herdr-api-snapshot";
const REFRESH_INTERVAL: Duration = Duration::from_secs(4);
const EXPIRY_MS: u64 = 15_000;

pub(super) fn source_id() -> &'static str {
    SOURCE_ID
}

pub(super) fn source_scheduler() -> SourceScheduler {
    SourceScheduler::new(ObservationSourceSpec {
        id: SOURCE_ID,
        capabilities: SourceCapabilities {
            progressive_snapshots: true,
            structured_events: true,
            diagnostics: true,
            cancellation: true,
            capability_detection: true,
        },
        foreground_interval_ms: 4_000,
        background_interval_ms: 20_000,
        expiry_ms: EXPIRY_MS,
    })
}

impl WorktreeAgentCoordinator {
    pub(super) fn start_herdr_polling(&mut self, ctx: &mut ModelContext<Self>) {
        self.schedule_herdr_poll(Duration::ZERO, ctx);
    }

    pub(super) fn local_worktree_for_cwd(&self, cwd: &str) -> Option<WorktreeKey> {
        let cwd = Path::new(cwd);
        self.worktree_metadata
            .keys()
            .filter(|key| {
                matches!(key.repository().host(), ExecutionHostKey::Local)
                    && cwd.starts_with(key.path())
            })
            .max_by_key(|key| key.path().components().count())
            .cloned()
    }

    fn schedule_herdr_poll(&mut self, delay: Duration, ctx: &mut ModelContext<Self>) {
        if self.herdr_task_in_flight {
            return;
        }
        let Some(visibility) = self.local_source_visibility(ctx) else {
            self.herdr_source.cancel(SourceDiagnostic::Cancelled);
            return;
        };
        self.herdr_task_in_flight = true;
        self.herdr_generation = self.herdr_generation.wrapping_add(1).max(1);
        let generation = self.herdr_generation;
        if !delay.is_zero() {
            ctx.spawn(
                async move {
                    Timer::after(delay).await;
                    generation
                },
                |coordinator, generation, ctx| {
                    coordinator.herdr_task_in_flight = false;
                    if generation == coordinator.herdr_generation {
                        coordinator.schedule_herdr_poll(Duration::ZERO, ctx);
                    }
                },
            );
            return;
        }
        let lease = self
            .herdr_source
            .begin(self.now_ms(), Some(visibility), false);
        ctx.spawn(
            async move {
                let result = if lease.is_some() {
                    Some(query_snapshot().await)
                } else {
                    None
                };
                (generation, lease, result, visibility)
            },
            |coordinator, (generation, lease, result, visibility), ctx| {
                coordinator.herdr_task_in_flight = false;
                if generation != coordinator.herdr_generation {
                    return;
                }
                let now_ms = coordinator.now_ms();
                if let (Some(lease), Some(result)) = (lease, result) {
                    let (health, diagnostic) = herdr_health(&result);
                    if coordinator
                        .herdr_source
                        .complete(lease, health, diagnostic, now_ms, visibility)
                        && let Ok(snapshot) = result
                    {
                        coordinator.apply_herdr_snapshot(snapshot, ctx);
                    }
                }
                if coordinator.observations.mark_expired(now_ms) > 0 {
                    ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
                    ctx.notify();
                }
                coordinator.schedule_herdr_poll(REFRESH_INTERVAL, ctx);
            },
        );
    }

    fn apply_herdr_snapshot(&mut self, snapshot: HerdrSnapshot, ctx: &mut ModelContext<Self>) {
        let now_ms = self.now_ms();
        let supports_exact_routes = snapshot.supports_exact_agent_routes();
        let mut next_keys = HashSet::new();
        let mut changed = false;
        for agent in snapshot.agents {
            let Some(worktree) = agent
                .cwd
                .as_deref()
                .and_then(|cwd| self.local_worktree_for_cwd(cwd))
            else {
                continue;
            };
            let key = AgentKey {
                worktree,
                provider: agent.provider,
                session_id: agent.provider_session_id.clone(),
            };
            next_keys.insert(key.clone());
            changed |= self.apply_observation(observation(key.clone(), &agent, now_ms));
            if supports_exact_routes {
                changed |= self
                    .routes
                    .publish(
                        key,
                        SOURCE_ID,
                        AgentRoute::Herdr {
                            session: agent.terminal_id.clone(),
                            workspace: Some(agent.workspace_id.clone()),
                            tab: agent.tab_id.clone(),
                            pane: agent.pane_id.clone(),
                            agent: agent.terminal_id,
                        },
                        RouteConfidence::Exact,
                        now_ms,
                        now_ms.saturating_add(EXPIRY_MS),
                    )
                    .is_some();
            } else {
                changed |= self.routes.remove_source(&key, SOURCE_ID);
            }
        }
        for key in self.herdr_agent_keys.difference(&next_keys) {
            changed |= self.observations.remove_source(key, SOURCE_ID);
            self.routes.remove_source(key, SOURCE_ID);
        }
        self.herdr_agent_keys = next_keys;
        if changed {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }
}

fn herdr_health(
    result: &Result<HerdrSnapshot, HerdrSnapshotError>,
) -> (SourceHealth, Option<SourceDiagnostic>) {
    match result {
        Ok(_) => (SourceHealth::Healthy, None),
        Err(HerdrSnapshotError::Unavailable) => (
            SourceHealth::Unavailable,
            Some(SourceDiagnostic::Unavailable),
        ),
        Err(HerdrSnapshotError::Timeout) => {
            (SourceHealth::Degraded, Some(SourceDiagnostic::TimedOut))
        }
        Err(HerdrSnapshotError::Failed)
        | Err(HerdrSnapshotError::TooLarge)
        | Err(HerdrSnapshotError::Invalid) => {
            (SourceHealth::Degraded, Some(SourceDiagnostic::Failed))
        }
    }
}

fn observation(key: AgentKey, agent: &HerdrAgentRecord, now_ms: u64) -> AgentObservation {
    let label = agent.label.as_deref().and_then(BoundedStatusLabel::new);
    AgentObservation {
        key,
        lifecycle: agent.lifecycle,
        authority: SourceAuthority::ProviderSnapshot,
        source_id: SOURCE_ID,
        sequence: Some(agent.sequence),
        observed_at_ms: now_ms,
        expires_at_ms: now_ms.saturating_add(EXPIRY_MS),
        runtime: AgentRuntimeHint::Herdr,
        relation: None,
        label: label.clone(),
        metrics: crate::worktree_agents::observation::AgentMetrics::with_session_title(label),
    }
}
