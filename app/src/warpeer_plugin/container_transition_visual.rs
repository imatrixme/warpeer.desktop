use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{
    ContainerId, ContainerTransition, ContainerTransitionStyle, Revision, SurfaceResizeEasing,
};
use warpui::elements::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext, Point,
    SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::image_cache::StaticImage;
use warpui::scene::CornerRadius;

struct ActiveTransition {
    revision: Revision,
    started: Instant,
    descriptor: ContainerTransition,
    outgoing: Option<Arc<StaticImage>>,
    from_size: Vector2F,
    to_size: Vector2F,
}

#[derive(Default)]
pub(crate) struct ContainerTransitionVisuals {
    active: HashMap<ContainerId, ActiveTransition>,
}

impl ContainerTransitionVisuals {
    pub fn layout_size(
        &mut self,
        container_id: ContainerId,
        revision: Revision,
        descriptor: Option<ContainerTransition>,
        reduced_motion: bool,
        from_size: Vector2F,
        to_size: Vector2F,
    ) -> Vector2F {
        let Some(active) = self.ensure(
            container_id,
            revision,
            descriptor,
            reduced_motion,
            from_size,
            to_size,
        ) else {
            return to_size;
        };
        let progress = transition_progress(active.started, active.descriptor);
        active.from_size + (active.to_size - active.from_size) * progress
    }

    pub fn resolve(
        &mut self,
        container_id: ContainerId,
        revision: Revision,
        descriptor: Option<ContainerTransition>,
        reduced_motion: bool,
        incoming_ready: bool,
        outgoing: Option<Arc<StaticImage>>,
    ) -> Option<(Arc<StaticImage>, Instant, ContainerTransition)> {
        let active = self.active.get_mut(&container_id)?;
        if active.revision != revision || descriptor != Some(active.descriptor) || reduced_motion {
            self.active.remove(&container_id);
            return None;
        }
        if active.outgoing.is_none() {
            active.outgoing = outgoing;
        }
        if incoming_ready
            && active.started.elapsed()
                >= Duration::from_millis(active.descriptor.duration_ms.into())
        {
            self.active.remove(&container_id);
            return None;
        }
        Some((active.outgoing.clone()?, active.started, active.descriptor))
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }

    fn ensure(
        &mut self,
        container_id: ContainerId,
        revision: Revision,
        descriptor: Option<ContainerTransition>,
        reduced_motion: bool,
        from_size: Vector2F,
        to_size: Vector2F,
    ) -> Option<&ActiveTransition> {
        let descriptor = descriptor.filter(|transition| {
            !reduced_motion
                && transition.duration_ms > 0
                && transition.style != ContainerTransitionStyle::None
        });
        let Some(descriptor) = descriptor else {
            self.active.remove(&container_id);
            return None;
        };
        let replace = self
            .active
            .get(&container_id)
            .is_none_or(|active| active.revision != revision || active.descriptor != descriptor);
        if replace {
            self.active.insert(
                container_id,
                ActiveTransition {
                    revision,
                    started: Instant::now(),
                    descriptor,
                    outgoing: None,
                    from_size,
                    to_size,
                },
            );
        }
        self.active.get(&container_id)
    }
}

