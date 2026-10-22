use std::collections::{HashMap, VecDeque};

use super::identity::WorktreeKey;
use super::observation::{
    AgentKey, AgentLifecycle, AgentMetrics, AgentObservation, AgentRelation, BoundedStatusLabel,
    SourceAuthority,
};
use super::providers::AgentProvider;
use super::providers::tmux::{InferredTmuxAgent, TmuxPaneRecord};

const MAX_METRIC_SAMPLES_PER_SESSION: usize = 240;
const MAX_TRACKED_SESSIONS: usize = 256;
const DUPLICATE_SAMPLE_INTERVAL_MS: u64 = 30_000;
const LIFECYCLE_GRACE_MS: u64 = 30_000;

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentMetricSample {
    observed_at_ms: u64,
    token_usage: Option<u64>,
    context_remaining_percent: Option<u8>,
}

#[derive(Clone)]
struct CachedField<T> {
    value: T,
    authority: SourceAuthority,
    observed_at_ms: u64,
}

#[derive(Default)]
struct AgentSessionInfo {
    last_observed_at_ms: u64,
    lifecycle: Option<CachedField<AgentLifecycle>>,
    relation: Option<CachedField<AgentRelation>>,
    label: Option<CachedField<BoundedStatusLabel>>,
    session_title: Option<CachedField<BoundedStatusLabel>>,
    model_name: Option<CachedField<BoundedStatusLabel>>,
    token_usage: Option<CachedField<u64>>,
    context_remaining_percent: Option<CachedField<u8>>,
    activity_at_ms: Option<CachedField<u64>>,
    metric_samples: VecDeque<AgentMetricSample>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TmuxRuntimeKey {
    provider: AgentProvider,
    socket: Option<String>,
    pane: String,
}

#[derive(Default)]
pub(crate) struct AgentSessionInfoCollector {
    sessions: HashMap<AgentKey, AgentSessionInfo>,
    process_sessions: HashMap<(AgentProvider, u32), String>,
    tmux_sessions: HashMap<TmuxRuntimeKey, String>,
}

impl AgentSessionInfoCollector {
    pub(crate) fn bind_tmux_process(
        &mut self,
        provider: AgentProvider,
        process_id: u32,
        pane: &TmuxPaneRecord,
        session_id: &str,
    ) {
        if session_id.trim().is_empty() {
            return;
        }
        self.process_sessions
            .insert((provider, process_id), session_id.to_string());
        self.tmux_sessions
            .insert(tmux_runtime_key(provider, pane), session_id.to_string());
    }

    pub(crate) fn key_for_tmux_process(
        &mut self,
        worktree: WorktreeKey,
        inferred: &InferredTmuxAgent,
    ) -> AgentKey {
        if let Some(session_id) = inferred
            .session_id
            .as_deref()
            .filter(|session_id| !session_id.trim().is_empty())
        {
            self.bind_tmux_process(
                inferred.provider,
                inferred.process_id,
                &inferred.pane,
                session_id,
            );
        }
        let runtime_key = tmux_runtime_key(inferred.provider, &inferred.pane);
        let session_id = self
            .process_sessions
            .get(&(inferred.provider, inferred.process_id))
            .or_else(|| self.tmux_sessions.get(&runtime_key))
            .cloned()
            .unwrap_or_else(|| format!("tmux-process:{}", inferred.process_id));
        self.process_sessions
            .insert((inferred.provider, inferred.process_id), session_id.clone());
        AgentKey {
            worktree,
            provider: inferred.provider,
            session_id,
        }
    }

    pub(crate) fn collect(&mut self, mut observation: AgentObservation) -> AgentObservation {
        if !self.sessions.contains_key(&observation.key)
            && self.sessions.len() >= MAX_TRACKED_SESSIONS
            && let Some(oldest) = self
                .sessions
                .iter()
                .min_by_key(|(_, session)| session.last_observed_at_ms)
                .map(|(key, _)| key.clone())
        {
            self.sessions.remove(&oldest);
            self.process_sessions
                .retain(|_, session_id| session_id != &oldest.session_id);
            self.tmux_sessions
                .retain(|_, session_id| session_id != &oldest.session_id);
        }
        let session = self.sessions.entry(observation.key.clone()).or_default();
        session.last_observed_at_ms = observation.observed_at_ms;
        session.merge(&observation);
        session.enrich(&mut observation);
        observation
    }
}

impl AgentSessionInfo {
    fn merge(&mut self, observation: &AgentObservation) {
        if observation.authority > SourceAuthority::ProcessInference {
            update_field(
                &mut self.lifecycle,
                Some(observation.lifecycle),
                observation,
                Some(LIFECYCLE_GRACE_MS),
            );
            update_field(
                &mut self.relation,
                observation.relation.clone(),
                observation,
                None,
            );
        }
        let label = meaningful_label(observation.label.clone(), observation.key.provider);
        update_field(&mut self.label, label.clone(), observation, None);
        let session_title = meaningful_label(
            observation.metrics.session_title.clone().or(label),
            observation.key.provider,
        );
        update_field(&mut self.session_title, session_title, observation, None);
        update_field(
            &mut self.model_name,
            observation.metrics.model_name.clone(),
            observation,
            None,
        );
        update_field(
            &mut self.token_usage,
            observation.metrics.token_usage,
            observation,
            None,
        );
        update_field(
            &mut self.context_remaining_percent,
            observation.metrics.context_remaining_percent,
            observation,
            None,
        );
        update_field(
            &mut self.activity_at_ms,
            observation.metrics.activity_at_ms,
            observation,
            None,
        );
        self.record_metric_sample(observation);
    }

