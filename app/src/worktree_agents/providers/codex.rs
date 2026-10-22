use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use command::r#async::Command;
use futures_lite::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader,
};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::{Value, json};
use warpui::r#async::FutureExt as _;

use super::codex_rollout;
use crate::worktree_agents::observation::{AgentLifecycle, AgentRelation, BoundedStatusLabel};

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);
const APP_SERVER_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const APP_SERVER_RETRY_INTERVAL: Duration = Duration::from_secs(12);
const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_ACTIVE_THREADS: usize = 96;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CodexAgentRecord {
    pub(crate) session_id: String,
    pub(crate) cwd: String,
    pub(crate) lifecycle: AgentLifecycle,
    pub(crate) label: Option<String>,
    pub(crate) sequence: Option<u64>,
    pub(crate) rollout_path: Option<String>,
    pub(crate) model_name: Option<String>,
    pub(crate) token_usage: Option<u64>,
    pub(crate) context_remaining_percent: Option<u8>,
    pub(crate) activity_unix_ms: Option<u64>,
    pub(crate) rollout_derived: bool,
    pub(crate) relation: Option<AgentRelation>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CodexActiveHints {
    pub(crate) session_ids: HashSet<String>,
    pub(crate) rollout_paths: HashSet<String>,
}

#[derive(Clone, Default)]
pub(crate) struct CodexSessionCollector {
    state: Arc<Mutex<CodexSessionCollectorState>>,
}

#[derive(Default)]
struct CodexSessionCollectorState {
    threads: Vec<RawThread>,
    last_refresh: Option<Instant>,
    last_attempt: Option<Instant>,
    rollouts: codex_rollout::CodexRolloutCache,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CodexSnapshotError {
    #[error("Codex is unavailable")]
    Unavailable,
    #[error("Codex discovery timed out")]
    Timeout,
    #[error("Codex discovery failed")]
    Failed,
    #[error("Codex App Server returned an oversized message")]
    TooLarge,
    #[error("Codex App Server returned an invalid response")]
    Invalid,
}

#[derive(Deserialize)]
struct ThreadList {
    data: Vec<RawThread>,
}

#[cfg(test)]
#[derive(Deserialize)]
struct ThreadRead {
    thread: RawThread,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawThread {
    id: String,
    cwd: String,
    #[serde(default)]
    parent_thread_id: Option<String>,
    #[serde(default)]
    agent_nickname: Option<String>,
    #[serde(default)]
    agent_role: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    first_user_message: String,
    #[serde(default)]
    preview: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    updated_at: Option<i64>,
    status: Value,
}

impl CodexSessionCollector {
    pub(crate) async fn query_snapshot(
        &self,
        worktree_roots: &[PathBuf],
        active_hints: &CodexActiveHints,
    ) -> Result<Vec<CodexAgentRecord>, CodexSnapshotError> {
        let now = Instant::now();
        let should_refresh = self.state.lock().should_refresh(now, active_hints);
        let refresh = if should_refresh {
            self.state.lock().last_attempt = Some(now);
            Some(query_threads().await)
        } else {
            None
        };
        let mut state = self.state.lock();
        let mut refresh_error = None;
        if let Some(refresh) = refresh {
            match refresh {
                Ok(threads) => {
                    state.threads = threads;
                    state.last_refresh = Some(now);
                }
                Err(error) => refresh_error = Some(error),
            }
        }
        if state.threads.is_empty() {
            return Err(refresh_error.unwrap_or(CodexSnapshotError::Failed));
        }
        let threads = state.threads.clone();
        let mut records = Vec::new();
        for thread in threads
            .into_iter()
            .filter(|thread| relevant_worktree(&thread.cwd, worktree_roots))
        {
            let summarize_rollout = active_hints.session_ids.contains(&thread.id)
                || thread
                    .path
                    .as_ref()
                    .is_some_and(|path| active_hints.rollout_paths.contains(path));
            if let Some(record) = parse_thread(thread, summarize_rollout, &mut state.rollouts) {
                records.push(record);
            }
        }
        Ok(records)
    }
}

impl CodexSessionCollectorState {
    fn should_refresh(&self, now: Instant, active_hints: &CodexActiveHints) -> bool {
        if self.last_attempt.is_some_and(|last_attempt| {
            now.checked_duration_since(last_attempt)
                .is_some_and(|elapsed| elapsed < APP_SERVER_RETRY_INTERVAL)
        }) {
            return false;
        }
        if self.threads.is_empty() || !self.covers(active_hints) {
            return true;
        }
        self.last_refresh.is_none_or(|last_refresh| {
            now.checked_duration_since(last_refresh)
                .is_none_or(|elapsed| elapsed >= APP_SERVER_REFRESH_INTERVAL)
        })
    }

    fn covers(&self, active_hints: &CodexActiveHints) -> bool {
        active_hints
            .session_ids
            .iter()
            .all(|session_id| self.threads.iter().any(|thread| thread.id == *session_id))
            && active_hints.rollout_paths.iter().all(|rollout_path| {
                self.threads
                    .iter()
                    .any(|thread| thread.path.as_deref() == Some(rollout_path))
            })
    }
}

async fn query_threads() -> Result<Vec<RawThread>, CodexSnapshotError> {
    match query_threads_inner().with_timeout(SNAPSHOT_TIMEOUT).await {
        Ok(result) => result,
        Err(_) => Err(CodexSnapshotError::Timeout),
    }
}

async fn query_threads_inner() -> Result<Vec<RawThread>, CodexSnapshotError> {
    let path = std::env::var("PATH")
        .map(|path| format!("/opt/homebrew/bin:/usr/local/bin:{path}"))
        .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_string());
    let mut command = Command::new_with_process_group("codex");
    command
        .args(["app-server", "--listen", "stdio://"])
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            CodexSnapshotError::Unavailable
        } else {
            CodexSnapshotError::Failed
        }
    })?;
    let stdin = child.stdin.take().ok_or(CodexSnapshotError::Failed)?;
    let stdout = child.stdout.take().ok_or(CodexSnapshotError::Failed)?;
    let mut rpc = CodexRpc::new(stdin, stdout);
    rpc.request(
        "initialize",
        json!({
            "clientInfo": {
                "name": "warpeer",
                "title": "Warpeer",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
    .await?;
    rpc.notify("initialized", json!({})).await?;
    let threads: ThreadList = serde_json::from_value(
        rpc.request(
            "thread/list",
            json!({
                "archived": false,
                "limit": MAX_ACTIVE_THREADS,
                "sortKey": "updated_at",
                "sortDirection": "desc",
                "sourceKinds": [
                    "cli",
                    "subAgent",
                    "subAgentReview",
                    "subAgentCompact",
                    "subAgentThreadSpawn",
                    "subAgentOther"
                ],
                "useStateDbOnly": false
            }),
        )
        .await?,
    )
    .map_err(|_| CodexSnapshotError::Invalid)?;
    Ok(threads.data)
}

struct CodexRpc<W, R> {
    stdin: W,
    stdout: BufReader<R>,
    next_id: u64,
}

impl<W, R> CodexRpc<W, R>
where
    W: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    fn new(stdin: W, stdout: R) -> Self {
        Self {
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        }
    }

    async fn request(&mut self, method: &str, params: Value) -> Result<Value, CodexSnapshotError> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.send(json!({"method": method, "id": id, "params": params}))
            .await?;
        loop {
            let message = self.read_message().await?;
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if message.get("error").is_some() {
                return Err(CodexSnapshotError::Failed);
            }
            return message
                .get("result")
                .cloned()
                .ok_or(CodexSnapshotError::Invalid);
        }
    }

    async fn notify(&mut self, method: &str, params: Value) -> Result<(), CodexSnapshotError> {
        self.send(json!({"method": method, "params": params})).await
    }

    async fn send(&mut self, message: Value) -> Result<(), CodexSnapshotError> {
        let mut bytes = serde_json::to_vec(&message).map_err(|_| CodexSnapshotError::Invalid)?;
        bytes.push(b'\n');
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|_| CodexSnapshotError::Failed)?;
        self.stdin
            .flush()
            .await
            .map_err(|_| CodexSnapshotError::Failed)
    }

    async fn read_message(&mut self) -> Result<Value, CodexSnapshotError> {
        let mut line = String::new();
        let count = self
            .stdout
            .read_line(&mut line)
            .await
            .map_err(|_| CodexSnapshotError::Failed)?;
        if count == 0 {
            return Err(CodexSnapshotError::Failed);
        }
        if line.len() > MAX_MESSAGE_BYTES {
            return Err(CodexSnapshotError::TooLarge);
        }
        serde_json::from_str(&line).map_err(|_| CodexSnapshotError::Invalid)
    }
}

