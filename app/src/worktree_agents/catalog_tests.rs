use super::*;

const OUTPUT: &str = "warpeer-current /repo\0warpeer-common /repo/.git\0worktree /repo\0HEAD abcdef012345\0branch refs/heads/main\0\0worktree /repo/feature with spaces\0HEAD 123456789abc\0detached\0locked in use\0\0worktree /repo/stale\0HEAD feedface\0branch refs/heads/stale\0prunable missing\0\0";

#[test]
fn prompt_output_marks_current_and_preserves_paths() {
    let entries = parse_menu_entries(OUTPUT);

    assert_eq!(entries.len(), 3);
    assert!(entries[0].is_current);
    assert_eq!(entries[1].path, "/repo/feature with spaces");
    assert_eq!(entries[1].common_dir, "/repo/.git");
    assert!(entries[1].detached);
    assert!(entries[1].locked);
    assert!(entries[2].prunable);
}

#[test]
fn prompt_chip_is_hidden_outside_git_and_visible_inside_git() {
    assert_eq!(normalize_chip_value(None), None);
    assert_eq!(
        normalize_chip_value(Some(ChipValue::Text(OUTPUT.to_string()))),
        Some(ChipValue::Text("worktrees".to_string()))
    );
    let single = "warpeer-current /repo\0warpeer-common /repo/.git\0worktree /repo\0HEAD abcdef012345\0branch refs/heads/main\0\0";
    assert_eq!(
        normalize_chip_value(Some(ChipValue::Text(single.to_string()))),
        None
    );
    let values = filter_on_click_values(Some(vec![single.to_string()])).unwrap();
    assert_eq!(decode_menu_entries(&values).len(), 1);
}

#[test]
fn encoded_entries_round_trip_shell_sensitive_paths() {
    let values = filter_on_click_values(Some(vec![OUTPUT.to_string()])).unwrap();
    let entry = WorktreeMenuEntry::decode(&values[1]).unwrap();

    assert_eq!(entry.path, "/repo/feature with spaces");
    assert!(entry.can_navigate());
    assert!(entry.display_name().contains("feature with spaces"));
}

#[test]
fn current_and_prunable_worktrees_are_not_navigation_targets() {
    let entries = parse_menu_entries(OUTPUT);

    assert!(!entries[0].can_navigate());
    assert!(!entries[2].can_navigate());
}

#[test]
fn menu_labels_separate_checkout_type_from_git_state() {
    let entries = parse_menu_entries(OUTPUT);

    assert_eq!(entries[0].compact_name(), "repo");
    assert_eq!(entries[0].checkout_type_label(), "Primary");
    assert_eq!(entries[0].git_state_label(), "main");
    assert_eq!(entries[1].checkout_type_label(), "Worktree");
    assert_eq!(entries[1].git_state_label(), "Detached 12345678");
}
