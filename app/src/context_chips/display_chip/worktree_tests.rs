use super::*;
use crate::worktree_agents::WorktreeDirtyState;

#[test]
fn menu_items_preserve_current_and_detached_state() {
    let entry = WorktreeMenuEntry {
        common_dir: "/repo/.git".to_string(),
        path: "/repo/feature".to_string(),
        branch: None,
        head: Some("123456789abc".to_string()),
        is_current: true,
        detached: true,
        locked: false,
        prunable: false,
        dirty: WorktreeDirtyState::Dirty,
        agent_count: 2,
    };
    let values = vec![entry.encode().unwrap()];
    let items = menu_items(&values);

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name(), "feature");
    assert_eq!(items[0].entry.checkout_type_label(), "Worktree");
    assert_eq!(items[0].entry.git_state_label(), "Detached 12345678");
    assert!(!items[0].entry.can_navigate());
}
