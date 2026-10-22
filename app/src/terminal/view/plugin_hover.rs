use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{ContextId, Revision, TerminalHoverContext};
use warpui::r#async::Timer;
use warpui::elements::{Empty, MouseStateHandle};
use warpui::{AppContext, Element, SingletonEntity as _, ViewContext, ViewHandle};

use super::{GridHighlightedLink, TerminalAction, TerminalView};
use crate::menu::{Event as MenuEvent, Menu};
use crate::terminal::model::RespectObfuscatedSecrets;
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::WithinModel;
use crate::warpeer_plugin::{
    PLUGIN_WINDOW_TITLEBAR_HEIGHT, PluginHostClient, PluginSurfacePresentation, PresentedContainer,
    SurfacePresentationOptions, SurfaceSnapshot, WarpeerPluginHost, container_bounds,
    plugin_window_container_metrics,
};

#[path = "plugin_hover_resource.rs"]
mod resource;
use resource::{is_supported_image_target, snapshot_grid_link};

#[path = "plugin_hover_presentation.rs"]
mod presentation;

#[path = "plugin_container_presentation.rs"]
mod container_presentation;

#[path = "plugin_container_menu.rs"]
mod container_menu;

#[cfg(feature = "integration_tests")]
#[path = "plugin_hover_integration.rs"]
mod integration;

static NEXT_HOVER_REVISION: AtomicU64 = AtomicU64::new(1);
const HOVER_DISMISS_GRACE: Duration = Duration::from_millis(2500);
const IMAGE_PREVIEW_PLUGIN_ID: &str = "dev.warpeer.image-preview";

#[derive(Default)]
struct HoverDismissalState {
    generation: u64,
    surface_hovered: bool,
}

impl HoverDismissalState {
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    fn schedule(&mut self) -> Option<u64> {
        if self.surface_hovered {
            return None;
        }
        self.invalidate();
        Some(self.generation)
    }

    fn set_surface_hovered(&mut self, is_hovered: bool) -> Option<u64> {
        self.surface_hovered = is_hovered;
        self.invalidate();
        (!is_hovered).then_some(self.generation)
    }

    fn should_cancel(&self, generation: u64) -> bool {
        !self.surface_hovered && self.generation == generation
    }

    fn reset(&mut self) {
        self.surface_hovered = false;
        self.invalidate();
    }
}

struct ActiveHover {
    context: TerminalHoverContext,
    target: String,
    pinned: bool,
}

pub(super) struct PluginHoverState {
    client: PluginHostClient,
    active: Option<ActiveHover>,
    last_position: Option<WithinModel<Point>>,
    last_pointer: Option<Vector2F>,
    dismissal: HoverDismissalState,
    surface_mouse_state: MouseStateHandle,
    action_mouse_states: RefCell<HashMap<String, MouseStateHandle>>,
    action_menu: ViewHandle<Menu<TerminalAction>>,
    open_action_menu_position: Option<String>,
    presentation: PluginSurfacePresentation,
}

impl PluginHoverState {
    pub fn new(ctx: &mut ViewContext<TerminalView>) -> Self {
        let client = WarpeerPluginHost::as_ref(ctx).client();
        let action_menu = ctx.add_typed_action_view(|_| {
            Menu::new()
                .with_width(220.0)
                .with_drop_shadow()
                .prevent_interaction_with_other_elements()
        });
        ctx.subscribe_to_view(&action_menu, |terminal, _, event, ctx| {
            if matches!(event, MenuEvent::Close { .. }) {
                terminal.plugin_hover.open_action_menu_position = None;
                ctx.notify();
            }
        });
        Self {
            presentation: PluginSurfacePresentation::new(client.clone()),
            client,
            active: None,
            last_position: None,
            last_pointer: None,
            dismissal: HoverDismissalState::default(),
            surface_mouse_state: MouseStateHandle::default(),
            action_mouse_states: RefCell::default(),
            action_menu,
            open_action_menu_position: None,
        }
    }

    pub fn last_position(&self) -> Option<WithinModel<Point>> {
        self.last_position
    }

    pub fn last_pointer(&self) -> Option<Vector2F> {
        self.last_pointer
    }

    fn is_pinned(&self) -> bool {
        self.active.as_ref().is_some_and(|active| active.pinned)
    }

    pub fn cancel(&mut self) {
        self.dismissal.reset();
        if let Ok(mut state) = self.surface_mouse_state.lock() {
            state.reset_interaction_state();
        }
        if let Some(active) = self.active.take() {
            self.client
                .cancel_hover(active.context.context_id, active.context.revision);
        }
        self.presentation.clear();
        self.action_mouse_states.borrow_mut().clear();
        self.open_action_menu_position = None;
    }

    pub(in crate::terminal::view) fn release_drag_capture(&self) {
        self.presentation.release_drag_capture();
    }

