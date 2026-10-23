use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use command::r#async::Command;
use itertools::Itertools as _;
use parking_lot::Mutex;
use tokio::sync::Semaphore;

use super::shared::shell_escape_single_quotes;
use super::{CommandExecutor, CommandOutput, ExecuteCommandOptions};
use crate::env_vars::{EnvVarValue, serialize_variables_for_shell};
use crate::terminal::shell::Shell;

/// Maximum number of background commands multiplexed over one ControlMaster at a time. Each one
/// opens a session channel, and sshd refuses channels beyond `MaxSessions` (10 by default, shared
/// with the interactive session and any other `ssh` reusing the master), which prints
/// `channel N: open failed` into the user's terminal.
const MAX_CONCURRENT_COMMANDS_PER_CONTROL_MASTER: usize = 2;

/// Keyed by ControlPath because every session reusing a master shares its `MaxSessions` budget, and
/// `supports_parallel_command_execution` is only advisory to callers.
static CONTROL_MASTER_PERMITS: LazyLock<Mutex<HashMap<PathBuf, Arc<Semaphore>>>> =
    LazyLock::new(Default::default);

fn control_master_permits(control_socket_path: &Path) -> Arc<Semaphore> {
    CONTROL_MASTER_PERMITS
        .lock()
        .entry(control_socket_path.to_path_buf())
        .or_insert_with(|| Arc::new(Semaphore::new(MAX_CONCURRENT_COMMANDS_PER_CONTROL_MASTER)))
        .clone()
}

/// `CommandExecutor` implementation that executes the given `command` in a forked process
/// that establishes a one-off SSH session with the same remote host as the active SSH session
/// using SSH's ControlMaster/ControlPath feature. This is typically used to run generator
/// commands for remote SSH sessions.
#[derive(Debug)]
pub struct RemoteCommandExecutor {
    control_socket_path: PathBuf,
    wsl_distro: Option<String>,
    permits: Arc<Semaphore>,
}

impl RemoteCommandExecutor {
    pub fn new(control_socket_path: PathBuf, wsl_distro: Option<String>) -> Self {
        Self {
            permits: control_master_permits(&control_socket_path),
            control_socket_path,
            wsl_distro,
        }
    }
}

#[async_trait]
impl CommandExecutor for RemoteCommandExecutor {
    async fn execute_command(
        &self,
        command: &str,
        shell: &Shell,
        current_directory_path: Option<&str>,
        environment_variables: Option<HashMap<String, String>>,
        _execute_command_options: ExecuteCommandOptions,
    ) -> Result<CommandOutput> {
        // We can't use `.env` and `.current_dir` here to set the PATH and current dir respectively
        // since this is run locally. We just need the subprocess to send the bytes over the
        // ssh connection. That's why we explicitly set the path and cwd as part of command_str.
        let mut command_str = String::new();
        if let Some(environment_variables) = environment_variables {
            let env_vars = environment_variables
                .into_iter()
                .map(|(key, value)| (key, EnvVarValue::Constant(value)))
                .collect_vec();
            let env_vars_str = serialize_variables_for_shell(
                env_vars.iter().map(|(key, value)| (key.as_str(), value)),
                shell.shell_type(),
            );
            command_str.push_str(&env_vars_str);
            command_str.push(';');
        }
        if let Some(current_directory_path) = current_directory_path {
            // Ensure the path escapes embedded single quotes from the remote host's serialized block.
            let escaped_path =
                shell_escape_single_quotes(current_directory_path, shell.shell_type());
            command_str.push_str(&format!("cd '{escaped_path}' && "));
        }
        command_str.push_str(command);

        let ssh_args = [
            "-q",
            "-o",
            "PasswordAuthentication=no",
            // Disable X11 forwarding, as none of our background commands
            // should need it, and it can cause warnings to appear in the
            // user's session.
            "-o",
            "ForwardX11=no",
            "-o",
            &format!(
                "ControlPath={}",
                self.control_socket_path
                    .to_str()
                    .ok_or_else(|| anyhow!("socket path must exist"))?
            ),
            "placeholder@placeholder",
            command_str.as_str(),
        ];

        // If the SSH session originated from WSL, the ControlPath also exists inside WSL.
        // Therefore, SSH commands directly from the Windows host will not work. They must be run
        // inside that same WSL instance
        let mut command = match &self.wsl_distro {
            None => Command::new("ssh"),
            Some(distro_name) => {
                let mut command = Command::new("wsl");
                command.args(["-d", distro_name.as_str(), "-e", "ssh"]);
                command
            }
        };

        command.args(ssh_args);
        let _permit = self.permits.acquire().await?;
        command
            .kill_on_drop(true)
            .output()
            .await
            .map(|output| output.into())
            .map_err(|e| anyhow!(e))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    /// The RemoteCommandExecutor can't be run in parallel because a remote box could have a value
    /// of `MaxSessions` (the max number of open sessions for a single connection) that is less than
    /// the number of commands we'd be trying to run in in parallel. Functionally, this would result
    /// in  `channel: open  failed` messages sent back over the PTY in the running sessions.
    fn supports_parallel_command_execution(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "remote_command_executor_tests.rs"]
mod tests;
