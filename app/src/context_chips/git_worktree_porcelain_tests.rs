use super::parse_git_worktree_porcelain;

#[test]
fn parses_linked_detached_locked_and_prunable_records() {
    let records = parse_git_worktree_porcelain(
        "worktree /repo\nHEAD abcdef\nbranch refs/heads/main\n\n\
         worktree /repo/feature\nHEAD 123456\ndetached\nlocked in use\nprunable missing\n",
    );

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].path, "/repo");
    assert_eq!(records[0].branch.as_deref(), Some("refs/heads/main"));
    assert!(records[1].detached);
    assert_eq!(records[1].lock_reason.as_deref(), Some("in use"));
    assert_eq!(records[1].prune_reason.as_deref(), Some("missing"));
}

#[test]
fn nul_mode_preserves_spaces_without_git_quoting() {
    let records = parse_git_worktree_porcelain(
        "worktree /repo/feature with spaces\0HEAD abcdef\0branch refs/heads/feature\0\0",
    );

    assert_eq!(records[0].path, "/repo/feature with spaces");
}

#[test]
fn line_mode_decodes_git_quoted_paths() {
    let records = parse_git_worktree_porcelain(
        "worktree \"/repo/line\\nname/utf8-\\346\\265\\213\\350\\257\\225\"\nHEAD abcdef\ndetached\n",
    );

    assert_eq!(records[0].path, "/repo/line\nname/utf8-测试");
}
