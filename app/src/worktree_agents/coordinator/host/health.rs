use crate::worktree_agents::observation::SourceHealth;
use crate::worktree_agents::providers::claude::{ClaudeAgentRecord, ClaudeSnapshotError};
use crate::worktree_agents::providers::codex::{CodexAgentRecord, CodexSnapshotError};
use crate::worktree_agents::providers::tmux::{TmuxProcessSnapshot, TmuxSnapshotError};
use crate::worktree_agents::sources::{
    ObservationSourceSpec, SourceCapabilities, SourceDiagnostic,
};

pub(super) fn source_spec(id: &'static str, expiry_ms: u64) -> ObservationSourceSpec {
    ObservationSourceSpec {
        id,
        capabilities: SourceCapabilities {
            progressive_snapshots: true,
            structured_events: false,
            diagnostics: true,
            cancellation: true,
            capability_detection: true,
        },
        foreground_interval_ms: 6_000,
        background_interval_ms: 30_000,
        expiry_ms,
    }
}

pub(super) fn claude_health(
    result: &Result<Vec<ClaudeAgentRecord>, ClaudeSnapshotError>,
) -> (SourceHealth, Option<SourceDiagnostic>) {
    match result {
        Ok(_) => (SourceHealth::Healthy, None),
        Err(ClaudeSnapshotError::Unavailable) => unavailable_health(),
        Err(ClaudeSnapshotError::Timeout) => timed_out_health(),
        Err(ClaudeSnapshotError::Failed)
        | Err(ClaudeSnapshotError::TooLarge)
        | Err(ClaudeSnapshotError::Invalid) => failed_health(),
    }
}

pub(super) fn codex_health(
    result: &Result<Vec<CodexAgentRecord>, CodexSnapshotError>,
) -> (SourceHealth, Option<SourceDiagnostic>) {
    match result {
        Ok(_) => (SourceHealth::Healthy, None),
        Err(CodexSnapshotError::Unavailable) => unavailable_health(),
        Err(CodexSnapshotError::Timeout) => timed_out_health(),
        Err(CodexSnapshotError::Failed)
        | Err(CodexSnapshotError::TooLarge)
        | Err(CodexSnapshotError::Invalid) => failed_health(),
    }
}

pub(super) fn tmux_health(
    result: &Result<TmuxProcessSnapshot, TmuxSnapshotError>,
) -> (SourceHealth, Option<SourceDiagnostic>) {
    match result {
        Ok(_) => (SourceHealth::Healthy, None),
        Err(TmuxSnapshotError::Unavailable) => unavailable_health(),
        Err(TmuxSnapshotError::Timeout) => timed_out_health(),
        Err(TmuxSnapshotError::Failed)
        | Err(TmuxSnapshotError::TooLarge)
        | Err(TmuxSnapshotError::Invalid) => failed_health(),
    }
}

fn unavailable_health() -> (SourceHealth, Option<SourceDiagnostic>) {
    (
        SourceHealth::Unavailable,
        Some(SourceDiagnostic::Unavailable),
    )
}

fn timed_out_health() -> (SourceHealth, Option<SourceDiagnostic>) {
    (SourceHealth::Degraded, Some(SourceDiagnostic::TimedOut))
}

fn failed_health() -> (SourceHealth, Option<SourceDiagnostic>) {
    (SourceHealth::Degraded, Some(SourceDiagnostic::Failed))
}
