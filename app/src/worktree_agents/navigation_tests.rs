use super::*;

#[test]
fn existing_pane_is_reused_before_current_pane() {
    let target = EntityId::new();
    assert_eq!(
        decide(
            &ExecutionHostKey::Local,
            Path::new("/repo/two"),
            Some(target),
            true,
            true,
        ),
        WorktreeNavigationDecision::FocusPane(target)
    );
}

#[test]
fn busy_local_navigation_opens_a_new_terminal() {
    assert_eq!(
        decide(
            &ExecutionHostKey::Local,
            Path::new("/repo/two"),
            None,
            false,
            true,
        ),
        WorktreeNavigationDecision::OpenLocalTerminal(PathBuf::from("/repo/two"))
    );
}

#[test]
fn busy_remote_navigation_never_opens_a_local_path() {
    assert_eq!(
        decide(
            &ExecutionHostKey::Remote("host".to_string()),
            Path::new("/repo/two"),
            None,
            false,
            true,
        ),
        WorktreeNavigationDecision::BusyRemote
    );
}

#[test]
fn stale_target_never_executes() {
    assert_eq!(
        decide(
            &ExecutionHostKey::Local,
            Path::new("/repo/missing"),
            None,
            true,
            false,
        ),
        WorktreeNavigationDecision::Stale
    );
}
