use super::*;

#[test]
fn initial_origin_prefers_below_then_clamps_inside_viewport() {
    let origin = anchored_origin(
        [180.0, 240.0, 20.0, 20.0],
        [0.0, 0.0, 400.0, 300.0],
        SurfaceContentSize {
            width: 240.0,
            height: 120.0,
        },
    );
    assert_eq!(origin.y, 110.0);
    assert_eq!(origin.x, 70.0);
}

#[test]
fn initial_origin_can_seed_above_trigger_without_locking_to_it() {
    let origin = anchored_origin(
        [40.0, 20.0, 10.0, 10.0],
        [0.0, 0.0, 320.0, 240.0],
        SurfaceContentSize {
            width: 160.0,
            height: 80.0,
        },
    );
    assert_eq!(origin.y, 40.0);
    assert_eq!(origin.x, 8.0);
}
