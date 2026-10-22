use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;

use super::{connector_curve, cubic_point, select_anchor_pair};

#[test]
fn connector_curve_uses_agent_row_and_titlebar_edges() {
    let source = RectF::new(vec2f(10., 40.), vec2f(100., 30.));
    let target = RectF::new(vec2f(300., 8.), vec2f(180., 34.));
    let (start, end, control_1, control_2) = connector_curve(source, target);

    assert_eq!(start, vec2f(110., 55.));
    assert_eq!(end, vec2f(300., 25.));
    assert_eq!(cubic_point(start, control_1, control_2, end, 0.), start);
    assert_eq!(cubic_point(start, control_1, control_2, end, 1.), end);
}

#[test]
fn connector_curve_uses_vertical_edges_for_a_pane_below() {
    let source = RectF::new(vec2f(100., 50.), vec2f(100., 30.));
    let target = RectF::new(vec2f(120., 220.), vec2f(100., 34.));
    let (source_anchor, target_anchor) = select_anchor_pair(source, target);

    assert_eq!(source_anchor.point, vec2f(150., 80.));
    assert_eq!(source_anchor.normal, vec2f(0., 1.));
    assert_eq!(target_anchor.point, vec2f(170., 220.));
    assert_eq!(target_anchor.normal, vec2f(0., -1.));
}
