use super::*;

#[test]
fn empty_status_is_clean() {
    assert_eq!(classify_status(b"\n"), DirtyProbeResult::Clean);
}

#[test]
fn tracked_change_is_dirty() {
    assert_eq!(
        classify_status(b" M app/src/lib.rs\n"),
        DirtyProbeResult::Dirty
    );
}

#[test]
fn oversized_status_is_dirty_without_retaining_it() {
    assert_eq!(
        classify_status(&vec![b'x'; MAX_STATUS_BYTES + 1]),
        DirtyProbeResult::Dirty
    );
}
