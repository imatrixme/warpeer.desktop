use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use warpui::r#async::FutureExt as _;

use crate::worktree_agents::providers::AgentProvider;

const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024;
const FIELD_SEPARATOR: char = '\u{1f}';
const PANE_FORMAT: &str = "#{socket_path}\u{1f}#{session_name}\u{1f}#{window_id}\u{1f}#{pane_id}\u{1f}#{pane_pid}\u{1f}#{pane_current_path}\u{1f}#{pane_current_command}\u{1f}#{session_attached}";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TmuxPaneRecord {
    pub(crate) socket: Option<String>,
    pub(crate) session: String,
    pub(crate) window: String,
    pub(crate) pane: String,
    pub(crate) pane_pid: u32,
    pub(crate) cwd: String,
    pub(crate) command: String,
    pub(crate) attached: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProcessRecord {
    pub(crate) pid: u32,
    pub(crate) parent_pid: u32,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TmuxProcessSnapshot {
    pub(crate) panes: Vec<TmuxPaneRecord>,
    pub(crate) processes: Vec<ProcessRecord>,
    pub(crate) rollout_paths: HashMap<u32, String>,
    pub(crate) session_ids: HashMap<u32, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InferredTmuxAgent {
    pub(crate) provider: AgentProvider,
    pub(crate) process_id: u32,
    pub(crate) session_id: Option<String>,
    pub(crate) rollout_path: Option<String>,
    pub(crate) pane: TmuxPaneRecord,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TmuxSnapshotError {
    #[error("tmux is unavailable")]
    Unavailable,
    #[error("tmux discovery timed out")]
    Timeout,
    #[error("tmux discovery failed")]
    Failed,
    #[error("tmux discovery exceeded the size limit")]
    TooLarge,
    #[error("tmux returned an invalid pane snapshot")]
    Invalid,
}

pub(crate) async fn query_snapshot() -> Result<TmuxProcessSnapshot, TmuxSnapshotError> {
    let sockets = candidate_sockets(std::env::var("TMUX").ok().as_deref());
    let panes = query_panes(&sockets);
    let process_args = ["-axo", "pid=,ppid=,args="];
    let processes = query_command("ps", &process_args);
    let (panes, processes) = futures::join!(panes, processes);
    let panes = panes?;
    let processes = parse_processes(&processes?).ok_or(TmuxSnapshotError::Invalid)?;
    let artifacts = query_codex_artifacts(&processes).await.unwrap_or_default();
    Ok(TmuxProcessSnapshot {
        panes,
        processes,
        rollout_paths: artifacts.rollout_paths,
        session_ids: artifacts.session_ids,
    })
}

async fn query_panes(sockets: &[Option<String>]) -> Result<Vec<TmuxPaneRecord>, TmuxSnapshotError> {
    let mut panes = Vec::new();
    let mut successful_query = false;
    let mut first_error = None;
    for socket in sockets {
        let mut args = Vec::new();
        if let Some(socket) = socket {
            args.extend(["-S", socket.as_str()]);
        }
        args.extend(["list-panes", "-a", "-F", PANE_FORMAT]);
        match query_command("tmux", &args).await {
            Ok(bytes) => {
                let records = parse_panes(&bytes).ok_or(TmuxSnapshotError::Invalid)?;
                successful_query = true;
                merge_panes(&mut panes, records);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if successful_query {
        Ok(panes)
    } else {
        Err(first_error.unwrap_or(TmuxSnapshotError::Failed))
    }
}

fn merge_panes(panes: &mut Vec<TmuxPaneRecord>, records: Vec<TmuxPaneRecord>) {
    for record in records {
        if !panes
            .iter()
            .any(|current| current.socket == record.socket && current.pane == record.pane)
        {
            panes.push(record);
        }
    }
}

fn candidate_sockets(tmux: Option<&str>) -> Vec<Option<String>> {
    let mut sockets = vec![None];
    if let Some(socket) = tmux
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sockets.push(Some(socket.to_string()));
    }
    sockets
}

async fn query_command(program: &str, args: &[&str]) -> Result<Vec<u8>, TmuxSnapshotError> {
    let path = std::env::var("PATH")
        .map(|path| format!("/opt/homebrew/bin:/usr/local/bin:{path}"))
        .unwrap_or_else(|_| "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin".to_string());
    let mut command = Command::new(program);
    command
        .args(args)
        .env("PATH", path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = match command.output().with_timeout(SNAPSHOT_TIMEOUT).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TmuxSnapshotError::Unavailable);
        }
        Ok(Ok(_)) | Ok(Err(_)) => return Err(TmuxSnapshotError::Failed),
        Err(_) => return Err(TmuxSnapshotError::Timeout),
    };
    if output.stdout.len() > MAX_SNAPSHOT_BYTES {
        return Err(TmuxSnapshotError::TooLarge);
    }
    Ok(output.stdout)
}

pub(crate) fn pane_for_process(
    snapshot: &TmuxProcessSnapshot,
    process_id: u32,
) -> Option<&TmuxPaneRecord> {
    let parents: HashMap<_, _> = snapshot
        .processes
        .iter()
        .map(|process| (process.pid, process.parent_pid))
        .collect();
    snapshot.panes.iter().find(|pane| {
        let mut current = Some(process_id);
        let mut remaining = 128;
        while let Some(pid) = current {
            if pid == pane.pane_pid {
                return true;
            }
            if remaining == 0 {
                return false;
            }
            remaining -= 1;
            current = parents.get(&pid).copied().filter(|parent| *parent != pid);
        }
        false
    })
}

pub(crate) fn infer_agents(snapshot: &TmuxProcessSnapshot) -> Vec<InferredTmuxAgent> {
    let mut agents = Vec::new();
    for process in &snapshot.processes {
        let Some(provider) = provider_from_command(&process.command) else {
            continue;
        };
        let Some(pane) = pane_for_process(snapshot, process.pid) else {
            continue;
        };
        let candidate = InferredTmuxAgent {
            provider,
            process_id: process.pid,
            session_id: session_id_from_command(provider, &process.command)
                .or_else(|| snapshot.session_ids.get(&process.pid).cloned()),
            rollout_path: snapshot.rollout_paths.get(&process.pid).cloned(),
            pane: pane.clone(),
        };
        if let Some(existing) = agents.iter_mut().find(|agent: &&mut InferredTmuxAgent| {
            agent.pane.socket == pane.socket
                && agent.pane.pane == pane.pane
                && agent.provider == provider
        }) {
            if candidate.session_id.is_some() || candidate.rollout_path.is_some() {
                *existing = candidate;
            }
        } else {
            agents.push(candidate);
        }
    }
    agents
}

fn provider_from_command(command: &str) -> Option<AgentProvider> {
    AgentProvider::from_command(command)
}

fn session_id_from_command(provider: AgentProvider, command: &str) -> Option<String> {
    if provider != AgentProvider::Codex {
        return None;
    }
    let mut arguments = command.split_whitespace();
    arguments
        .find(|argument| *argument == "resume")
        .and_then(|_| arguments.next())
        .filter(|session_id| !session_id.starts_with('-'))
        .map(str::to_string)
}

#[derive(Default)]
pub(crate) struct CodexProcessArtifacts {
    pub(crate) rollout_paths: HashMap<u32, String>,
    pub(crate) session_ids: HashMap<u32, String>,
}

async fn query_codex_artifacts(
    processes: &[ProcessRecord],
) -> Result<CodexProcessArtifacts, TmuxSnapshotError> {
    let pids = processes
        .iter()
        .filter(|process| provider_from_command(&process.command) == Some(AgentProvider::Codex))
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>();
    if pids.is_empty() {
        return Ok(CodexProcessArtifacts::default());
    }
    let pid_list = pids.join(",");
    let args = ["-a", "-p", pid_list.as_str(), "-Fn"];
    query_command("lsof", &args)
        .await
        .map(|bytes| parse_codex_artifacts(&bytes))
}

pub(crate) fn parse_codex_artifacts(bytes: &[u8]) -> CodexProcessArtifacts {
    let Some(output) = std::str::from_utf8(bytes).ok() else {
        return CodexProcessArtifacts::default();
    };
    let mut artifacts = CodexProcessArtifacts::default();
    let mut current_pid = None;
    for line in output.lines() {
        match line.as_bytes().first() {
            Some(b'p') => current_pid = line[1..].parse().ok(),
            Some(b'n') if line.contains("/rollout-") && line.ends_with(".jsonl") => {
                if let Some(pid) = current_pid {
                    artifacts
                        .rollout_paths
                        .entry(pid)
                        .or_insert_with(|| line[1..].to_string());
                }
            }
            Some(b'n') if line.contains("/thread-writer-locks/") && line.ends_with(".lock") => {
                if let Some(pid) = current_pid
                    && let Some(session_id) = line[1..]
                        .rsplit('/')
                        .next()
                        .and_then(|name| name.strip_suffix(".lock"))
                        .filter(|session_id| !session_id.is_empty())
                {
                    artifacts
                        .session_ids
                        .entry(pid)
                        .or_insert_with(|| session_id.to_string());
                }
            }
            Some(_) | None => {}
        }
    }
    artifacts
}

pub(crate) fn parse_panes(bytes: &[u8]) -> Option<Vec<TmuxPaneRecord>> {
    let output = std::str::from_utf8(bytes).ok()?;
    Some(
        output
            .lines()
            .filter_map(|line| {
                let fields: Vec<_> = line.split(FIELD_SEPARATOR).collect();
                let [
                    socket,
                    session,
                    window,
                    pane,
                    pane_pid,
                    cwd,
                    command,
                    attached,
                ] = fields.as_slice()
                else {
                    return None;
                };
                Some(TmuxPaneRecord {
                    socket: (!socket.is_empty()).then(|| (*socket).to_string()),
                    session: (*session).to_string(),
                    window: (*window).to_string(),
                    pane: (*pane).to_string(),
                    pane_pid: pane_pid.parse().ok()?,
                    cwd: (*cwd).to_string(),
                    command: (*command).to_string(),
                    attached: attached.parse::<u32>().ok()? > 0,
                })
            })
            .collect(),
    )
}

pub(crate) fn parse_processes(bytes: &[u8]) -> Option<Vec<ProcessRecord>> {
    let output = std::str::from_utf8(bytes).ok()?;
    Some(
        output
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                Some(ProcessRecord {
                    pid: fields.next()?.parse().ok()?,
                    parent_pid: fields.next()?.parse().ok()?,
                    command: fields.collect::<Vec<_>>().join(" "),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
#[path = "tmux_tests.rs"]
mod tests;
