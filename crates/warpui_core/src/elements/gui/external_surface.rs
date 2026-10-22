use std::sync::Arc;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;

use super::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext, Point,
    SizeConstraint,
};
use crate::event::DispatchedEvent;
use crate::image_cache::StaticImage;
use crate::scene::CornerRadius;

/// Composites one complete frame supplied by an external renderer.
///
/// The host owns placement, clipping, and input. The external runtime owns every
/// pixel in the frame, so WarpUI never interprets a plugin UI tree.
pub struct ExternalSurface {
    image: Arc<StaticImage>,
    corner_radius: CornerRadius,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl ExternalSurface {
    pub fn new(image: Arc<StaticImage>) -> Self {
        Self {
            image,
            corner_radius: CornerRadius::default(),
            size: None,
            origin: None,
        }
    }

    pub fn with_corner_radius(mut self, corner_radius: CornerRadius) -> Self {
        self.corner_radius = corner_radius;
        self
    }
}

impl Element for ExternalSurface {
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
        let Some(size) = self.size else {
            return;
        };
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        ctx.scene.draw_image(
            RectF::new(origin, size),
            self.image.clone(),
            1.0,
            self.corner_radius,
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_frame_rejects_incorrect_buffer_size() {
        assert!(StaticImage::from_rgba(2, 2, vec![0; 15]).is_none());
        assert!(StaticImage::from_rgba(2, 2, vec![0; 16]).is_some());
    }
}
