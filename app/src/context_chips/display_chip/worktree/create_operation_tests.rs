use std::fs;
use std::process::Command as StdCommand;

use super::*;

fn request(name: &str) -> CreateWorktreeRequest {
    CreateWorktreeRequest {
        source_path: PathBuf::from("/repo"),
        common_dir: "/repo/.git".to_string(),
        destination: PathBuf::from("/definitely-missing-warpeer-test-path").join(name),
        worktree_name: name.to_string(),
        mode: CreateMode::NewBranch,
        branch: name.to_string(),
    }
}

#[test]
fn worktree_name_is_a_single_safe_path_component() {
    assert!(request("feature-agent-ui").validate().is_ok());
    assert_eq!(
        request("nested/name").validate().unwrap_err(),
        "Use a single folder name without slashes."
    );
    assert_eq!(
        request(" ").validate().unwrap_err(),
        "Enter a worktree name."
    );
}

#[test]
fn existing_branch_mode_requires_a_branch() {
    let mut request = request("feature-agent-ui");
    request.mode = CreateMode::ExistingBranch;
    request.branch.clear();

    assert_eq!(
        request.validate().unwrap_err(),
        "Choose an existing branch."
    );
}

#[test]
fn missing_repository_context_disables_creation() {
    let mut request = request("feature-agent-ui");
    request.source_path.clear();

    assert_eq!(
        request.validate().unwrap_err(),
        "No local Git repository is available."
    );
}

#[test]
fn creates_a_new_branch_worktree_from_head() {
    let temp = tempfile::TempDir::new().unwrap();
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).unwrap();
    git(&repository, ["init", "-b", "main"]);
    git(
        &repository,
        ["config", "user.email", "warpeer@example.test"],
    );
    git(&repository, ["config", "user.name", "Warpeer Test"]);
    fs::write(repository.join("README.md"), "test\n").unwrap();
    git(&repository, ["add", "README.md"]);
    git(&repository, ["commit", "-m", "initial"]);
    let common_dir = git_text_sync(
        &repository,
        ["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    let destination = temp.path().join("feature-agent-ui");

    let entry = futures::executor::block_on(create_worktree(CreateWorktreeRequest {
        source_path: repository,
        common_dir,
        destination: destination.clone(),
        worktree_name: "feature-agent-ui".to_string(),
        mode: CreateMode::NewBranch,
        branch: "feature-agent-ui".to_string(),
    }))
    .unwrap();

    assert_eq!(entry.path, destination.to_string_lossy());
    assert_eq!(entry.branch.as_deref(), Some("feature-agent-ui"));
    assert!(destination.join("README.md").is_file());
}

fn git<const N: usize>(repository: &Path, args: [&str; N]) {
    let status = StdCommand::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

fn git_text_sync<const N: usize>(repository: &Path, args: [&str; N]) -> String {
    let output = StdCommand::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
