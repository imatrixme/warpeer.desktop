#[path = "host/codex.rs"]
mod codex;
#[path = "host/health.rs"]
mod health;

use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use health::{claude_health, codex_health, source_spec, tmux_health};
use warpui::ModelContext;
use warpui::r#async::Timer;

use super::{WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent};
use crate::worktree_agents::launch::NativeResumeTarget;
use crate::worktree_agents::observation::{
    AgentKey, AgentObservation, BoundedStatusLabel, SourceAuthority, SourceHealth,
};
use crate::worktree_agents::providers::AgentProvider;
use crate::worktree_agents::providers::claude::{ClaudeAgentRecord, query_snapshot};
use crate::worktree_agents::providers::codex::CodexSessionCollector;
use crate::worktree_agents::providers::tmux::{
    InferredTmuxAgent, TmuxPaneRecord, TmuxProcessSnapshot, infer_agents, pane_for_process,
    query_snapshot as query_tmux_snapshot,
};
use crate::worktree_agents::routing::{AgentRoute, RouteConfidence};
use crate::worktree_agents::runtimes::AgentRuntimeHint;
use crate::worktree_agents::sources::{SourceDiagnostic, SourceScheduler};

const OBSERVATION_SOURCE_ID: &str = "claude-agent-snapshot";
const CODEX_OBSERVATION_SOURCE_ID: &str = "codex-app-server-snapshot";
const INFERENCE_SOURCE_ID: &str = "tmux-process-inference";
const ROUTE_SOURCE_ID: &str = "claude-tmux-correlation";
const CLAUDE_RESUME_SOURCE_ID: &str = "claude-exact-resume";
const CODEX_RESUME_SOURCE_ID: &str = "codex-exact-resume";
const REFRESH_INTERVAL: Duration = Duration::from_secs(6);
const EXPIRY_MS: u64 = 20_000;

pub(super) struct HostSourcePolls {
    claude: SourceScheduler,
    codex: SourceScheduler,
    codex_collector: CodexSessionCollector,
    tmux: SourceScheduler,
    tmux_snapshot: Option<(u64, TmuxProcessSnapshot)>,
}

impl Default for HostSourcePolls {
    fn default() -> Self {
        Self {
            claude: SourceScheduler::new(source_spec(OBSERVATION_SOURCE_ID, EXPIRY_MS)),
            codex: SourceScheduler::new(source_spec(CODEX_OBSERVATION_SOURCE_ID, EXPIRY_MS)),
            codex_collector: CodexSessionCollector::default(),
            tmux: SourceScheduler::new(source_spec(INFERENCE_SOURCE_ID, EXPIRY_MS)),
            tmux_snapshot: None,
        }
    }
}

impl HostSourcePolls {
    pub(super) fn health_for(&self, source_id: &'static str) -> SourceHealth {
        match source_id {
            OBSERVATION_SOURCE_ID => self.claude.health(),
            CODEX_OBSERVATION_SOURCE_ID => self.codex.health(),
            INFERENCE_SOURCE_ID => self.tmux.health(),
            _ => SourceHealth::Degraded,
        }
    }

    fn tmux_snapshot(&self, now_ms: u64) -> Option<&TmuxProcessSnapshot> {
        self.tmux_snapshot
            .as_ref()
            .filter(|(observed_at_ms, _)| {
                observed_at_ms.saturating_add(self.tmux.spec().expiry_ms) > now_ms
            })
            .map(|(_, snapshot)| snapshot)
    }
}

impl WorktreeAgentCoordinator {
    pub(super) fn start_host_polling(&mut self, ctx: &mut ModelContext<Self>) {
        self.schedule_host_poll(Duration::ZERO, ctx);
    }