    fn enrich(&self, observation: &mut AgentObservation) {
        if let Some(lifecycle) = self.lifecycle.as_ref().filter(|lifecycle| {
            lifecycle.observed_at_ms.saturating_add(LIFECYCLE_GRACE_MS)
                >= observation.observed_at_ms
        }) {
            observation.lifecycle = lifecycle.value;
        }
        observation.relation = cached_value(&self.relation).or(observation.relation.take());
        observation.label = cached_value(&self.label).or(observation.label.take());
        observation.metrics = AgentMetrics {
            session_title: cached_value(&self.session_title)
                .or(observation.metrics.session_title.take()),
            model_name: cached_value(&self.model_name).or(observation.metrics.model_name.take()),
            token_usage: cached_value(&self.token_usage).or(observation.metrics.token_usage),
            context_remaining_percent: cached_value(&self.context_remaining_percent)
                .or(observation.metrics.context_remaining_percent),
            activity_at_ms: cached_value(&self.activity_at_ms)
                .or(observation.metrics.activity_at_ms),
        };
    }

    fn record_metric_sample(&mut self, observation: &AgentObservation) {
        if observation.metrics.token_usage.is_none()
            && observation.metrics.context_remaining_percent.is_none()
        {
            return;
        }
        let sample = AgentMetricSample {
            observed_at_ms: observation.observed_at_ms,
            token_usage: observation.metrics.token_usage,
            context_remaining_percent: observation.metrics.context_remaining_percent,
        };
        if self.metric_samples.back().is_some_and(|previous| {
            previous.token_usage == sample.token_usage
                && previous.context_remaining_percent == sample.context_remaining_percent
                && previous
                    .observed_at_ms
                    .saturating_add(DUPLICATE_SAMPLE_INTERVAL_MS)
                    > sample.observed_at_ms
        }) {
            return;
        }
        self.metric_samples.push_back(sample);
        if self.metric_samples.len() > MAX_METRIC_SAMPLES_PER_SESSION {
            self.metric_samples.pop_front();
        }
    }
}

fn update_field<T>(
    target: &mut Option<CachedField<T>>,
    incoming: Option<T>,
    observation: &AgentObservation,
    stale_after_ms: Option<u64>,
) {
    let Some(value) = incoming else {
        return;
    };
    let should_replace = target.as_ref().is_none_or(|current| {
        observation.authority > current.authority
            || (observation.authority == current.authority
                && observation.observed_at_ms >= current.observed_at_ms)
            || stale_after_ms.is_some_and(|stale_after_ms| {
                current.observed_at_ms.saturating_add(stale_after_ms) < observation.observed_at_ms
            })
    });
    if should_replace {
        *target = Some(CachedField {
            value,
            authority: observation.authority,
            observed_at_ms: observation.observed_at_ms,
        });
    }
}

fn cached_value<T: Clone>(field: &Option<CachedField<T>>) -> Option<T> {
    field.as_ref().map(|field| field.value.clone())
}

fn meaningful_label(
    label: Option<BoundedStatusLabel>,
    provider: AgentProvider,
) -> Option<BoundedStatusLabel> {
    label.filter(|label| {
        let value = label.as_str();
        !value.eq_ignore_ascii_case(provider.display_name()) && !value.eq_ignore_ascii_case("agent")
    })
}

fn tmux_runtime_key(provider: AgentProvider, pane: &TmuxPaneRecord) -> TmuxRuntimeKey {
    TmuxRuntimeKey {
        provider,
        socket: pane.socket.clone(),
        pane: pane.pane.clone(),
    }
}

#[cfg(test)]
#[path = "session_info_tests.rs"]
mod tests;
