use std::path::{Path, PathBuf};

use command::r#async::Command;

use crate::worktree_agents::{WorktreeDirtyState, WorktreeMenuEntry};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateMode {
    NewBranch,
    ExistingBranch,
}

#[derive(Clone, Debug)]
pub(super) struct CreateWorktreeRequest {
    pub(super) source_path: PathBuf,
    pub(super) common_dir: String,
    pub(super) destination: PathBuf,
    pub(super) worktree_name: String,
    pub(super) mode: CreateMode,
    pub(super) branch: String,
}

impl CreateWorktreeRequest {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.source_path.as_os_str().is_empty() {
            return Err("No local Git repository is available.".to_string());
        }
        validate_worktree_name(&self.worktree_name)?;
        if self.destination.exists() {
            return Err("The destination folder already exists.".to_string());
        }
        if self.mode == CreateMode::ExistingBranch && self.branch.trim().is_empty() {
            return Err("Choose an existing branch.".to_string());
        }
        Ok(())
    }
}

pub(super) async fn create_worktree(
    request: CreateWorktreeRequest,
) -> Result<WorktreeMenuEntry, String> {
    request.validate()?;
    check_branch(&request).await?;
    let mut command = git_command(&request.source_path);
    command.arg("worktree").arg("add");
    match request.mode {
        CreateMode::NewBranch => {
            command
                .arg("-b")
                .arg(&request.branch)
                .arg(&request.destination)
                .arg("HEAD");
        }
        CreateMode::ExistingBranch => {
            command.arg(&request.destination).arg(&request.branch);
        }
    }
    let output = command
        .output()
        .await
        .map_err(|error| format!("Failed to start Git: {error}"))?;
    if !output.status.success() {
        return Err(git_error(
            &output.stderr,
            "Git could not create the worktree.",
        ));
    }

    let branch = git_text(
        &request.destination,
        ["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .await
    .ok();
    let head = git_text(&request.destination, ["rev-parse", "HEAD"])
        .await
        .ok();
    Ok(WorktreeMenuEntry {
        common_dir: request.common_dir,
        path: request.destination.to_string_lossy().into_owned(),
        branch: branch.clone(),
        head,
        is_current: false,
        detached: branch.is_none(),
        locked: false,
        prunable: false,
        dirty: WorktreeDirtyState::Clean,
        agent_count: 0,
    })
}

async fn check_branch(request: &CreateWorktreeRequest) -> Result<(), String> {
    let branch = request.branch.trim();
    let output = git_command(&request.source_path)
        .args(["check-ref-format", "--branch", branch])
        .output()
        .await
        .map_err(|error| format!("Failed to validate the branch: {error}"))?;
    if !output.status.success() {
        return Err("The branch name is not valid for Git.".to_string());
    }

    let reference = format!("refs/heads/{branch}");
    let exists = git_command(&request.source_path)
        .args(["show-ref", "--verify", "--quiet", reference.as_str()])
        .status()
        .await
        .map_err(|error| format!("Failed to inspect branches: {error}"))?
        .success();
    match request.mode {
        CreateMode::NewBranch if exists => {
            Err("A branch with this worktree name already exists.".to_string())
        }
        CreateMode::ExistingBranch if !exists => {
            Err("The selected local branch does not exist.".to_string())
        }
        CreateMode::NewBranch | CreateMode::ExistingBranch => Ok(()),
    }
}

fn git_command(repository: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(repository)
        .kill_on_drop(true);
    command
}

async fn git_text<const N: usize>(repository: &Path, args: [&str; N]) -> Result<String, String> {
    let output = git_command(repository)
        .args(args)
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(git_error(&output.stderr, "Git query failed."));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty())
        .then_some(value)
        .ok_or_else(|| "Git returned an empty value.".to_string())
}

fn git_error(stderr: &[u8], fallback: &str) -> String {
    let message: String = String::from_utf8_lossy(stderr)
        .chars()
        .filter(|character| !character.is_control() || *character == ' ')
        .take(240)
        .collect();
    let message = message.trim();
    if message.is_empty() {
        fallback.to_string()
    } else {
        message.to_string()
    }
}

fn validate_worktree_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a worktree name.".to_string());
    }
    if name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err("Use a single folder name without slashes.".to_string());
    }
    Ok(())
}

#[cfg(test)]
#[path = "create_operation_tests.rs"]
mod tests;
