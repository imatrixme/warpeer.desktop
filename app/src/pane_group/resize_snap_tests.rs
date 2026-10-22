use super::*;

#[test]
fn snaps_to_common_ratios_from_both_sides() {
    assert_eq!(snap_boundary(243., 1000., 50., 950.), 250.);
    assert_eq!(snap_boundary(326., 1000., 50., 950.), 1000. / 3.);
    assert_eq!(snap_boundary(508., 1000., 50., 950.), 500.);
    assert_eq!(snap_boundary(674., 1000., 50., 950.), 2000. / 3.);
    assert_eq!(snap_boundary(757., 1000., 50., 950.), 750.);
}

#[test]
fn releases_immediately_beyond_eight_pixels() {
    assert_eq!(snap_boundary(509., 1000., 50., 950.), 509.);
    assert_eq!(snap_boundary(491., 1000., 50., 950.), 491.);
}

#[test]
fn leaves_distant_boundaries_unchanged() {
    assert_eq!(snap_boundary(400., 1000., 50., 950.), 400.);
}

#[test]
fn ignores_targets_that_violate_pane_minimums() {
    assert_eq!(snap_boundary(250., 1000., 300., 700.), 250.);
}
