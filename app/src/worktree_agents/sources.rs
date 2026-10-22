use crate::worktree_agents::observation::SourceHealth;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceVisibility {
    Foreground,
    Background,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceCapabilities {
    pub(crate) progressive_snapshots: bool,
    pub(crate) structured_events: bool,
    pub(crate) diagnostics: bool,
    pub(crate) cancellation: bool,
    pub(crate) capability_detection: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObservationSourceSpec {
    pub(crate) id: &'static str,
    pub(crate) capabilities: SourceCapabilities,
    pub(crate) foreground_interval_ms: u64,
    pub(crate) background_interval_ms: u64,
    pub(crate) expiry_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceDiagnostic {
    TimedOut,
    Unavailable,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceLease {
    generation: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceScheduler {
    spec: ObservationSourceSpec,
    health: SourceHealth,
    diagnostic: Option<SourceDiagnostic>,
    generation: u64,
    in_flight: Option<u64>,
    next_poll_at_ms: u64,
}

impl SourceScheduler {
    pub(crate) fn new(spec: ObservationSourceSpec) -> Self {
        Self {
            spec,
            health: SourceHealth::Unavailable,
            diagnostic: None,
            generation: 0,
            in_flight: None,
            next_poll_at_ms: 0,
        }
    }

    pub(crate) fn begin(
        &mut self,
        now_ms: u64,
        visibility: Option<SourceVisibility>,
        manual: bool,
    ) -> Option<SourceLease> {
        visibility?;
        if self.in_flight.is_some() || (!manual && now_ms < self.next_poll_at_ms) {
            return None;
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.in_flight = Some(self.generation);
        Some(SourceLease {
            generation: self.generation,
        })
    }

    pub(crate) fn complete(
        &mut self,
        lease: SourceLease,
        health: SourceHealth,
        diagnostic: Option<SourceDiagnostic>,
        now_ms: u64,
        visibility: SourceVisibility,
    ) -> bool {
        if self.in_flight != Some(lease.generation) || self.generation != lease.generation {
            return false;
        }
        self.in_flight = None;
        self.health = health;
        self.diagnostic = diagnostic;
        let cadence = match visibility {
            SourceVisibility::Foreground => self.spec.foreground_interval_ms,
            SourceVisibility::Background => self.spec.background_interval_ms,
        };
        let backoff = match health {
            SourceHealth::Healthy => 0,
            SourceHealth::Degraded => 30_000,
            SourceHealth::Unavailable => 60_000,
        };
        self.next_poll_at_ms = now_ms.saturating_add(cadence.max(backoff));
        true
    }

    pub(crate) fn cancel(&mut self, diagnostic: SourceDiagnostic) {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.in_flight = None;
        self.diagnostic = Some(diagnostic);
    }

    pub(crate) fn health(&self) -> SourceHealth {
        self.health
    }

    #[cfg(test)]
    pub(crate) fn diagnostic(&self) -> Option<SourceDiagnostic> {
        self.diagnostic
    }

    pub(crate) fn spec(&self) -> ObservationSourceSpec {
        self.spec
    }
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
