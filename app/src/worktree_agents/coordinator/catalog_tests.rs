use super::*;

fn entry() -> WorktreeMenuEntry {
    WorktreeMenuEntry {
        common_dir: "/repo/.git".to_string(),
        path: "/repo".to_string(),
        branch: Some("main".to_string()),
        head: None,
        is_current: true,
        detached: false,
        locked: false,
        prunable: false,
        dirty: WorktreeDirtyState::Unknown,
        agent_count: 0,
    }
}

#[test]
fn dirty_results_require_the_current_catalog_generation() {
    assert!(accepts_dirty_generation(Some(4), 4));
    assert!(!accepts_dirty_generation(Some(5), 4));
    assert!(!accepts_dirty_generation(None, 4));
}

#[test]
fn remote_catalogs_never_run_local_dirty_probes() {
    assert!(can_probe_dirty(&ExecutionHostKey::Local));
    assert!(!can_probe_dirty(&ExecutionHostKey::Remote(
        "host".to_string()
    )));
}

#[test]
fn dirty_cache_survives_an_unchanged_catalog_refresh() {
    let metadata = WorktreeMetadata {
        branch: Some("main".to_string()),
        detached_head: None,
        locked: false,
        available: true,
        dirty: WorktreeDirtyState::Dirty,
    };

    assert!(same_catalog_metadata(&metadata, &entry()));
}
