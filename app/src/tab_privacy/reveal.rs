use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::Point;
use warpui::event::DispatchedEvent;
use warpui::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext,
    SizeConstraint,
};

const REVEAL_BANDS: usize = 64;

pub(super) struct CircularReveal {
    progress: f32,
    color: ColorU,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl CircularReveal {
    pub(super) fn new(progress: f32, color: ColorU) -> Self {
        Self {
            progress: progress.clamp(0., 1.),
            color,
            size: None,
            origin: None,
        }
    }
}

impl Element for CircularReveal {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        _ctx: &mut LayoutContext,
        _app: &AppContext,
    ) -> Vector2F {
        self.size = Some(constraint.max);
        constraint.max
    }

    fn after_layout(&mut self, _ctx: &mut AfterLayoutContext, _app: &AppContext) {}

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, _app: &AppContext) {
        let size = self.size.expect("reveal layout precedes paint");
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        ctx.scene
            .draw_rect_with_hit_recording(RectF::new(origin, size));

        let eased = 1. - (1. - self.progress).powi(3);
        let maximum_radius = (size.x().powi(2) + size.y().powi(2)).sqrt() * 0.55;
        let radius = maximum_radius * eased;
        let center = origin + size * 0.5;
        let band_height = size.y() / REVEAL_BANDS as f32;
        for band in 0..REVEAL_BANDS {
            let top = origin.y() + band as f32 * band_height;
            let middle = top + band_height * 0.5;
            let dy = (middle - center.y()).abs();
            let half_hole = if dy < radius {
                (radius.powi(2) - dy.powi(2)).sqrt()
            } else {
                0.
            };
            let left_width = (center.x() - half_hole - origin.x()).clamp(0., size.x());
            let right_start = (center.x() + half_hole).clamp(origin.x(), origin.x() + size.x());
            if left_width > 0. {
                ctx.scene
                    .draw_rect_without_hit_recording(RectF::new(
                        Vector2F::new(origin.x(), top),
                        Vector2F::new(left_width, band_height + 1.),
                    ))
                    .with_background(self.color);
            }
            let right_width = origin.x() + size.x() - right_start;
            if right_width > 0. {
                ctx.scene
                    .draw_rect_without_hit_recording(RectF::new(
                        Vector2F::new(right_start, top),
                        Vector2F::new(right_width, band_height + 1.),
                    ))
                    .with_background(self.color);
            }
        }
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
