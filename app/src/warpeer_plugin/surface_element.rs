use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use pathfinder_geometry::vector::Vector2F;
use warp_core::ui::theme::ColorScheme;
use warpeer_plugin_protocol::{
    AppearanceKind, ContainerDragRegion, ContainerId, ContainerTransition, FrameTransport,
    Revision, SurfaceBounds, SurfaceContainerMetrics, SurfaceFrame, SurfaceId, SurfaceInput,
    SurfacePalette, SurfacePoint,
};
use warpui::SingletonEntity as _;
use warpui::elements::{AppContext, Element, ExternalSurface};
use warpui::image_cache::StaticImage;
use warpui::scene::CornerRadius;

use super::container_transition_visual::{AnimatedExternalSurface, ContainerTransitionVisuals};
use super::host::PluginHostClient;
use super::surface_drag::{SurfaceDragKey, SurfaceDragState, bounds_at_origin};
use super::surface_store::SurfaceSnapshot;
use crate::appearance::Appearance;

#[path = "surface_element_host.rs"]
mod host_element;
#[path = "surface_element_support.rs"]
mod support;
use host_element::PluginSurfaceElement;
pub(crate) use support::{
    PLUGIN_WINDOW_TITLEBAR_HEIGHT, container_bounds, plugin_window_container_metrics,
};
use support::{rgba, rgba_from_bgra, system_reduce_motion_enabled};

struct CachedFrame {
    surface_id: SurfaceId,
    revision: Revision,
    image: Arc<StaticImage>,
}

#[derive(Clone)]
pub(crate) struct SurfacePresentationOptions {
    pub appearance: AppearanceKind,
    pub palette: SurfacePalette,
    pub reduced_motion: bool,
    pub focused: bool,
    pub corner_radius: CornerRadius,
    pub container: SurfaceContainerMetrics,
    drag_region: Option<SurfaceDragRegion>,
    container_drag: Option<ContainerDragPresentation>,
}

#[derive(Clone, Copy)]
struct SurfaceDragRegion {
    height: f32,
    viewport: SurfaceBounds,
}

#[derive(Clone)]
struct ContainerDragPresentation {
    id: ContainerId,
    origin: SurfacePoint,
    viewport: SurfaceBounds,
    regions: Vec<ContainerDragRegion>,
}

impl SurfacePresentationOptions {
    pub fn standard(app: &AppContext, focused: bool, corner_radius: CornerRadius) -> Self {
        let theme = Appearance::as_ref(app).theme();
        let appearance = match theme.inferred_color_scheme() {
            ColorScheme::LightOnDark => AppearanceKind::Dark,
            ColorScheme::DarkOnLight => AppearanceKind::Light,
        };
        let surface = theme.surface_1();
        Self {
            appearance,
            palette: SurfacePalette {
                background: rgba(theme.background().into_solid()),
                surface: rgba(surface.into_solid()),
                text: rgba(theme.main_text_color(surface).into_solid()),
                muted_text: rgba(theme.sub_text_color(surface).into_solid()),
                border: rgba(theme.outline().into_solid()),
                accent: rgba(theme.accent().into_solid()),
            },
            reduced_motion: system_reduce_motion_enabled(),
            focused,
            corner_radius,
            container: SurfaceContainerMetrics::default(),
            drag_region: None,
            container_drag: None,
        }
    }

    pub fn with_window_container(mut self) -> Self {
        self.container = plugin_window_container_metrics();
        self
    }

    pub fn with_container_metrics(mut self, container: SurfaceContainerMetrics) -> Self {
        self.container = container;
        self
    }

    pub fn with_drag_region(mut self, height: f32, viewport: SurfaceBounds) -> Self {
        self.drag_region = Some(SurfaceDragRegion { height, viewport });
        self
    }

    pub fn with_container_drag(
        mut self,
        id: ContainerId,
        origin: SurfacePoint,
        viewport: SurfaceBounds,
        regions: Vec<ContainerDragRegion>,
    ) -> Self {
        self.container_drag = Some(ContainerDragPresentation {
            id,
            origin,
            viewport,
            regions,
        });
        self
    }
}

pub(crate) struct PluginSurfacePresentation {
    client: PluginHostClient,
    cached_frame: RefCell<Option<CachedFrame>>,
    last_focus: RefCell<Option<bool>>,
    drag: Rc<RefCell<SurfaceDragState>>,
    container_transitions: Rc<RefCell<ContainerTransitionVisuals>>,
}

impl PluginSurfacePresentation {
    pub fn new(client: PluginHostClient) -> Self {
        Self {
            client,
            cached_frame: RefCell::new(None),
            last_focus: RefCell::new(None),
            drag: Rc::new(RefCell::new(SurfaceDragState::default())),
            container_transitions: Rc::new(RefCell::new(ContainerTransitionVisuals::default())),
        }
    }

    pub fn display_bounds(
        &self,
        requested: SurfaceBounds,
        viewport: SurfaceBounds,
        id: SurfaceId,
        revision: Revision,
    ) -> SurfaceBounds {
        let mut drag = self.drag.borrow_mut();
        drag.prepare(
            SurfaceDragKey::Legacy { id, revision },
            Vector2F::new(requested.x, requested.y),
        );
        bounds_at_origin(
            requested,
            drag.origin
                .unwrap_or_else(|| Vector2F::new(requested.x, requested.y)),
            viewport,
        )
    }

