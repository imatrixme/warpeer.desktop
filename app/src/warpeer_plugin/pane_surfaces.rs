use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use pathfinder_geometry::vector::vec2f;
use warpeer_plugin_protocol::{ContextId, PaneContext, Revision, SurfaceId, SurfaceSlot};
use warpui::elements::{
    ChildAnchor, Clipped, ConstrainedBox, Dismiss, EventDispatchMode, OffsetPositioning,
    ParentAnchor, ParentElement, ParentOffsetBounds, Stack,
};
use warpui::{AppContext, Element, SingletonEntity as _};

use super::{
    PluginHostClient, PluginSurfacePresentation, SurfacePresentationOptions, WarpeerPluginHost,
    container_bounds, plugin_window_container_metrics,
};

static NEXT_PANE_REVISION: AtomicU64 = AtomicU64::new(1);

pub(crate) struct PanePluginSurfaceState {
    client: PluginHostClient,
    context: PaneContext,
    presentations: RefCell<HashMap<SurfaceId, PluginSurfacePresentation>>,
}

impl PanePluginSurfaceState {
    pub fn new(pane_id: String, app: &AppContext) -> Self {
        let client = WarpeerPluginHost::as_ref(app).client();
        let context = PaneContext {
            context_id: ContextId::new(),
            revision: Revision(NEXT_PANE_REVISION.fetch_add(1, Ordering::Relaxed)),
            pane_id,
            workspace_id: None,
            working_directory: None,
        };
        client.pane_context(context.clone());
        Self {
            client,
            context,
            presentations: RefCell::default(),
        }
    }

    pub fn render(
        &self,
        pane: Box<dyn Element>,
        focused: bool,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let surfaces = self.client.pane_surfaces(&self.context.pane_id);
        self.presentations.borrow_mut().retain(|surface_id, _| {
            surfaces
                .iter()
                .any(|surface| surface.descriptor.id == *surface_id)
        });
        if surfaces.is_empty() {
            return pane;
        }
        let mut stack = Stack::new().with_event_dispatch_mode(EventDispatchMode::Waterfall);
        for surface in surfaces
            .iter()
            .filter(|surface| surface.descriptor.slot == SurfaceSlot::PaneBackground)
        {
            stack.add_child(Clipped::new(self.surface(surface, focused, app)).finish());
        }
        stack.add_child(pane);
        for surface in surfaces
            .iter()
            .filter(|surface| surface.descriptor.slot == SurfaceSlot::PaneOverlay)
        {
            stack.add_child(Clipped::new(self.surface(surface, focused, app)).finish());
        }
        for surface in surfaces.iter().filter(|surface| {
            matches!(
                surface.descriptor.slot,
                SurfaceSlot::PaneHeader | SurfaceSlot::PanePopover
            )
        }) {
            self.add_positioned_surface(&mut stack, surface, focused, app);
        }
        stack.finish()
    }

    fn surface(
        &self,
        snapshot: &super::surface_store::SurfaceSnapshot,
        focused: bool,
        app: &AppContext,
    ) -> Box<dyn Element> {
        let mut presentations = self.presentations.borrow_mut();
        let presentation = presentations
            .entry(snapshot.descriptor.id)
            .or_insert_with(|| PluginSurfacePresentation::new(self.client.clone()));
        let mut options = SurfacePresentationOptions::standard(
            app,
            focused,
            warpui::elements::CornerRadius::default(),
        );
        if snapshot.descriptor.slot == SurfaceSlot::PanePopover {
            options = options.with_window_container();
        }
        presentation.element(snapshot.clone(), options)
    }

    fn add_positioned_surface(
        &self,
        stack: &mut Stack,
        snapshot: &super::surface_store::SurfaceSnapshot,
        focused: bool,
        app: &AppContext,
    ) {
        let bounds = if snapshot.descriptor.slot == SurfaceSlot::PanePopover {
            container_bounds(
                snapshot.descriptor.bounds,
                plugin_window_container_metrics(),
            )
        } else {
            snapshot.descriptor.bounds
        };
        let child = Clipped::new(
            ConstrainedBox::new(self.surface(snapshot, focused, app))
                .with_width(bounds.width.max(1.))
                .with_height(bounds.height.max(1.))
                .finish(),
        )
        .finish();
        let (child, offset, parent_anchor, child_anchor) = match snapshot.descriptor.slot {
            SurfaceSlot::PaneHeader => (
                child,
                vec2f(-8., 8.),
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
            SurfaceSlot::PanePopover => {
                let client = self.client.clone();
                let surface_id = snapshot.descriptor.id;
                (
                    Dismiss::new(child)
                        .on_dismiss(move |_, _| {
                            client.surface_input(
                                surface_id,
                                warpeer_plugin_protocol::SurfaceInput::Dismiss,
                            );
                        })
                        .finish(),
                    vec2f(bounds.x.max(0.), bounds.y.max(0.)),
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                )
            }
            _ => return,
        };
        stack.add_positioned_overlay_child(
            child,
            OffsetPositioning::offset_from_parent(
                offset,
                ParentOffsetBounds::ParentByPosition,
                parent_anchor,
                child_anchor,
            ),
        );
    }
}

impl Drop for PanePluginSurfaceState {
    fn drop(&mut self) {
        self.client
            .cancel_context(self.context.context_id, self.context.revision);
    }
}
