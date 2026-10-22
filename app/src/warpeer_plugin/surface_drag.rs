use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{ContainerId, Revision, SurfaceBounds, SurfaceId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SurfaceDragKey {
    Legacy { id: SurfaceId, revision: Revision },
    Container(ContainerId),
}

#[derive(Clone, Copy)]
pub(crate) struct SurfaceDragStart {
    pub pointer: Vector2F,
    pub origin: Vector2F,
}

#[derive(Default)]
pub(crate) struct SurfaceDragState {
    pub origin: Option<Vector2F>,
    pub start: Option<SurfaceDragStart>,
    key: Option<SurfaceDragKey>,
}

impl SurfaceDragState {
    pub fn prepare(&mut self, key: SurfaceDragKey, origin: Vector2F) {
        if self.key != Some(key) {
            self.key = Some(key);
            self.origin = Some(origin);
            self.start = None;
        } else if self.start.is_none() {
            self.origin = Some(origin);
        }
    }

    pub fn release_capture(&mut self) -> Option<ContainerId> {
        let captured_container = self.start.take().and_then(|_| match self.key {
            Some(SurfaceDragKey::Container(id)) => Some(id),
            _ => None,
        });
        self.key = None;
        self.origin = None;
        captured_container
    }
}

pub(crate) fn bounds_at_origin(
    requested: SurfaceBounds,
    origin: Vector2F,
    viewport: SurfaceBounds,
) -> SurfaceBounds {
    clamp_surface_origin(
        requested,
        origin - Vector2F::new(requested.x, requested.y),
        viewport,
    )
}

pub(crate) fn clamp_surface_origin(
    requested: SurfaceBounds,
    offset: Vector2F,
    viewport: SurfaceBounds,
) -> SurfaceBounds {
    let width = requested.width.min(viewport.width).max(1.);
    let height = requested.height.min(viewport.height).max(1.);
    let max_x = (viewport.x + viewport.width - width).max(viewport.x);
    let max_y = (viewport.y + viewport.height - height).max(viewport.y);
    SurfaceBounds {
        x: (requested.x + offset.x()).clamp(viewport.x, max_x),
        y: (requested.y + offset.y()).clamp(viewport.y, max_y),
        width,
        height,
        scale: requested.scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_surface_origin_is_clamped_to_viewport() {
        let requested = SurfaceBounds {
            x: 100.,
            y: 80.,
            width: 240.,
            height: 160.,
            scale: 1.,
        };
        let viewport = SurfaceBounds {
            x: 10.,
            y: 20.,
            width: 500.,
            height: 300.,
            scale: 1.,
        };

        let upper_left = clamp_surface_origin(requested, Vector2F::new(-500., -500.), viewport);
        assert_eq!((upper_left.x, upper_left.y), (10., 20.));
        let lower_right = clamp_surface_origin(requested, Vector2F::new(900., 900.), viewport);
        assert_eq!((lower_right.x, lower_right.y), (270., 160.));
        assert_eq!((lower_right.width, lower_right.height), (240., 160.));
    }

    #[test]
    fn stable_container_identity_survives_authoritative_revision_changes() {
        let id = ContainerId::new();
        let mut drag = SurfaceDragState::default();
        drag.prepare(SurfaceDragKey::Container(id), Vector2F::new(40., 50.));
        drag.start = Some(SurfaceDragStart {
            pointer: Vector2F::new(10., 10.),
            origin: Vector2F::new(40., 50.),
        });
        drag.origin = Some(Vector2F::new(80., 90.));
        drag.prepare(SurfaceDragKey::Container(id), Vector2F::new(20., 30.));
        assert_eq!(drag.origin, Some(Vector2F::new(80., 90.)));
    }

    #[test]
    fn container_can_move_above_and_across_its_trigger_seed() {
        let requested = SurfaceBounds {
            x: 240.,
            y: 260.,
            width: 180.,
            height: 120.,
            scale: 1.,
        };
        let viewport = SurfaceBounds {
            x: 0.,
            y: 0.,
            width: 800.,
            height: 600.,
            scale: 1.,
        };

        let moved = bounds_at_origin(requested, Vector2F::new(40., 30.), viewport);
        assert_eq!((moved.x, moved.y), (40., 30.));
    }

    #[test]
    fn viewport_shrink_applies_only_the_required_clamp() {
        let requested = SurfaceBounds {
            x: 300.,
            y: 220.,
            width: 240.,
            height: 180.,
            scale: 2.,
        };
        let viewport = SurfaceBounds {
            x: 20.,
            y: 30.,
            width: 420.,
            height: 300.,
            scale: 2.,
        };
        let clamped = bounds_at_origin(requested, Vector2F::new(300., 220.), viewport);
        assert_eq!((clamped.x, clamped.y), (200., 150.));
        assert_eq!((clamped.width, clamped.height), (240., 180.));

        let unchanged = bounds_at_origin(requested, Vector2F::new(80., 70.), viewport);
        assert_eq!((unchanged.x, unchanged.y), (80., 70.));
    }

    #[test]
    fn legacy_surface_revision_ends_stale_drag_capture() {
        let id = SurfaceId::new();
        let mut drag = SurfaceDragState::default();
        drag.prepare(
            SurfaceDragKey::Legacy {
                id,
                revision: Revision(1),
            },
            Vector2F::new(40., 50.),
        );
        drag.start = Some(SurfaceDragStart {
            pointer: Vector2F::new(10., 10.),
            origin: Vector2F::new(40., 50.),
        });
        drag.prepare(
            SurfaceDragKey::Legacy {
                id,
                revision: Revision(2),
            },
            Vector2F::new(60., 70.),
        );
        assert!(drag.start.is_none());
        assert_eq!(drag.origin, Some(Vector2F::new(60., 70.)));
    }

    #[test]
    fn releasing_container_drag_capture_returns_its_stable_identity() {
        let id = ContainerId::new();
        let mut drag = SurfaceDragState::default();
        drag.prepare(SurfaceDragKey::Container(id), Vector2F::new(40., 50.));
        drag.start = Some(SurfaceDragStart {
            pointer: Vector2F::new(10., 10.),
            origin: Vector2F::new(40., 50.),
        });

        assert_eq!(drag.release_capture(), Some(id));
        assert!(drag.start.is_none());
        assert!(drag.origin.is_none());
        assert_eq!(drag.release_capture(), None);
    }
}
