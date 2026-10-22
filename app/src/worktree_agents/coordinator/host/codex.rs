use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use warpui::ModelContext;

use super::{CODEX_OBSERVATION_SOURCE_ID, CODEX_RESUME_SOURCE_ID, EXPIRY_MS};
use crate::worktree_agents::coordinator::{
    WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent,
};
use crate::worktree_agents::observation::{
    AgentKey, AgentLifecycle, AgentMetrics, AgentObservation, BoundedStatusLabel, SourceAuthority,
};
use crate::worktree_agents::providers::AgentProvider;
use crate::worktree_agents::providers::codex::{CodexActiveHints, CodexAgentRecord};
use crate::worktree_agents::providers::tmux::{
    InferredTmuxAgent, TmuxProcessSnapshot, infer_agents,
};
use crate::worktree_agents::runtimes::AgentRuntimeHint;

impl WorktreeAgentCoordinator {
    pub(super) fn apply_codex_snapshot(
        &mut self,
        agents: Vec<CodexAgentRecord>,
        tmux: Option<&TmuxProcessSnapshot>,
        ctx: &mut ModelContext<Self>,
    ) {
        let now_ms = self.now_ms();
        let mut next_keys = HashSet::new();
        let mut next_process_ids = HashSet::new();
        let mut changed = false;
        let inferred_agents = tmux
            .map(infer_agents)
            .unwrap_or_default()
            .into_iter()
            .filter(|agent| agent.provider == AgentProvider::Codex)
            .collect::<Vec<_>>();
        for agent in agents {
            let Some(worktree) = self.local_worktree_for_cwd(&agent.cwd) else {
                continue;
            };
            let inferred = inferred_agents.iter().find(|inferred| {
                !next_process_ids.contains(&inferred.process_id)
                    && Path::new(&inferred.pane.cwd).starts_with(worktree.path())
                    && record_matches_process(&agent, inferred)
            });
            if inferred.is_none()
                && (agent.relation.is_none() || agent.lifecycle == AgentLifecycle::Detected)
            {
                continue;
            }
            let key = AgentKey {
                worktree: worktree.clone(),
                provider: AgentProvider::Codex,
                session_id: agent.session_id,
            };
            next_keys.insert(key.clone());
            if let Some(inferred) = inferred {
                next_process_ids.insert(inferred.process_id);
                self.session_info.bind_tmux_process(
                    AgentProvider::Codex,
                    inferred.process_id,
                    &inferred.pane,
                    &key.session_id,
                );
            }
            let label = agent.label.and_then(BoundedStatusLabel::new);
            changed |= self.apply_observation(AgentObservation {
                key: key.clone(),
                lifecycle: agent.lifecycle,
                authority: if agent.rollout_derived {
                    SourceAuthority::StructuredEvent
                } else {
                    SourceAuthority::ProviderSnapshot
                },
                source_id: CODEX_OBSERVATION_SOURCE_ID,
                sequence: agent.sequence,
                observed_at_ms: now_ms,
                expires_at_ms: now_ms.saturating_add(EXPIRY_MS),
                runtime: if inferred.is_some() {
                    AgentRuntimeHint::Tmux
                } else {
                    AgentRuntimeHint::Unknown
                },
                relation: agent.relation,
                label: label.clone(),
                metrics: AgentMetrics {
                    session_title: label,
                    model_name: agent.model_name.and_then(BoundedStatusLabel::new),
                    token_usage: agent.token_usage,
                    context_remaining_percent: agent.context_remaining_percent,
                    activity_at_ms: activity_at_ms(now_ms, agent.activity_unix_ms),
                },
            });
            changed |= self.update_tmux_route(
                key.clone(),
                inferred.map(|inferred| &inferred.pane),
                now_ms,
            );
            changed |= self.update_native_resume_route(
                key,
                CODEX_RESUME_SOURCE_ID,
                inferred.is_none(),
                now_ms,
            );
        }
        for key in self.codex_agent_keys.difference(&next_keys) {
            changed |= self
                .observations
                .remove_source(key, CODEX_OBSERVATION_SOURCE_ID);
            changed |= self.routes.remove_source(key, CODEX_RESUME_SOURCE_ID);
        }
        self.codex_agent_keys = next_keys;
        self.codex_process_ids = next_process_ids;
        if changed {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }
}

pub(super) fn active_hints(snapshot: &TmuxProcessSnapshot) -> CodexActiveHints {
    let mut hints = CodexActiveHints::default();
    for agent in infer_agents(snapshot)
        .into_iter()
        .filter(|agent| agent.provider == AgentProvider::Codex)
    {
        hints.session_ids.extend(agent.session_id);
        hints.rollout_paths.extend(agent.rollout_path);
    }
    hints
}

fn record_matches_process(agent: &CodexAgentRecord, inferred: &InferredTmuxAgent) -> bool {
    inferred.session_id.as_deref() == Some(agent.session_id.as_str())
        || inferred
            .rollout_path
            .as_deref()
            .is_some_and(|rollout_path| agent.rollout_path.as_deref() == Some(rollout_path))
}

fn activity_at_ms(now_ms: u64, activity_unix_ms: Option<u64>) -> Option<u64> {
    let activity_unix_ms = activity_unix_ms?;
    let current_unix_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis()
        .try_into()
        .ok()?;
    Some(now_ms.saturating_sub(current_unix_ms.saturating_sub(activity_unix_ms)))
}
