use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use serde::Deserialize;
use warpui::r#async::FutureExt as _;

use crate::worktree_agents::observation::AgentLifecycle;

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClaudeAgentRecord {
    pub(crate) session_id: String,
    pub(crate) cwd: String,
    pub(crate) lifecycle: AgentLifecycle,
    pub(crate) label: Option<String>,
    pub(crate) pid: Option<u32>,
    pub(crate) sequence: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ClaudeSnapshotError {
    #[error("Claude Code is unavailable")]
    Unavailable,
    #[error("Claude Code discovery timed out")]
    Timeout,
    #[error("Claude Code discovery failed")]
    Failed,
    #[error("Claude Code discovery exceeded the size limit")]
    TooLarge,
    #[error("Claude Code returned an invalid Agent snapshot")]
    Invalid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawClaudeAgent {
    session_id: String,
    cwd: String,
    #[serde(default)]
    pid: Option<u32>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    started_at: Option<u64>,
}

pub(crate) async fn query_snapshot() -> Result<Vec<ClaudeAgentRecord>, ClaudeSnapshotError> {
    let path = std::env::var("PATH")
        .map(|path| format!("/opt/homebrew/bin:/usr/local/bin:{path}"))
        .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_string());
    let mut command = Command::new("claude");
    command
        .args(["agents", "--json"])
        .env("PATH", path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match command.output().with_timeout(SNAPSHOT_TIMEOUT).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ClaudeSnapshotError::Unavailable);
        }
        Ok(Ok(_)) | Ok(Err(_)) => return Err(ClaudeSnapshotError::Failed),
        Err(_) => return Err(ClaudeSnapshotError::Timeout),
    };
    if output.stdout.len() > MAX_SNAPSHOT_BYTES {
        return Err(ClaudeSnapshotError::TooLarge);
    }
    parse_snapshot(&output.stdout).ok_or(ClaudeSnapshotError::Invalid)
}

pub(crate) fn parse_snapshot(bytes: &[u8]) -> Option<Vec<ClaudeAgentRecord>> {
    let records: Vec<RawClaudeAgent> = serde_json::from_slice(bytes).ok()?;
    Some(
        records
            .into_iter()
            .filter(|record| !record.session_id.trim().is_empty() && !record.cwd.trim().is_empty())
            .map(|record| ClaudeAgentRecord {
                session_id: record.session_id,
                cwd: record.cwd,
                lifecycle: lifecycle(record.state.as_deref(), record.status.as_deref()),
                label: record.name,
                pid: record.pid,
                sequence: record.started_at,
            })
            .collect(),
    )
}

fn lifecycle(state: Option<&str>, status: Option<&str>) -> AgentLifecycle {
    match state.map(str::to_ascii_lowercase).as_deref() {
        Some("starting" | "initializing") => AgentLifecycle::Starting,
        Some("thinking" | "planning") => AgentLifecycle::Thinking,
        Some("working" | "busy" | "running") => AgentLifecycle::Working,
        Some("blocked" | "needs_input" | "waiting") => AgentLifecycle::NeedsInput,
        Some("ready" | "ready_for_review") => AgentLifecycle::ReadyForReview,
        Some("done" | "completed" | "stopped") => AgentLifecycle::Completed,
        Some(_) | None => match status.map(str::to_ascii_lowercase).as_deref() {
            Some("starting" | "initializing") => AgentLifecycle::Starting,
            Some("thinking" | "planning") => AgentLifecycle::Thinking,
            Some("blocked" | "needs_input" | "waiting") => AgentLifecycle::NeedsInput,
            Some("busy" | "working" | "running") => AgentLifecycle::Working,
            Some("done" | "completed" | "stopped") => AgentLifecycle::Completed,
            Some("idle" | "ready") => AgentLifecycle::ReadyForReview,
            Some(_) | None => AgentLifecycle::Stale,
        },
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
