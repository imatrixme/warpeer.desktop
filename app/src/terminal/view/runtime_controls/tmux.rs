use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use command::r#async::Command;
use warpui::r#async::FutureExt as _;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
const CLIENT_FIELD_SEPARATOR: char = '\u{1f}';
const CLIENT_FORMAT: &str =
    "#{client_name}\u{1f}#{client_session}\u{1f}#{client_activity}\u{1f}#{client_pid}";

pub(super) struct TmuxSwitchTarget {
    pub(super) socket: Option<String>,
    pub(super) source_session: String,
    pub(super) target_session: String,
    pub(super) target_window: String,
    pub(super) target_pane: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum TmuxSwitchError {
    #[error("tmux is unavailable")]
    Unavailable,
    #[error("tmux command timed out")]
    Timeout,
    #[error("tmux command failed")]
    Failed,
    #[error("the current tmux client could not be identified")]
    ClientNotFound,
}

#[derive(Debug, Eq, PartialEq)]
struct TmuxClient {
    name: String,
    session: String,
    activity: u64,
    process_id: u32,
}

pub(super) async fn switch_client(target: TmuxSwitchTarget) -> Result<(), TmuxSwitchError> {
    let clients = run_tmux(
        target.socket.as_deref(),
        &["list-clients", "-F", CLIENT_FORMAT],
    )
    .await?;
    let process_sessions = client_process_sessions(&clients).await.unwrap_or_default();
    let client = current_client(&clients, &target.source_session, &process_sessions)
        .ok_or(TmuxSwitchError::ClientNotFound)?;
    run_tmux(
        target.socket.as_deref(),
        &[
            "display-message",
            "-p",
            "-t",
            &target.target_pane,
            "#{pane_id}",
            ";",
            "select-window",
            "-t",
            &target.target_window,
            ";",
            "select-pane",
            "-t",
            &target.target_pane,
            ";",
            "switch-client",
            "-c",
            &client.name,
            "-t",
            &target.target_session,
        ],
    )
    .await?;
    Ok(())
}

pub(super) fn attached_session_from_command(command: &str) -> Option<String> {
    let tokens = shell_words::split(command).ok()?;
    let mut session = None;
    for (index, token) in tokens.iter().enumerate() {
        let option = match token.as_str() {
            "attach" | "attach-session" | "switch-client" => "-t",
            "new" | "new-session" => "-s",
            _ => continue,
        };
        session = option_after(&tokens[index + 1..], option).or(session);
    }
    session
}

fn option_after(tokens: &[String], option: &str) -> Option<String> {
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.as_str(), ";" | "&&" | "||") {
            return None;
        }
        if token == option {
            return tokens.get(index + 1).cloned();
        }
        if let Some(value) = token
            .strip_prefix(option)
            .map(|value| value.strip_prefix('=').unwrap_or(value))
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn current_client(
    output: &[u8],
    source_session: &str,
    process_sessions: &HashMap<u32, String>,
) -> Option<TmuxClient> {
    let output = std::str::from_utf8(output).ok()?;
    output
        .lines()
        .filter_map(parse_client)
        .filter(|client| {
            process_sessions.get(&client.process_id).map(String::as_str) == Some(source_session)
                || client.session == source_session
        })
        .max_by_key(|client| client.activity)
}

fn parse_client(line: &str) -> Option<TmuxClient> {
    let mut fields = line.split(CLIENT_FIELD_SEPARATOR);
    let client = TmuxClient {
        name: fields.next()?.to_string(),
        session: fields.next()?.to_string(),
        activity: fields.next()?.parse().ok()?,
        process_id: fields.next()?.parse().ok()?,
    };
    fields.next().is_none().then_some(client)
}

async fn client_process_sessions(output: &[u8]) -> Result<HashMap<u32, String>, TmuxSwitchError> {
    let clients = std::str::from_utf8(output)
        .ok()
        .into_iter()
        .flat_map(str::lines)
        .filter_map(parse_client)
        .collect::<Vec<_>>();
    if clients.is_empty() {
        return Ok(HashMap::new());
    }
    let process_ids = clients
        .iter()
        .map(|client| client.process_id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let output = run_command("ps", &["-p", &process_ids, "-o", "pid=,args="]).await?;
    Ok(parse_process_sessions(&output))
}

fn parse_process_sessions(output: &[u8]) -> HashMap<u32, String> {
    let Some(output) = std::str::from_utf8(output).ok() else {
        return HashMap::new();
    };
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let split_at = line.find(char::is_whitespace)?;
            let process_id = line[..split_at].parse().ok()?;
            let session = attached_session_from_command(line[split_at..].trim())?;
            Some((process_id, session))
        })
        .collect()
}

async fn run_tmux(socket: Option<&str>, args: &[&str]) -> Result<Vec<u8>, TmuxSwitchError> {
    let mut command_args = Vec::new();
    if let Some(socket) = socket {
        command_args.extend(["-S", socket]);
    }
    command_args.extend(args);
    run_command("tmux", &command_args).await
}

async fn run_command(program: &str, args: &[&str]) -> Result<Vec<u8>, TmuxSwitchError> {
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
    let output = match command.output().with_timeout(COMMAND_TIMEOUT).await {
        Ok(Ok(output)) if output.status.success() => output,
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(TmuxSwitchError::Unavailable);
        }
        Ok(Ok(_)) | Ok(Err(_)) => return Err(TmuxSwitchError::Failed),
        Err(_) => return Err(TmuxSwitchError::Timeout),
    };
    Ok(output.stdout)
}

#[cfg(test)]
#[path = "tmux_tests.rs"]
mod tests;
