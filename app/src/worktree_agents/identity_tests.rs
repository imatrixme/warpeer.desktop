use std::path::Path;

use super::{ExecutionHostKey, RepositoryKey, WorktreeKey};

#[test]
fn repository_identity_keeps_execution_hosts_separate() {
    let local = RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git");
    let remote = RepositoryKey::new(
        ExecutionHostKey::Remote("dev@example.com".to_string()),
        "/repo/.git",
    );

    assert_ne!(local, remote);
}

#[test]
fn worktree_identity_includes_repository_identity() {
    let repository = RepositoryKey::new(ExecutionHostKey::Local, "/repo/.git");
    let worktree = WorktreeKey::new(repository.clone(), "/repo/feature");

    assert_eq!(worktree.repository(), &repository);
    assert_eq!(worktree.path(), Path::new("/repo/feature"));
}
