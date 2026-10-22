use parking_lot::Mutex;

use super::{HoverDismissalState, snapshot_without_retaining};

#[test]
fn terminal_snapshot_lock_is_released_before_dispatch_scope() {
    let terminal_model = Mutex::new(41_u8);
    let snapshot = snapshot_without_retaining(|| {
        let model = terminal_model.lock();
        *model + 1
    });
    assert_eq!(snapshot, 42);
    assert!(terminal_model.try_lock().is_some());
}

#[test]
fn surface_hover_invalidates_pending_dismissal_until_pointer_leaves() {
    let mut dismissal = HoverDismissalState::default();
    let source_leave = dismissal.schedule().unwrap();

    assert!(dismissal.set_surface_hovered(true).is_none());
    assert!(!dismissal.should_cancel(source_leave));
    assert!(dismissal.schedule().is_none());

    let surface_leave = dismissal.set_surface_hovered(false).unwrap();
    assert!(dismissal.should_cancel(surface_leave));
    dismissal.invalidate();
    assert!(!dismissal.should_cancel(surface_leave));
}
