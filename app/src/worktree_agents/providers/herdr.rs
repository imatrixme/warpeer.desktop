use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use serde::Deserialize;
use warpui::r#async::FutureExt as _;

use crate::worktree_agents::observation::AgentLifecycle;
use crate::worktree_agents::providers::AgentProvider;

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
const MIN_EXACT_AGENT_ROUTE_PROTOCOL: u64 = 19;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HerdrSnapshot {
    pub(crate) version: String,
    pub(crate) protocol: u64,
    pub(crate) agents: Vec<HerdrAgentRecord>,
}

impl HerdrSnapshot {
    pub(crate) fn supports_exact_agent_routes(&self) -> bool {
        self.protocol >= MIN_EXACT_AGENT_ROUTE_PROTOCOL
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HerdrAgentRecord {
    pub(crate) provider: AgentProvider,
    pub(crate) provider_session_id: String,
    pub(crate) cwd: Option<String>,
    pub(crate) lifecycle: AgentLifecycle,
    pub(crate) label: Option<String>,
    pub(crate) sequence: u64,
    pub(crate) terminal_id: String,
    pub(crate) workspace_id: String,
    pub(crate) tab_id: String,
    pub(crate) pane_id: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum HerdrSnapshotError {
    #[error("Herdr is unavailable")]
    Unavailable,
    #[error("Herdr snapshot timed out")]
    Timeout,
    #[error("Herdr snapshot failed")]
    Failed,
    #[error("Herdr snapshot exceeded the size limit")]
    TooLarge,
    #[error("Herdr returned an invalid snapshot")]
    Invalid,
}

#[derive(Deserialize)]
struct ApiEnvelope {
    result: ApiResult,
}

#[derive(Deserialize)]
struct ApiResult {
    snapshot: RawSnapshot,
}

#[derive(Deserialize)]
struct RawSnapshot {
    version: String,
    protocol: u64,
    #[serde(default)]
    agents: Vec<RawAgent>,
    #[serde(default)]
    panes: Vec<RawAgent>,
}

#[derive(Clone, Deserialize)]
struct RawAgent {
    terminal_id: String,
    workspace_id: String,
    tab_id: String,
    pane_id: String,
    agent_status: String,
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    display_agent: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    foreground_cwd: Option<String>,
    #[serde(default)]
    agent_session: Option<RawAgentSession>,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    state_change_seq: Option<u64>,
}

#[derive(Clone, Deserialize)]
struct RawAgentSession {
    agent: String,
    value: String,
}

pub(crate) async fn query_snapshot() -> Result<HerdrSnapshot, HerdrSnapshotError> {
    let path = std::env::var("PATH")
        .map(|path| format!("/opt/homebrew/bin:/usr/local/bin:{path}"))
        .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_string());
    let mut command = Command::new("herdr");
    command
        .args(["api", "snapshot"])
        .env("PATH", path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match command.output().with_timeout(SNAPSHOT_TIMEOUT).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HerdrSnapshotError::Unavailable);
        }
        Ok(Ok(_)) | Ok(Err(_)) => return Err(HerdrSnapshotError::Failed),
        Err(_) => return Err(HerdrSnapshotError::Timeout),
    };
    if output.stdout.len() > MAX_SNAPSHOT_BYTES {
        return Err(HerdrSnapshotError::TooLarge);
    }
    parse_snapshot(&output.stdout).ok_or(HerdrSnapshotError::Invalid)
}

pub(crate) fn parse_snapshot(bytes: &[u8]) -> Option<HerdrSnapshot> {
    let raw: ApiEnvelope = serde_json::from_slice(bytes).ok()?;
    let mut agents = HashMap::new();
    for agent in raw.result.snapshot.panes {
        if should_include(&agent) {
            agents.insert(agent.terminal_id.clone(), normalize(agent));
        }
    }
    for agent in raw.result.snapshot.agents {
        agents.insert(agent.terminal_id.clone(), normalize(agent));
    }
    let mut agents: Vec<_> = agents.into_values().collect();
    agents.sort_by(|left, right| left.terminal_id.cmp(&right.terminal_id));
    Some(HerdrSnapshot {
        version: raw.result.snapshot.version,
        protocol: raw.result.snapshot.protocol,
        agents,
    })
}

fn should_include(agent: &RawAgent) -> bool {
    agent.agent.is_some()
        || agent.agent_session.is_some()
        || !agent.agent_status.eq_ignore_ascii_case("unknown")
}

fn normalize(agent: RawAgent) -> HerdrAgentRecord {
    let provider_name = agent
        .agent_session
        .as_ref()
        .map(|session| session.agent.as_str())
        .or(agent.agent.as_deref())
        .or(agent.display_agent.as_deref())
        .unwrap_or("herdr");
    let provider = provider_from_name(provider_name);
    let provider_session_id = agent
        .agent_session
        .as_ref()
        .map(|session| session.value.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("herdr:{}", agent.terminal_id));
    HerdrAgentRecord {
        provider,
        provider_session_id,
        cwd: agent.foreground_cwd.or(agent.cwd),
        lifecycle: lifecycle_from_status(&agent.agent_status),
        label: agent.name.or(agent.display_agent).or(agent.agent),
        sequence: agent.state_change_seq.unwrap_or(agent.revision),
        terminal_id: agent.terminal_id,
        workspace_id: agent.workspace_id,
        tab_id: agent.tab_id,
        pane_id: agent.pane_id,
    }
}

fn provider_from_name(name: &str) -> AgentProvider {
    AgentProvider::from_name(name)
}

fn lifecycle_from_status(status: &str) -> AgentLifecycle {
    match status.to_ascii_lowercase().as_str() {
        "working" => AgentLifecycle::Working,
        "blocked" => AgentLifecycle::NeedsInput,
        "idle" => AgentLifecycle::ReadyForReview,
        "done" => AgentLifecycle::Completed,
        "unknown" => AgentLifecycle::Stale,
        _ => AgentLifecycle::Stale,
    }
}

#[cfg(test)]
#[path = "herdr_tests.rs"]
mod tests;