    fn update(
        &mut self,
        context: TerminalHoverContext,
        position: WithinModel<Point>,
        pointer: Option<Vector2F>,
    ) -> bool {
        self.last_position = Some(position);
        self.last_pointer = pointer;
        self.dismissal.invalidate();
        let pinned = context.explicit_preview;
        let target = context
            .resolved_resource
            .as_deref()
            .unwrap_or(&context.resource_text);
        if pinned
            && self
                .active
                .as_ref()
                .is_some_and(|active| active.target == target)
        {
            let active = self.active.as_mut().expect("matching hover is active");
            let needs_dispatch = self.client.terminal_container(&active.context).is_none()
                && self.client.terminal_surface(&active.context).is_none();
            active.pinned = true;
            active.context.explicit_preview = true;
            let upgraded = active.context.clone();
            if needs_dispatch {
                self.client.terminal_hover(upgraded);
            }
            return true;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.target == target && active.pinned == pinned)
        {
            return false;
        }
        if self.is_pinned() && !pinned {
            return false;
        }
        self.cancel();
        self.active = Some(ActiveHover {
            target: context
                .resolved_resource
                .clone()
                .unwrap_or_else(|| context.resource_text.clone()),
            pinned,
            context,
        });
        self.client.terminal_hover(
            self.active
                .as_ref()
                .expect("hover context was stored")
                .context
                .clone(),
        );
        true
    }

    fn clear_position(&mut self) {
        self.last_position = None;
        self.last_pointer = None;
    }

    fn schedule_cancel(&mut self, clear_position: bool) -> Option<u64> {
        if clear_position {
            self.clear_position();
        }
        self.dismissal.schedule()
    }

    fn should_cancel(&self, generation: u64) -> bool {
        self.dismissal.should_cancel(generation)
    }

    #[cfg(feature = "integration_tests")]
    fn integration_update(&mut self, context: TerminalHoverContext) {
        self.cancel();
        self.active = Some(ActiveHover {
            target: context
                .resolved_resource
                .clone()
                .unwrap_or_else(|| context.resource_text.clone()),
            pinned: context.explicit_preview,
            context,
        });
        self.client.terminal_hover(
            self.active
                .as_ref()
                .expect("integration hover context was stored")
                .context
                .clone(),
        );
    }

    fn render(&self, snapshot: SurfaceSnapshot, app: &AppContext) -> Box<dyn Element> {
        let Some(active) = &self.active else {
            return Empty::new().finish();
        };
        let mut options = SurfacePresentationOptions::standard(
            app,
            false,
            warpui::elements::CornerRadius::with_all(warpui::elements::Radius::Pixels(0.)),
        )
        .with_window_container();
        if active.pinned {
            options = options.with_drag_region(
                PLUGIN_WINDOW_TITLEBAR_HEIGHT,
                warpeer_plugin_protocol::SurfaceBounds {
                    x: active.context.viewport[0],
                    y: active.context.viewport[1],
                    width: active.context.viewport[2],
                    height: active.context.viewport[3],
                    scale: 1.,
                },
            );
        }
        self.presentation.element(snapshot, options)
    }

    fn render_container(
        &self,
        presented: PresentedContainer,
        app: &AppContext,
    ) -> Box<dyn Element> {
        container_presentation::render(self, presented, app)
    }

    fn action_mouse_state(&self, key: String) -> MouseStateHandle {
        self.action_mouse_states
            .borrow_mut()
            .entry(key)
            .or_default()
            .clone()
    }
}

impl TerminalView {
    pub(super) fn sync_plugin_hover(
        &mut self,
        position: Option<WithinModel<Point>>,
        pointer: Option<Vector2F>,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.plugin_hover.is_pinned() {
            return;
        }
        let Some(position) = position else {
            self.schedule_plugin_hover_cancel(true, ctx);
            return;
        };
        self.plugin_hover.last_position = Some(position);
        self.plugin_hover.last_pointer = pointer;
        let Some(link) = self.highlighted_link.clone_inner() else {
            self.schedule_plugin_hover_cancel(false, ctx);
            return;
        };
        let context = self.plugin_hover_context(&link, position, pointer, false, ctx);
        if self.plugin_hover.update(context, position, pointer) {
            ctx.notify();
        }
    }

