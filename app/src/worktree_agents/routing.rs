use std::collections::HashMap;

use warpui::EntityId;

use super::launch::NativeResumeTarget;
use super::observation::AgentKey;
use super::pane_index::LaunchCorrelation;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum RouteConfidence {
    Stale,
    Inferred,
    Exact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentRoute {
    ExistingPane {
        terminal_view_id: EntityId,
        correlation: LaunchCorrelation,
    },
    Tmux {
        socket: Option<String>,
        session: String,
        window: String,
        pane: String,
    },
    Herdr {
        session: String,
        workspace: Option<String>,
        tab: String,
        pane: String,
        agent: String,
    },
    NativeResume(NativeResumeTarget),
    ObserveOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RouteRecord {
    pub(crate) source_id: &'static str,
    pub(crate) route: AgentRoute,
    pub(crate) confidence: RouteConfidence,
    pub(crate) validated_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) generation: u64,
}

#[derive(Default)]
pub(crate) struct RouteRegistry {
    routes: HashMap<AgentKey, HashMap<&'static str, RouteRecord>>,
    next_generation: u64,
}

impl RouteRegistry {
    pub(crate) fn publish(
        &mut self,
        key: AgentKey,
        source_id: &'static str,
        route: AgentRoute,
        confidence: RouteConfidence,
        validated_at_ms: u64,
        expires_at_ms: u64,
    ) -> Option<u64> {
        if let Some(current) = self
            .routes
            .get_mut(&key)
            .and_then(|sources| sources.get_mut(source_id))
            && current.route == route
            && current.confidence == confidence
        {
            current.validated_at_ms = validated_at_ms;
            current.expires_at_ms = expires_at_ms;
            return None;
        }
        let previous = self.get(&key).cloned();
        if self
            .routes
            .get(&key)
            .and_then(|sources| sources.get(source_id))
            .is_some_and(|current| {
                confidence < current.confidence && validated_at_ms <= current.expires_at_ms
            })
        {
            return None;
        }

        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        let generation = self.next_generation;
        self.routes.entry(key.clone()).or_default().insert(
            source_id,
            RouteRecord {
                source_id,
                route,
                confidence,
                validated_at_ms,
                expires_at_ms,
                generation,
            },
        );
        (previous.as_ref() != self.get(&key)).then_some(generation)
    }

    #[cfg(test)]
    pub(crate) fn resolve_exact(
        &self,
        key: &AgentKey,
        expected_generation: u64,
        now_ms: u64,
    ) -> Option<&AgentRoute> {
        self.resolve_exact_record(key, expected_generation, now_ms)
            .map(|record| &record.route)
    }

    pub(crate) fn resolve_exact_record(
        &self,
        key: &AgentKey,
        expected_generation: u64,
        now_ms: u64,
    ) -> Option<&RouteRecord> {
        let record = self.get(key)?;
        (record.generation == expected_generation
            && record.confidence == RouteConfidence::Exact
            && record.expires_at_ms > now_ms)
            .then_some(record)
    }

    pub(crate) fn get(&self, key: &AgentKey) -> Option<&RouteRecord> {
        self.routes.get(key).and_then(best_route)
    }

    pub(crate) fn remove_source(&mut self, key: &AgentKey, source_id: &'static str) -> bool {
        let mut removed = false;
        let remove_key = self.routes.get_mut(key).is_some_and(|sources| {
            removed = sources.remove(source_id).is_some();
            sources.is_empty()
        });
        if remove_key {
            self.routes.remove(key);
        }
        removed
    }
}

fn best_route<'a>(sources: &'a HashMap<&'static str, RouteRecord>) -> Option<&'a RouteRecord> {
    sources.values().max_by_key(|record| {
        (
            record.confidence,
            route_priority(&record.route),
            record.validated_at_ms,
        )
    })
}

fn route_priority(route: &AgentRoute) -> u8 {
    match route {
        AgentRoute::ExistingPane { .. } => 5,
        AgentRoute::Tmux { .. } | AgentRoute::Herdr { .. } => 3,
        AgentRoute::NativeResume(_) => 2,
        AgentRoute::ObserveOnly => 1,
    }
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