    fn schedule_host_poll(&mut self, delay: Duration, ctx: &mut ModelContext<Self>) {
        if self.host_poll_in_flight {
            return;
        }
        let Some(visibility) = self.local_source_visibility(ctx) else {
            self.host_source_polls
                .claude
                .cancel(SourceDiagnostic::Cancelled);
            self.host_source_polls
                .codex
                .cancel(SourceDiagnostic::Cancelled);
            self.host_source_polls
                .tmux
                .cancel(SourceDiagnostic::Cancelled);
            return;
        };
        self.host_poll_in_flight = true;
        self.host_generation = self.host_generation.wrapping_add(1).max(1);
        let generation = self.host_generation;
        if !delay.is_zero() {
            ctx.spawn(
                async move {
                    Timer::after(delay).await;
                    generation
                },
                |coordinator, generation, ctx| {
                    coordinator.host_poll_in_flight = false;
                    if generation == coordinator.host_generation {
                        coordinator.schedule_host_poll(Duration::ZERO, ctx);
                    }
                },
            );
            return;
        }
        let now_ms = self.now_ms();
        let claude_lease = self
            .host_source_polls
            .claude
            .begin(now_ms, Some(visibility), false);
        let codex_lease = self
            .host_source_polls
            .codex
            .begin(now_ms, Some(visibility), false);
        let tmux_lease = self
            .host_source_polls
            .tmux
            .begin(now_ms, Some(visibility), false);
        let worktree_roots = self
            .worktree_metadata
            .keys()
            .map(|worktree| worktree.path().to_path_buf())
            .collect::<Vec<_>>();
        let codex_hints = self
            .host_source_polls
            .tmux_snapshot(now_ms)
            .map(codex::active_hints)
            .unwrap_or_default();
        let codex_collector = self.host_source_polls.codex_collector.clone();
        ctx.spawn(
            async move {
                let (claude, codex, tmux) = futures::join!(
                    async {
                        if claude_lease.is_some() {
                            Some(query_snapshot().await)
                        } else {
                            None
                        }
                    },
                    async {
                        if codex_lease.is_some() {
                            Some(
                                codex_collector
                                    .query_snapshot(&worktree_roots, &codex_hints)
                                    .await,
                            )
                        } else {
                            None
                        }
                    },
                    async {
                        if tmux_lease.is_some() {
                            Some(query_tmux_snapshot().await)
                        } else {
                            None
                        }
                    }
                );
                (
                    generation,
                    visibility,
                    claude_lease,
                    claude,
                    codex_lease,
                    codex,
                    tmux_lease,
                    tmux,
                )
            },
            |coordinator,
             (
                generation,
                visibility,
                claude_lease,
                claude,
                codex_lease,
                codex,
                tmux_lease,
                tmux,
            ),
             ctx| {
                coordinator.host_poll_in_flight = false;
                if generation == coordinator.host_generation {
                    let now_ms = coordinator.now_ms();
                    if let (Some(lease), Some(result)) = (tmux_lease, tmux) {
                        let (health, diagnostic) = tmux_health(&result);
                        if coordinator
                            .host_source_polls
                            .tmux
                            .complete(lease, health, diagnostic, now_ms, visibility)
                            && let Ok(snapshot) = result
                        {
                            coordinator.host_source_polls.tmux_snapshot = Some((now_ms, snapshot));
                        }
                    }
                    let tmux_snapshot =
                        coordinator.host_source_polls.tmux_snapshot(now_ms).cloned();
                    if let (Some(lease), Some(result)) = (claude_lease, claude) {
                        let (health, diagnostic) = claude_health(&result);
                        if coordinator
                            .host_source_polls
                            .claude
                            .complete(lease, health, diagnostic, now_ms, visibility)
                            && let Ok(agents) = result
                        {
                            coordinator.apply_claude_snapshot(agents, tmux_snapshot.as_ref(), ctx);
                        }
                    }
                    if let (Some(lease), Some(result)) = (codex_lease, codex) {
                        let (health, diagnostic) = codex_health(&result);
                        if coordinator
                            .host_source_polls
                            .codex
                            .complete(lease, health, diagnostic, now_ms, visibility)
                            && let Ok(agents) = result
                        {
                            coordinator.apply_codex_snapshot(agents, tmux_snapshot.as_ref(), ctx);
                        }
                    }
                    if let Some(tmux) = tmux_snapshot.as_ref() {
                        coordinator.apply_process_inference(tmux, ctx);
                    }
                    if coordinator.observations.mark_expired(now_ms) > 0 {
                        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
                        ctx.notify();
                    }
                }
                coordinator.schedule_host_poll(REFRESH_INTERVAL, ctx);
            },
        );
    }