    fn plugin_hover_context(
        &mut self,
        link: &GridHighlightedLink,
        position: WithinModel<Point>,
        pointer: Option<Vector2F>,
        explicit_preview: bool,
        ctx: &AppContext,
    ) -> TerminalHoverContext {
        let (resource_text, resolved_resource, explicit_hyperlink) =
            snapshot_without_retaining(|| {
                snapshot_grid_link(&self.model.lock(), link, RespectObfuscatedSecrets::No)
            });
        let point = position.get_inner();
        let cell_width = self.size_info.cell_width_px().as_f32();
        let cell_height = self.size_info.cell_height_px().as_f32();
        let (x, y, anchor_width, anchor_height) = pointer.map_or_else(
            || {
                let x = self.size_info.padding_x_px().as_f32() + point.col as f32 * cell_width;
                let y = self.size_info.padding_y_px().as_f32()
                    + (point.row.max(0) as usize).min(self.size_info.rows().saturating_sub(1))
                        as f32
                        * cell_height;
                (x, y, cell_width, cell_height)
            },
            |pointer| (pointer.x(), pointer.y(), 1., 1.),
        );
        let revision = Revision(NEXT_HOVER_REVISION.fetch_add(1, Ordering::Relaxed));
        TerminalHoverContext {
            context_id: ContextId::new(),
            revision,
            terminal_id: format!("{:?}", self.view_id),
            pane_id: self.focus_handle.as_ref().map_or_else(
                || format!("{:?}", self.view_id),
                |handle| format!("{:?}", handle.pane_id()),
            ),
            workspace_id: None,
            working_directory: self
                .active_session_path_if_local(ctx)
                .map(|path| path.to_string_lossy().into_owned()),
            resource_text,
            resolved_resource,
            explicit_hyperlink,
            explicit_preview,
            anchor: [x, y, anchor_width, anchor_height],
            viewport: [
                0.0,
                0.0,
                self.size_info.pane_width_px().as_f32(),
                self.size_info.pane_height_px().as_f32(),
            ],
        }
    }

    pub(super) fn can_manually_preview_grid_image(
        &self,
        link: &GridHighlightedLink,
        model: &crate::terminal::TerminalModel,
        app: &AppContext,
    ) -> bool {
        let (_, resolved_resource, _) =
            snapshot_grid_link(model, link, RespectObfuscatedSecrets::Yes);
        let Some(target) = resolved_resource else {
            return false;
        };
        let plugin_enabled = WarpeerPluginHost::as_ref(app)
            .inventory()
            .ok()
            .and_then(|plugins| {
                plugins.into_iter().find(|plugin| {
                    plugin.manifest.id.to_string() == IMAGE_PREVIEW_PLUGIN_ID
                        && plugin.enabled
                        && plugin.manifest.accepts_terminal_hover(&target)
                        && plugin
                            .settings
                            .get("trigger_mode")
                            .and_then(serde_json::Value::as_str)
                            .map(|mode| !mode.eq_ignore_ascii_case("hover"))
                            .unwrap_or(true)
                })
            })
            .is_some();
        plugin_enabled && is_supported_image_target(&target)
    }

    pub(super) fn preview_grid_image(
        &mut self,
        link: &GridHighlightedLink,
        position: WithinModel<Point>,
        pointer: Option<Vector2F>,
        ctx: &mut ViewContext<Self>,
    ) {
        let context = self.plugin_hover_context(link, position, pointer, true, ctx);
        if self.plugin_hover.update(context, position, pointer) {
            self.open_grid_link_tool_tip = None;
            ctx.notify();
        }
    }

    pub(super) fn preview_grid_image_action(
        &self,
        link: &GridHighlightedLink,
        pointer: Option<Vector2F>,
    ) -> TerminalAction {
        let position = self
            .plugin_hover
            .last_position()
            .filter(|position| link.contains(position))
            .unwrap_or_else(|| link.anchor_position());
        TerminalAction::PreviewGridImage {
            link: link.clone(),
            position,
            pointer: pointer.or_else(|| self.plugin_hover.last_pointer()),
        }
    }

    fn schedule_plugin_hover_cancel(&mut self, clear_position: bool, ctx: &mut ViewContext<Self>) {
        if self.plugin_hover.is_pinned() {
            return;
        }
        if self.plugin_hover.active.is_none() {
            if clear_position {
                self.plugin_hover.clear_position();
            }
            return;
        }
        let Some(generation) = self.plugin_hover.schedule_cancel(clear_position) else {
            return;
        };
        self.schedule_plugin_hover_cancel_generation(generation, ctx);
    }

    fn schedule_plugin_hover_cancel_generation(
        &mut self,
        generation: u64,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.spawn(
            async move { Timer::after(HOVER_DISMISS_GRACE).await },
            move |terminal, _, ctx| {
                if terminal.plugin_hover.should_cancel(generation) {
                    terminal.cancel_plugin_hover(ctx);
                }
            },
        );
    }

    pub(super) fn plugin_preview_surface_hover_changed(
        &mut self,
        is_hovered: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if self.plugin_hover.is_pinned() {
            return;
        }
        if let Some(generation) = self.plugin_hover.dismissal.set_surface_hovered(is_hovered) {
            self.schedule_plugin_hover_cancel_generation(generation, ctx);
        }
    }

    pub(super) fn cancel_plugin_hover(&mut self, ctx: &mut ViewContext<Self>) {
        self.plugin_hover.clear_position();
        if self.plugin_hover.active.is_some() {
            self.plugin_hover.cancel();
            ctx.notify();
        }
    }

    pub(super) fn cancel_transient_plugin_hover(&mut self, ctx: &mut ViewContext<Self>) {
        if !self.plugin_hover.is_pinned() {
            self.cancel_plugin_hover(ctx);
        }
    }
}

fn snapshot_without_retaining<T>(snapshot: impl FnOnce() -> T) -> T {
    snapshot()
}
#[cfg(test)]
#[path = "plugin_hover_tests.rs"]
mod tests;