pub(crate) struct AnimatedExternalSurface {
    outgoing: Arc<StaticImage>,
    incoming: Option<Arc<StaticImage>>,
    transition: ContainerTransition,
    started: Instant,
    corner_radius: CornerRadius,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

impl AnimatedExternalSurface {
    pub fn new(
        outgoing: Arc<StaticImage>,
        incoming: Option<Arc<StaticImage>>,
        transition: ContainerTransition,
        started: Instant,
        corner_radius: CornerRadius,
    ) -> Self {
        Self {
            outgoing,
            incoming,
            transition,
            started,
            corner_radius,
            size: None,
            origin: None,
        }
    }
}

impl Element for AnimatedExternalSurface {
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
        let Some(size) = self.size else { return };
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        let Some(incoming) = self.incoming.clone() else {
            ctx.scene.draw_image(
                RectF::new(origin, size),
                self.outgoing.clone(),
                1.0,
                self.corner_radius,
            );
            ctx.repaint_after(Duration::from_millis(16));
            return;
        };
        let duration = Duration::from_millis(self.transition.duration_ms.max(1).into());
        let linear =
            (self.started.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        let progress = eased(linear, self.transition.easing);
        let travel = if self.transition.style == ContainerTransitionStyle::Slide {
            size.x() * 0.08
        } else {
            0.0
        };
        ctx.scene.draw_image(
            RectF::new(origin - Vector2F::new(travel * progress, 0.0), size),
            self.outgoing.clone(),
            1.0 - progress,
            self.corner_radius,
        );
        ctx.scene.draw_image(
            RectF::new(origin + Vector2F::new(travel * (1.0 - progress), 0.0), size),
            incoming,
            progress,
            self.corner_radius,
        );
        if linear < 1.0 {
            ctx.repaint_after(Duration::from_millis(16));
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

fn eased(value: f32, easing: SurfaceResizeEasing) -> f32 {
    match easing {
        SurfaceResizeEasing::Linear => value,
        SurfaceResizeEasing::EaseOut => 1.0 - (1.0 - value).powi(3),
        SurfaceResizeEasing::EaseInOut => value * value * (3.0 - 2.0 * value),
        SurfaceResizeEasing::Spring => {
            (1.0 - (-8.0 * value).exp() * (10.0 * value).cos()).clamp(0.0, 1.0)
        }
    }
}

fn transition_progress(started: Instant, transition: ContainerTransition) -> f32 {
    let duration = Duration::from_millis(transition.duration_ms.max(1).into());
    let linear = (started.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
    eased(linear, transition.easing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(channel: u8) -> Arc<StaticImage> {
        StaticImage::from_rgba(1, 1, vec![channel, 0, 0, 255]).unwrap()
    }

    fn transition() -> ContainerTransition {
        ContainerTransition {
            style: ContainerTransitionStyle::Fade,
            duration_ms: 300,
            easing: SurfaceResizeEasing::EaseOut,
        }
    }

    #[test]
    fn stable_revision_reuses_one_transition_clock() {
        let id = ContainerId::new();
        let mut visuals = ContainerTransitionVisuals::default();
        let from = Vector2F::new(100.0, 80.0);
        let to = Vector2F::new(240.0, 160.0);
        visuals.layout_size(id, Revision(2), Some(transition()), false, from, to);
        let first = visuals
            .resolve(
                id,
                Revision(2),
                Some(transition()),
                false,
                false,
                Some(image(1)),
            )
            .unwrap();
        let second = visuals
            .resolve(
                id,
                Revision(2),
                Some(transition()),
                false,
                true,
                Some(image(2)),
            )
            .unwrap();
        assert_eq!(first.1, second.1);
        assert!(Arc::ptr_eq(&first.0, &second.0));
    }

    #[test]
    fn reduce_motion_and_missing_outgoing_clear_stale_visuals() {
        let id = ContainerId::new();
        let mut visuals = ContainerTransitionVisuals::default();
        let size = Vector2F::new(100.0, 80.0);
        visuals.layout_size(id, Revision(2), Some(transition()), false, size, size);
        assert!(
            visuals
                .resolve(
                    id,
                    Revision(2),
                    Some(transition()),
                    false,
                    false,
                    Some(image(1)),
                )
                .is_some()
        );
        visuals.layout_size(id, Revision(3), Some(transition()), false, size, size);
        assert!(
            visuals
                .resolve(id, Revision(3), Some(transition()), false, false, None)
                .is_none()
        );
        visuals.layout_size(id, Revision(4), Some(transition()), true, size, size);
        assert!(
            visuals
                .resolve(
                    id,
                    Revision(4),
                    Some(transition()),
                    true,
                    true,
                    Some(image(3)),
                )
                .is_none()
        );
        assert!(visuals.active.is_empty());
    }
}