    pub fn display_container_bounds(
        &self,
        mut requested: SurfaceBounds,
        viewport: SurfaceBounds,
        id: ContainerId,
        origin: SurfacePoint,
        revision: Revision,
        transition: Option<ContainerTransition>,
        reduced_motion: bool,
        outgoing: Option<SurfaceBounds>,
    ) -> SurfaceBounds {
        let from = outgoing
            .map(|bounds| Vector2F::new(bounds.width, bounds.height))
            .unwrap_or_else(|| Vector2F::new(requested.width, requested.height));
        let to = Vector2F::new(requested.width, requested.height);
        let size = self.container_transitions.borrow_mut().layout_size(
            id,
            revision,
            transition,
            reduced_motion,
            from,
            to,
        );
        requested.width = size.x();
        requested.height = size.y();
        let mut drag = self.drag.borrow_mut();
        drag.prepare(
            SurfaceDragKey::Container(id),
            Vector2F::new(origin.x, origin.y),
        );
        let bounds = bounds_at_origin(
            requested,
            drag.origin
                .unwrap_or_else(|| Vector2F::new(origin.x, origin.y)),
            viewport,
        );
        if bounds.x != origin.x || bounds.y != origin.y {
            self.client.move_container(
                id,
                SurfacePoint {
                    x: bounds.x,
                    y: bounds.y,
                },
            );
        }
        bounds
    }

    pub fn element(
        &self,
        snapshot: SurfaceSnapshot,
        options: SurfacePresentationOptions,
    ) -> Box<dyn Element> {
        if self.last_focus.replace(Some(options.focused)) != Some(options.focused) {
            self.client.surface_input(
                snapshot.descriptor.id,
                SurfaceInput::Focus {
                    focused: options.focused,
                },
            );
        }
        let image = snapshot
            .frame
            .as_ref()
            .and_then(|frame| self.image_for_frame(snapshot.descriptor.id, frame));
        let corner_radius = options.corner_radius;
        PluginSurfaceElement {
            client: self.client.clone(),
            snapshot,
            options,
            drag: self.drag.clone(),
            child: image.map(|image| {
                ExternalSurface::new(image)
                    .with_corner_radius(corner_radius)
                    .finish()
            }),
            size: None,
            origin: None,
            content_origin: None,
        }
        .finish()
    }

    pub fn container_element(
        &self,
        snapshot: SurfaceSnapshot,
        outgoing: Option<SurfaceSnapshot>,
        options: SurfacePresentationOptions,
        container_id: ContainerId,
        container_revision: Revision,
        transition: Option<ContainerTransition>,
    ) -> Box<dyn Element> {
        if self.last_focus.replace(Some(options.focused)) != Some(options.focused) {
            self.client.surface_input(
                snapshot.descriptor.id,
                SurfaceInput::Focus {
                    focused: options.focused,
                },
            );
        }
        let previous = self
            .cached_frame
            .borrow()
            .as_ref()
            .map(|frame| frame.image.clone());
        let outgoing = outgoing
            .and_then(|surface| {
                surface
                    .frame
                    .as_ref()
                    .and_then(|frame| self.image_for_frame(surface.descriptor.id, frame))
            })
            .or(previous);
        let incoming = snapshot
            .frame
            .as_ref()
            .and_then(|frame| self.image_for_frame(snapshot.descriptor.id, frame));
        let corner_radius = options.corner_radius;
        let visual = self.container_transitions.borrow_mut().resolve(
            container_id,
            container_revision,
            transition,
            options.reduced_motion,
            incoming.is_some(),
            outgoing,
        );
        let child = if let Some((outgoing, started, transition)) = visual {
            Some(
                AnimatedExternalSurface::new(
                    outgoing,
                    incoming,
                    transition,
                    started,
                    corner_radius,
                )
                .finish(),
            )
        } else {
            incoming.map(|image| {
                ExternalSurface::new(image)
                    .with_corner_radius(corner_radius)
                    .finish()
            })
        };
        PluginSurfaceElement {
            client: self.client.clone(),
            snapshot,
            options,
            drag: self.drag.clone(),
            child,
            size: None,
            origin: None,
            content_origin: None,
        }
        .finish()
    }

    pub fn clear(&self) {
        self.cached_frame.borrow_mut().take();
        self.last_focus.borrow_mut().take();
        self.release_drag_capture();
        self.container_transitions.borrow_mut().clear();
    }

    pub fn release_drag_capture(&self) {
        if let Some(container_id) = self.drag.borrow_mut().release_capture() {
            self.client.set_container_drag_capture(container_id, false);
        }
    }

    fn image_for_frame(
        &self,
        surface_id: SurfaceId,
        frame: &SurfaceFrame,
    ) -> Option<Arc<StaticImage>> {
        if let Some(cached) = self.cached_frame.borrow().as_ref()
            && cached.surface_id == surface_id
            && cached.revision == frame.frame_revision
        {
            return Some(cached.image.clone());
        }
        let image = match &frame.transport {
            FrameTransport::SharedBuffer {
                bytes,
                stride_bytes,
            } => StaticImage::from_rgba(
                frame.width_px,
                frame.height_px,
                rgba_from_bgra(bytes, *stride_bytes, frame.width_px, frame.height_px)?,
            )?,
            #[cfg(target_os = "macos")]
            FrameTransport::IoSurface { surface_id, seed } => {
                let readback = warpeer_plugin_surface_transport::read_surface(
                    *surface_id,
                    *seed,
                    frame.width_px,
                    frame.height_px,
                )
                .ok()?;
                StaticImage::from_io_surface(
                    frame.width_px,
                    frame.height_px,
                    *surface_id,
                    readback.actual_seed,
                    rgba_from_bgra(
                        &readback.bgra,
                        readback.stride_bytes,
                        frame.width_px,
                        frame.height_px,
                    )?,
                )?
            }
        };
        *self.cached_frame.borrow_mut() = Some(CachedFrame {
            surface_id,
            revision: frame.frame_revision,
            image: image.clone(),
        });
        Some(image)
    }
}
