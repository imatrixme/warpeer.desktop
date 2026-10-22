use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpui::elements::{CornerRadius, Point, Radius};
use warpui::event::DispatchedEvent;
use warpui::{
    AfterLayoutContext, AppContext, Element, EntityId, EventContext, LayoutContext, PaintContext,
    SizeConstraint,
};

use crate::worktree_agents::observation::AgentKey;

const ANT_SPACING: f32 = 9.;
const ANT_SIZE: f32 = 2.;
const ANCHOR_SIZE: f32 = 6.;
const REPAINT_INTERVAL: Duration = Duration::from_millis(32);

#[derive(Clone, Copy, Debug)]
struct EdgeAnchor {
    point: Vector2F,
    normal: Vector2F,
}

pub(crate) fn pane_header_position_id(terminal_view_id: EntityId) -> String {
    format!("worktree-agent-pane-header:{terminal_view_id}")
}

pub(super) fn agent_row_position_id(key: &AgentKey) -> String {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("worktree-agent-row:{:016x}", hasher.finish())
}

pub(super) struct PaneConnector {
    source_position_id: String,
    target_position_id: String,
    color: ColorU,
    reduced_motion: bool,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl PaneConnector {
    pub(super) fn new(
        key: &AgentKey,
        target_pane: EntityId,
        color: ColorU,
        reduced_motion: bool,
    ) -> Self {
        Self {
            source_position_id: agent_row_position_id(key),
            target_position_id: pane_header_position_id(target_pane),
            color,
            reduced_motion,
            size: None,
            origin: None,
        }
    }
}

impl Element for PaneConnector {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        self.size = Some(constraint.min);
        constraint.min
    }

    fn after_layout(&mut self, _ctx: &mut AfterLayoutContext, _app: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _app: &AppContext) {
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let Some(source) = ctx.position_cache.get_position(&self.source_position_id) else {
            return;
        };
        let Some(target) = ctx.position_cache.get_position(&self.target_position_id) else {
            return;
        };
        let (start, end, control_1, control_2) = connector_curve(source, target);
        let dx = end.x() - start.x();
        let dy = end.y() - start.y();
        let length = (dx * dx + dy * dy).sqrt().max(1.) + 72.;
        let phase = if self.reduced_motion {
            0.
        } else {
            ctx.repaint_after(REPAINT_INTERVAL);
            animation_phase()
        };
        let mut distance = phase * ANT_SPACING;
        while distance <= length {
            let point = cubic_point(start, control_1, control_2, end, distance / length);
            ctx.scene
                .draw_rect_without_hit_recording(RectF::new(
                    point - Vector2F::splat(ANT_SIZE * 0.5),
                    Vector2F::splat(ANT_SIZE),
                ))
                .with_background(self.color)
                .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)));
            distance += ANT_SPACING;
        }
        paint_anchor(start, self.color, ctx);
        paint_anchor(end, self.color, ctx);
    }

    fn dispatch_event(
        &mut self,
        _event: &DispatchedEvent,
        _ctx: &mut EventContext,
        _app: &AppContext,
    ) -> bool {
        false
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

fn paint_anchor(point: Vector2F, color: ColorU, ctx: &mut PaintContext) {
    ctx.scene
        .draw_rect_without_hit_recording(RectF::new(
            point - Vector2F::splat(ANCHOR_SIZE * 0.5),
            Vector2F::splat(ANCHOR_SIZE),
        ))
        .with_background(color)
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)));
}

fn connector_curve(source: RectF, target: RectF) -> (Vector2F, Vector2F, Vector2F, Vector2F) {
    let (source_anchor, target_anchor) = select_anchor_pair(source, target);
    let start = source_anchor.point;
    let end = target_anchor.point;
    let distance = vector_length(end - start);
    let bend = (distance * 0.32).clamp(28., 108.);
    let control_1 = start + source_anchor.normal * bend;
    let control_2 = end + target_anchor.normal * bend;
    (start, end, control_1, control_2)
}

fn select_anchor_pair(source: RectF, target: RectF) -> (EdgeAnchor, EdgeAnchor) {
    let source_anchors = edge_anchors(source);
    let target_anchors = edge_anchors(target);
    let mut best = (source_anchors[0], target_anchors[0]);
    let mut best_score = f32::MAX;
    for source_anchor in source_anchors {
        for target_anchor in target_anchors {
            let score = anchor_pair_score(source_anchor, target_anchor);
            if score < best_score {
                best = (source_anchor, target_anchor);
                best_score = score;
            }
        }
    }
    best
}

fn edge_anchors(rect: RectF) -> [EdgeAnchor; 4] {
    let center = rect.center();
    [
        EdgeAnchor {
            point: vec2f(center.x(), rect.min_y()),
            normal: vec2f(0., -1.),
        },
        EdgeAnchor {
            point: vec2f(rect.max_x(), center.y()),
            normal: vec2f(1., 0.),
        },
        EdgeAnchor {
            point: vec2f(center.x(), rect.max_y()),
            normal: vec2f(0., 1.),
        },
        EdgeAnchor {
            point: vec2f(rect.min_x(), center.y()),
            normal: vec2f(-1., 0.),
        },
    ]
}

fn anchor_pair_score(source: EdgeAnchor, target: EdgeAnchor) -> f32 {
    let delta = target.point - source.point;
    let distance = vector_length(delta).max(1.);
    let direction = delta / distance;
    let source_alignment = dot(source.normal, direction);
    let target_alignment = dot(-target.normal, direction);
    distance + (2. - source_alignment - target_alignment) * 48.
}

fn vector_length(vector: Vector2F) -> f32 {
    vector.x().hypot(vector.y())
}

fn dot(left: Vector2F, right: Vector2F) -> f32 {
    left.x().mul_add(right.x(), left.y() * right.y())
}

fn cubic_point(
    start: Vector2F,
    control_1: Vector2F,
    control_2: Vector2F,
    end: Vector2F,
    progress: f32,
) -> Vector2F {
    let progress = progress.clamp(0., 1.);
    let inverse = 1. - progress;
    start * inverse.powi(3)
        + control_1 * (3. * inverse.powi(2) * progress)
        + control_2 * (3. * inverse * progress.powi(2))
        + end * progress.powi(3)
}

fn animation_phase() -> f32 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    (elapsed % 900) as f32 / 900.
}

#[cfg(test)]
#[path = "pane_connector_tests.rs"]
mod tests;
