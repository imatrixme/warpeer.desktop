use std::collections::HashMap;

use super::identity::WorktreeKey;
use super::providers::AgentProvider;
use super::runtimes::AgentRuntimeHint;

const MAX_STATUS_LABEL_CHARS: usize = 80;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AgentKey {
    pub(crate) worktree: WorktreeKey,
    pub(crate) provider: AgentProvider,
    pub(crate) session_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentLifecycle {
    Detected,
    Starting,
    Thinking,
    Working,
    NeedsInput,
    ReadyForReview,
    Completed,
    Failed,
    Stopped,
    Stale,
}

impl AgentLifecycle {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stopped)
    }

    pub(crate) fn is_active(self) -> bool {
        !matches!(
            self,
            Self::Completed | Self::Failed | Self::Stopped | Self::Stale
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourceAuthority {
    ProcessInference,
    ProviderSnapshot,
    StructuredEvent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceHealth {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundedStatusLabel(String);

impl BoundedStatusLabel {
    pub(crate) fn new(value: impl AsRef<str>) -> Option<Self> {
        let value: String = value
            .as_ref()
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_STATUS_LABEL_CHARS)
            .collect();
        let value = value.trim();
        (!value.is_empty()).then(|| Self(value.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct AgentMetrics {
    pub(crate) session_title: Option<BoundedStatusLabel>,
    pub(crate) model_name: Option<BoundedStatusLabel>,
    pub(crate) token_usage: Option<u64>,
    pub(crate) context_remaining_percent: Option<u8>,
    pub(crate) activity_at_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentRelation {
    pub(crate) parent_session_id: String,
    pub(crate) role: Option<BoundedStatusLabel>,
}

impl AgentMetrics {
    pub(crate) fn with_session_title(title: Option<BoundedStatusLabel>) -> Self {
        Self {
            session_title: title,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentObservation {
    pub(crate) key: AgentKey,
    pub(crate) lifecycle: AgentLifecycle,
    pub(crate) authority: SourceAuthority,
    pub(crate) source_id: &'static str,
    pub(crate) sequence: Option<u64>,
    pub(crate) observed_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) runtime: AgentRuntimeHint,
    pub(crate) relation: Option<AgentRelation>,
    pub(crate) label: Option<BoundedStatusLabel>,
    pub(crate) metrics: AgentMetrics,
}

#[derive(Clone, Debug)]
struct StoredObservation {
    observation: AgentObservation,
    tombstone: bool,
}

#[derive(Default)]
pub(crate) struct ObservationStore {
    observations: HashMap<AgentKey, HashMap<&'static str, StoredObservation>>,
}

impl ObservationStore {
    pub(crate) fn apply(&mut self, incoming: AgentObservation) -> bool {
        let key = incoming.key.clone();
        let previous = self.get(&key).cloned();
        if previous.as_ref().is_some_and(|current| {
            current.lifecycle.is_terminal()
                && incoming.observed_at_ms <= current.observed_at_ms
                && incoming.authority <= current.authority
        }) {
            return false;
        }

        let sources = self.observations.entry(key.clone()).or_default();
        if let Some(current) = sources.get(incoming.source_id) {
            if current.tombstone && incoming.observed_at_ms <= current.observation.observed_at_ms {
                return false;
            }
            if incoming.sequence.is_some() && incoming.sequence < current.observation.sequence {
                return false;
            }
            if incoming.sequence == current.observation.sequence
                && same_observation_payload(&incoming, &current.observation)
            {
                sources.insert(
                    incoming.source_id,
                    StoredObservation {
                        tombstone: incoming.lifecycle.is_terminal(),
                        observation: incoming,
                    },
                );
                return false;
            }
            if incoming == current.observation {
                return false;
            }
        }
        sources.insert(
            incoming.source_id,
            StoredObservation {
                tombstone: incoming.lifecycle.is_terminal(),
                observation: incoming,
            },
        );
        previous.as_ref() != self.get(&key)
    }

    pub(crate) fn mark_expired(&mut self, now_ms: u64) -> usize {
        let mut changed = 0;
        for sources in self.observations.values_mut() {
            for stored in sources.values_mut() {
                if !stored.tombstone
                    && stored.observation.lifecycle != AgentLifecycle::Stale
                    && stored.observation.expires_at_ms <= now_ms
                {
                    stored.observation.lifecycle = AgentLifecycle::Stale;
                    stored.observation.observed_at_ms = now_ms;
                    changed += 1;
                }
            }
        }
        changed
    }

    pub(crate) fn get(&self, key: &AgentKey) -> Option<&AgentObservation> {
        self.observations.get(key).and_then(best_observation)
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &AgentObservation> {
        self.observations.values().filter_map(best_observation)
    }

    pub(crate) fn remove_source(&mut self, key: &AgentKey, source_id: &'static str) -> bool {
        let previous = self.get(key).cloned();
        let remove_key = self.observations.get_mut(key).is_some_and(|sources| {
            sources.remove(source_id);
            sources.is_empty()
        });
        if remove_key {
            self.observations.remove(key);
        }
        previous.as_ref() != self.get(key)
    }
}

fn best_observation<'a>(
    sources: &'a HashMap<&'static str, StoredObservation>,
) -> Option<&'a AgentObservation> {
    sources
        .values()
        .map(|stored| &stored.observation)
        .max_by_key(|observation| (observation.authority, observation.observed_at_ms))
}

fn same_observation_payload(left: &AgentObservation, right: &AgentObservation) -> bool {
    left.key == right.key
        && left.lifecycle == right.lifecycle
        && left.authority == right.authority
        && left.source_id == right.source_id
        && left.sequence == right.sequence
        && left.runtime == right.runtime
        && left.relation == right.relation
        && left.label == right.label
        && left.metrics == right.metrics
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