    fn apply_claude_snapshot(
        &mut self,
        agents: Vec<ClaudeAgentRecord>,
        tmux: Option<&TmuxProcessSnapshot>,
        ctx: &mut ModelContext<Self>,
    ) {
        let now_ms = self.now_ms();
        let mut next_keys = HashSet::new();
        let mut next_process_ids = HashSet::new();
        let mut changed = false;
        for agent in agents {
            let Some(worktree) = self.local_worktree_for_cwd(&agent.cwd) else {
                continue;
            };
            let key = AgentKey {
                worktree: worktree.clone(),
                provider: AgentProvider::ClaudeCode,
                session_id: agent.session_id,
            };
            next_keys.insert(key.clone());
            let pane = agent
                .pid
                .and_then(|pid| tmux.and_then(|snapshot| pane_for_process(snapshot, pid)))
                .filter(|pane| Path::new(&pane.cwd).starts_with(worktree.path()));
            if let (Some(pane), Some(process_id)) = (pane, agent.pid) {
                next_process_ids.insert(process_id);
                self.session_info.bind_tmux_process(
                    AgentProvider::ClaudeCode,
                    process_id,
                    pane,
                    &key.session_id,
                );
            }
            let label = agent.label.and_then(BoundedStatusLabel::new);
            changed |= self.apply_observation(AgentObservation {
                key: key.clone(),
                lifecycle: agent.lifecycle,
                authority: SourceAuthority::ProviderSnapshot,
                source_id: OBSERVATION_SOURCE_ID,
                sequence: agent.sequence,
                observed_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(EXPIRY_MS),
                runtime: if pane.is_some() {
                    AgentRuntimeHint::Tmux
                } else {
                    AgentRuntimeHint::Unknown
                },
                relation: None,
                label: label.clone(),
                metrics: crate::worktree_agents::observation::AgentMetrics::with_session_title(
                    label,
                ),
            });
            changed |= self.update_tmux_route(key.clone(), pane, now_ms);
            changed |= self.update_native_resume_route(
                key,
                CLAUDE_RESUME_SOURCE_ID,
                pane.is_none(),
                now_ms,
            );
        }
        for key in self.claude_agent_keys.difference(&next_keys) {
            changed |= self.observations.remove_source(key, OBSERVATION_SOURCE_ID);
            changed |= self.routes.remove_source(key, ROUTE_SOURCE_ID);
            changed |= self.routes.remove_source(key, CLAUDE_RESUME_SOURCE_ID);
        }
        self.claude_agent_keys = next_keys;
        self.claude_process_ids = next_process_ids;
        if changed {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }

    fn update_tmux_route(
        &mut self,
        key: AgentKey,
        pane: Option<&TmuxPaneRecord>,
        now_ms: u64,
    ) -> bool {
        let Some(pane) = pane else {
            return self.routes.remove_source(&key, ROUTE_SOURCE_ID);
        };
        self.routes
            .publish(
                key,
                ROUTE_SOURCE_ID,
                AgentRoute::Tmux {
                    socket: pane.socket.clone(),
                    session: pane.session.clone(),
                    window: pane.window.clone(),
                    pane: pane.pane.clone(),
                },
                RouteConfidence::Exact,
                now_ms,
                now_ms.saturating_add(EXPIRY_MS),
            )
            .is_some()
    }

    fn update_native_resume_route(
        &mut self,
        key: AgentKey,
        source_id: &'static str,
        available: bool,
        now_ms: u64,
    ) -> bool {
        if !available || key.session_id.trim().is_empty() || !key.worktree.path().is_dir() {
            return self.routes.remove_source(&key, source_id);
        }
        let target = NativeResumeTarget {
            provider: key.provider,
            session_id: key.session_id.clone(),
            worktree: key.worktree.path().to_path_buf(),
        };
        self.routes
            .publish(
                key,
                source_id,
                AgentRoute::NativeResume(target),
                RouteConfidence::Exact,
                now_ms,
                now_ms.saturating_add(EXPIRY_MS),
            )
            .is_some()
    }

    fn apply_process_inference(
        &mut self,
        snapshot: &TmuxProcessSnapshot,
        ctx: &mut ModelContext<Self>,
    ) {
        let now_ms = self.now_ms();
        let mut next_keys = HashSet::new();
        let mut changed = false;
        for inferred in infer_agents(snapshot) {
            let Some(worktree) = self.local_worktree_for_cwd(&inferred.pane.cwd) else {
                continue;
            };
            let key = self.session_info.key_for_tmux_process(worktree, &inferred);
            let matched_structured_process = self.claude_process_ids.contains(&inferred.process_id)
                || self.codex_process_ids.contains(&inferred.process_id);
            if matched_structured_process && key.session_id.starts_with("tmux-process:") {
                continue;
            }
            next_keys.insert(key.clone());
            changed |= self.apply_observation(AgentObservation {
                key: key.clone(),
                lifecycle: crate::worktree_agents::observation::AgentLifecycle::Detected,
                authority: SourceAuthority::ProcessInference,
                source_id: INFERENCE_SOURCE_ID,
                sequence: None,
                observed_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(EXPIRY_MS),
                runtime: AgentRuntimeHint::Tmux,
                relation: None,
                label: None,
                metrics: Default::default(),
            });
            changed |= self
                .routes
                .publish(
                    key,
                    INFERENCE_SOURCE_ID,
                    inferred_tmux_route(&inferred),
                    RouteConfidence::Exact,
                    now_ms,
                    now_ms.saturating_add(EXPIRY_MS),
                )
                .is_some();
        }
        for key in self.inferred_agent_keys.difference(&next_keys) {
            changed |= self.observations.remove_source(key, INFERENCE_SOURCE_ID);
            changed |= self.routes.remove_source(key, INFERENCE_SOURCE_ID);
        }
        self.inferred_agent_keys = next_keys;
        if changed {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }
}

fn inferred_tmux_route(inferred: &InferredTmuxAgent) -> AgentRoute {
    AgentRoute::Tmux {
        socket: inferred.pane.socket.clone(),
        session: inferred.pane.session.clone(),
        window: inferred.pane.window.clone(),
        pane: inferred.pane.pane.clone(),
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