#[cfg(test)]
pub(crate) fn parse_thread_read(value: Value) -> Option<CodexAgentRecord> {
    let response: ThreadRead = serde_json::from_value(value).ok()?;
    parse_thread(
        response.thread,
        true,
        &mut codex_rollout::CodexRolloutCache::default(),
    )
}

fn parse_thread(
    thread: RawThread,
    summarize_rollout: bool,
    rollouts: &mut codex_rollout::CodexRolloutCache,
) -> Option<CodexAgentRecord> {
    if thread.id.trim().is_empty() || thread.cwd.trim().is_empty() {
        return None;
    }
    let rollout = summarize_rollout
        .then(|| {
            thread
                .path
                .as_deref()
                .and_then(|path| rollouts.summarize(Path::new(path)))
        })
        .flatten();
    let label = thread
        .name
        .as_deref()
        .and_then(bounded_preview)
        .or_else(|| thread.title.as_deref().and_then(bounded_preview))
        .or_else(|| bounded_preview(&thread.first_user_message))
        .or_else(|| bounded_preview(&thread.preview))
        .or_else(|| rollout.as_ref().and_then(|summary| summary.title.clone()));
    Some(CodexAgentRecord {
        session_id: thread.id,
        cwd: thread.cwd,
        lifecycle: rollout
            .as_ref()
            .and_then(|summary| summary.lifecycle)
            .unwrap_or_else(|| lifecycle(&thread.status)),
        label,
        sequence: thread
            .updated_at
            .and_then(|value| u64::try_from(value).ok()),
        rollout_path: thread.path,
        model_name: rollout
            .as_ref()
            .and_then(|summary| summary.model_name.clone()),
        token_usage: rollout.as_ref().and_then(|summary| summary.token_usage),
        context_remaining_percent: rollout
            .as_ref()
            .and_then(|summary| summary.context_remaining_percent),
        activity_unix_ms: rollout
            .as_ref()
            .and_then(|summary| summary.activity_unix_ms)
            .or_else(|| {
                thread
                    .updated_at
                    .and_then(|value| u64::try_from(value).ok())
                    .map(|value| value.saturating_mul(1_000))
            }),
        rollout_derived: rollout.is_some(),
        relation: thread.parent_thread_id.and_then(|parent_session_id| {
            (!parent_session_id.trim().is_empty()).then(|| AgentRelation {
                parent_session_id,
                role: thread
                    .agent_role
                    .as_deref()
                    .and_then(BoundedStatusLabel::new)
                    .or_else(|| {
                        thread
                            .agent_nickname
                            .as_deref()
                            .and_then(BoundedStatusLabel::new)
                    }),
            })
        }),
    })
}

fn relevant_worktree(cwd: &str, worktree_roots: &[PathBuf]) -> bool {
    worktree_roots
        .iter()
        .any(|root| Path::new(cwd).starts_with(root))
}

fn bounded_preview(preview: &str) -> Option<String> {
    let value: String = preview
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect();
    (!value.is_empty()).then_some(value)
}

fn lifecycle(status: &Value) -> AgentLifecycle {
    match status.get("type").and_then(Value::as_str) {
        Some("active") => {
            let waiting = status
                .get("activeFlags")
                .and_then(Value::as_array)
                .is_some_and(|flags| {
                    flags.iter().any(|flag| {
                        matches!(
                            flag.as_str(),
                            Some("waitingOnApproval" | "waitingOnUserInput")
                        )
                    })
                });
            if waiting {
                AgentLifecycle::NeedsInput
            } else {
                AgentLifecycle::Working
            }
        }
        Some("idle") => AgentLifecycle::ReadyForReview,
        Some("systemError") => AgentLifecycle::Failed,
        Some("notLoaded") | Some(_) | None => AgentLifecycle::Detected,
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
