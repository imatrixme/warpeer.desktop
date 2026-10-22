pub mod header;
pub mod header_content;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

use header::PaneHeader;
pub use header::PaneHeaderAction;
pub use header::PaneHeaderAction::CustomAction as PaneHeaderCustomAction;
pub use header_content::{
    HeaderContent, HeaderRenderContext, StandardHeader, StandardHeaderOptions,
};
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warpui::elements::{
    Border, ChildAnchor, ConstrainedBox, Container, DropTarget, DropTargetData, Flex, Hoverable,
    Icon, MainAxisSize, MouseStateHandle, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, SavePosition, Shrinkable, Stack,
};
use warpui::keymap::EditableBinding;
use warpui::presenter::ChildView;
use warpui::{
    AppContext, Element, Entity, EntityId, ModelHandle, SingletonEntity, TypedActionView, View,
    ViewContext, ViewHandle,
};

use super::{
    BackingView, PaneConfiguration, PaneConfigurationEvent, PaneId, PaneStack, PaneStackEvent,
};
use crate::appearance::Appearance;
use crate::pane_group::focus_state::{PaneFocusHandle, PaneGroupFocusEvent};
use crate::pane_group::pane::ActionOrigin;
use crate::pane_group::{Direction, SplitPaneState, TabBarHoverIndex};
use crate::product_profile::is_full_warp;
use crate::server::telemetry::SharingDialogSource;
use crate::settings::{PaneSettings, PaneSettingsChangedEvent};
use crate::util::bindings::CustomAction;

const HAS_SHARED_OBJECT_CONTEXT_KEY: &str = "PaneView_HasSharedObject";

/// Max width applied to the pane header while the pane renders as a floating drag preview.
/// During a pane drag the pane is laid out with unbounded constraints; `MainAxisSize::Min`
/// avoids the infinite *vertical*-constraint panic, but the header's *width* would still be
/// unbounded. Content that stretches to fill the width — the orchestration pill bar's clipped
/// horizontal scrollable — cannot be laid out with an infinite width without reporting an
/// infinite/NaN viewport and panicking in `Scene::validate_rect`. Capping the preview keeps
/// the width finite while still producing a representative header ghost.
const DRAG_PREVIEW_HEADER_MAX_WIDTH: f32 = 400.;
const PANE_INDICATOR_SIZE: f32 = 16.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneIndicatorState {
    Active,
    Hovered,
}

fn pane_indicator_state(
    pane_supports_indicator: bool,
    indicator_enabled: bool,
    pane_is_active: bool,
    pane_is_hovered: bool,
) -> Option<PaneIndicatorState> {
    if !pane_supports_indicator || !indicator_enabled {
        None
    } else if pane_is_active {
        Some(PaneIndicatorState::Active)
    } else if pane_is_hovered {
        Some(PaneIndicatorState::Hovered)
    } else {
        None
    }
}

fn inactive_pane_overlay_opacity(pane_should_be_dimmed: bool, pane_is_hovered: bool) -> Option<u8> {
    pane_should_be_dimmed.then_some(if pane_is_hovered { 50 } else { 100 })
}

fn add_pane_indicator_to_stack(
    stack: &mut Stack,
    state: PaneIndicatorState,
    appearance: &Appearance,
) {
    let color = match state {
        PaneIndicatorState::Active => appearance.theme().ui_green_color(),
        PaneIndicatorState::Hovered => appearance.theme().accent().into_solid(),
    };
    let indicator = Icon::new("bundled/svg/upper-left-triangle.svg", color).finish();
    let child = ConstrainedBox::new(indicator)
        .with_height(PANE_INDICATOR_SIZE)
        .with_width(PANE_INDICATOR_SIZE)
        .finish();
    stack.add_positioned_child(
        child,
        OffsetPositioning::offset_from_parent(
            vec2f(0., 0.),
            ParentOffsetBounds::ParentBySize,
            ParentAnchor::TopLeft,
            ChildAnchor::TopLeft,
        ),
    );
}

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_editable_bindings([EditableBinding::new(
        "pane:share_pane_contents",
        "Share pane",
        PaneAction::ShareContents,
    )
    .with_custom_action(CustomAction::SharePaneContents)
    .with_enabled(is_full_warp)
    .with_context_predicate(id!("PaneView") & id!(HAS_SHARED_OBJECT_CONTEXT_KEY))]);
}

pub enum PaneViewEvent {
    MovePaneWithinPaneGroup {
        target_id: PaneId,
        direction: Direction,
    },
    DroppedOnTabBar {
        origin: ActionOrigin,
    },
    DraggedOntoTabBar {
        origin: ActionOrigin,
        tab_hover_index: TabBarHoverIndex,
        hidden_pane_preview_direction: Direction,
        drag_position: RectF,
    },
    PaneDraggedOutsideTabBarOrPaneGroup,
    PaneDragEnded,
    PaneHeaderClicked,
}

#[derive(Debug, Clone)]
pub enum PaneAction {
    ShareContents,
}

impl<P: BackingView> Entity for PaneView<P> {
    type Event = PaneViewEvent;
}

pub struct PaneView<P: BackingView> {
    pane_id: PaneId,
    /// Navigation stack of backing views.
    pane_stack: ModelHandle<PaneStack<P>>,
    pane_configuration: ModelHandle<PaneConfiguration>,
    header: ViewHandle<PaneHeader<P>>,
    is_being_dragged: bool,
    focus_handle: Option<PaneFocusHandle>,
    hover_state: MouseStateHandle,
}

impl<P: BackingView> PaneView<P> {
    pub(super) fn new(
        pane_id: PaneId,
        child: ViewHandle<P>,
        child_data: P::AssociatedData,
        pane_configuration: ModelHandle<PaneConfiguration>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        // Create the pane stack model
        let pane_stack = ctx.add_model(|ctx| PaneStack::new(child_data, child, ctx));

        let header = ctx.add_typed_action_view(|ctx| {
            // The PaneGroup will update the split pane state for the backing view once it's attached.
            PaneHeader::new(pane_stack.clone(), pane_configuration.clone(), ctx)
        });
        ctx.subscribe_to_view(&header, |me, _, event, ctx| {
            me.handle_header_event(event, ctx)
        });

        ctx.subscribe_to_model(&pane_configuration, |me, _, event, ctx| {
            me.handle_pane_configuration_event(event, ctx);
        });

        ctx.subscribe_to_model(&pane_stack, |me, _, event, ctx| {
            me.handle_pane_stack_event(event, ctx);
        });

        ctx.subscribe_to_model(&PaneSettings::handle(ctx), |_, _, event, ctx| {
            if matches!(
                event,
                PaneSettingsChangedEvent::ShouldDimInactivePanes { .. }
                    | PaneSettingsChangedEvent::ShowActivePaneIndicator { .. }
            ) {
                ctx.notify();
            }
        });

        Self {
            pane_id,
            pane_stack,
            pane_configuration,
            header,
            is_being_dragged: false,
            focus_handle: None,
            hover_state: MouseStateHandle::default(),
        }
    }

    /// Sets the focus handle for this pane view, enabling it to track its split pane state.
    pub fn set_focus_handle(&mut self, focus_handle: PaneFocusHandle, ctx: &mut ViewContext<Self>) {
        ctx.subscribe_to_model(focus_handle.focus_state_handle(), |me, _, event, ctx| {
            me.handle_focus_state_event(event, ctx);
        });
        self.header.update(ctx, |header, ctx| {
            header.set_focus_handle(focus_handle.clone(), ctx);
        });
        // Set the focus handle for every pane in the stack.
        let pane_stack = self.pane_stack.clone();
        let views: Vec<_> = pane_stack.as_ref(ctx).views().cloned().collect();
        for view in views {
            view.update(ctx, |child, ctx| {
                child.set_focus_handle(focus_handle.clone(), ctx);
            });
        }
        self.focus_handle = Some(focus_handle);
    }

    fn handle_focus_state_event(
        &mut self,
        event: &PaneGroupFocusEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            PaneGroupFocusEvent::FocusChanged { .. }
            | PaneGroupFocusEvent::InSplitPaneChanged
            | PaneGroupFocusEvent::FocusedPaneMaximizedChanged => {
                // Re-render to update dimming and header visibility.
                ctx.notify();
            }
            PaneGroupFocusEvent::ActiveSessionChanged { .. } => {}
        }
    }

    /// Returns the pane stack model.
    pub fn pane_stack(&self) -> &ModelHandle<PaneStack<P>> {
        &self.pane_stack
    }

    /// Returns the topmost (active) child view in the navigation stack.
    pub fn child(&self, app: &AppContext) -> ViewHandle<P> {
        self.pane_stack.as_ref(app).active_view().clone()
    }

    /// Returns the associated data for the active child view in the navigation stack.
    pub fn child_data<'a>(&self, app: &'a AppContext) -> &'a P::AssociatedData {
        self.pane_stack.as_ref(app).active_data()
    }

    pub fn header(&self) -> &ViewHandle<PaneHeader<P>> {
        &self.header
    }

    pub fn is_being_dragged(&self) -> bool {
        self.is_being_dragged
    }

    /// Handles events from the pane stack model.
    fn handle_pane_stack_event(&mut self, event: &PaneStackEvent<P>, ctx: &mut ViewContext<Self>) {
        // Set the focus handle for newly added views
        if let PaneStackEvent::ViewAdded(view) = event
            && let Some(focus_handle) = &self.focus_handle
        {
            view.update(ctx, |child, ctx| {
                child.set_focus_handle(focus_handle.clone(), ctx);
            });
        }

        let new_child = self.child(ctx);

        // Refresh overflow menu items from the new active view.
        let items = new_child.read(ctx, |view, ctx| view.pane_header_overflow_menu_items(ctx));
        self.header.update(ctx, |header, ctx| {
            header.set_overflow_menu_items(items, ctx);
        });

        // Refresh toolbelt buttons from the new active view.
        let buttons = new_child.read(ctx, |view, ctx| view.pane_header_toolbelt_buttons(ctx));
        self.header.update(ctx, |header, ctx| {
            header.set_toolbelt_buttons(buttons, ctx);
        });

        // Focus the new active child.
        new_child.update(ctx, |child, ctx| child.focus_contents(ctx));

        // TODO(ben): Refresh the pane title.

        ctx.notify();
    }

    fn handle_pane_configuration_event(
        &mut self,
        event: &PaneConfigurationEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            PaneConfigurationEvent::ShowAccentBorderUpdated
            | PaneConfigurationEvent::DimEvenIfFocusedUpdated => ctx.notify(),
            PaneConfigurationEvent::RefreshPaneHeaderOverflowMenuItems
            | PaneConfigurationEvent::SharedSessionLinkChanged => {
                let child = self.child(ctx);
                let items = child.read(ctx, |view, ctx| view.pane_header_overflow_menu_items(ctx));
                self.header.update(ctx, |header, ctx| {
                    header.set_overflow_menu_items(items, ctx);
                });
                let buttons = child.read(ctx, |view, ctx| view.pane_header_toolbelt_buttons(ctx));
                self.header.update(ctx, |header, ctx| {
                    header.set_toolbelt_buttons(buttons, ctx);
                });
                if matches!(event, PaneConfigurationEvent::SharedSessionLinkChanged) {
                    self.header.update(ctx, |header, ctx| {
                        header.refresh_shared_session_link(ctx);
                    });
                }
                ctx.notify();
            }
            PaneConfigurationEvent::ShareableObjectChanged(object) => {
                self.header.update(ctx, |header, ctx| {
                    header.set_shareable_object(object.clone(), ctx);
                });
            }
            PaneConfigurationEvent::ToggleSharingDialog(source) => {
                self.header.update(ctx, |header, ctx| {
                    header.share_pane_contents(*source, ctx);
                });
            }
            PaneConfigurationEvent::OpenSharingQrCode(source) => {
                self.header.update(ctx, |header, ctx| {
                    header.open_shared_session_qr_code(*source, ctx);
                });
            }
            _ => {}
        }
    }

    fn handle_header_event(
        &mut self,
        event: &header::Event<P::PaneHeaderOverflowMenuAction, P::CustomAction>,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            header::Event::PaneHeaderClicked => ctx.emit(PaneViewEvent::PaneHeaderClicked),
            header::Event::PaneHeaderOverflowMenuToggled(is_open) => {
                self.child(ctx).update(ctx, |child, ctx| {
                    child.on_pane_header_overflow_menu_toggled(*is_open, ctx);
                });
            }
            header::Event::SelectedOverflowMenuAction(action) => {
                self.child(ctx).update(ctx, |child, ctx| {
                    child.handle_pane_header_overflow_menu_action(action, ctx);
                });
            }
            header::Event::CustomAction(action) => self.child(ctx).update(ctx, |child, ctx| {
                child.handle_custom_action(action, ctx);
            }),
            header::Event::Close => {
                // Close all views in the stack so they can clean up.
                let views: Vec<_> = self.pane_stack.as_ref(ctx).views().cloned().collect();
                for view in views {
                    view.update(ctx, |child, ctx| {
                        child.close(ctx);
                    });
                }
            }
            header::Event::MovePaneWithinPaneGroup {
                target_id,
                direction,
            } => {
                self.is_being_dragged = true;
                ctx.emit(PaneViewEvent::MovePaneWithinPaneGroup {
                    target_id: *target_id,
                    direction: *direction,
                });
                ctx.notify();
            }
            header::Event::PaneDroppedWithinPaneGroup => {
                ctx.emit(PaneViewEvent::PaneDragEnded);
                self.is_being_dragged = false;
                ctx.notify();
            }
            header::Event::DroppedOnTabBar { origin } => {
                // If we're handling a drop event for a workspace pane, we want to get rid of the neutral background that obscures it.
                if matches!(origin, ActionOrigin::Pane) {
                    self.is_being_dragged = false;
                }

                ctx.emit(PaneViewEvent::DroppedOnTabBar { origin: *origin });
                ctx.notify();
            }
            header::Event::DraggedOverTabBar {
                origin,
                tab_hover_index,
                hidden_pane_preview_direction,
                drag_position,
            } => {
                // Adds a neutral background to the pane if it's being dragged over the workspace tab group.
                if matches!(origin, ActionOrigin::Pane) {
                    self.is_being_dragged = true;
                }

                ctx.emit(PaneViewEvent::DraggedOntoTabBar {
                    origin: *origin,
                    tab_hover_index: *tab_hover_index,
                    hidden_pane_preview_direction: *hidden_pane_preview_direction,
                    drag_position: *drag_position,
                });
                ctx.notify();
            }
            header::Event::PaneDraggedOutsideTabBarOrPaneGroup => {
                self.is_being_dragged = true;
                ctx.emit(PaneViewEvent::PaneDraggedOutsideTabBarOrPaneGroup);
                ctx.notify();
            }
            header::Event::PaneDroppedOutsideofTabBarOrPaneGroup => {
                ctx.emit(PaneViewEvent::PaneDragEnded);
                self.is_being_dragged = false;
                ctx.notify();
            }
            header::Event::OverlayClosed => {
                self.child(ctx)
                    .update(ctx, |child, ctx| child.focus_contents(ctx));
            }
        }
    }

    pub fn pane_id(&self) -> PaneId {
        self.pane_id
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub struct PaneDropTargetData {
    id: PaneId,
}

impl<P: BackingView> View for PaneView<P> {
    fn ui_name() -> &'static str {
        "PaneView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let pane_configuration = self.pane_configuration.as_ref(app);

        // Check if pane is visible before deciding on flex sizing
        if !self.header.as_ref(app).is_visible_in_pane_group() {
            // When header is not visible (e.g. during drag operation), use Min sizing to avoid infinite constraint panic.
            let column = Flex::column()
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(
                    ConstrainedBox::new(ChildView::new(&self.header).finish())
                        .with_max_width(DRAG_PREVIEW_HEADER_MAX_WIDTH)
                        .finish(),
                );
            return column.finish();
        }

        // Normal case: pane is visible (i.e. not being dragged), use Max sizing to fill available space
        let mut column = Flex::column().with_main_axis_size(MainAxisSize::Max);

        let split_pane_state = self
            .focus_handle
            .as_ref()
            .map(|fh| fh.split_pane_state(app))
            .unwrap_or(SplitPaneState::NotInSplitPane);

        let active_child = self.child(app);
        let pane_supports_indicator = active_child.as_ref(app).supports_pane_indicator();

        // If being dragged, we must render the pane header, since that's what receives drag events.
        // Otherwise, if we stop rendering the header partway through a drag, the pane will be stuck
        // in its dragged state.
        if active_child.as_ref(app).should_render_header(app) || self.is_being_dragged {
            column.add_child(ChildView::new(&self.header).finish());
        }

        // Add the underlying pane view.
        column.add_child(Shrinkable::new(1., ChildView::new(&active_child).finish()).finish());

        let mut container = Container::new(column.finish());
        if pane_configuration.show_accent_border {
            let border = Border::all(2.).with_border_fill(appearance.theme().accent());
            container = container.with_border(border);
        }

        // Dim inactive panes.
        let should_dim_inactive_panes = *PaneSettings::as_ref(app).should_dim_inactive_panes;
        let dim_even_if_focused = pane_configuration.dim_even_if_focused();
        let pane_should_be_dimmed = should_dim_inactive_panes
            && (dim_even_if_focused
                || split_pane_state.is_in_split_pane() && !split_pane_state.is_focused());

        if self.is_being_dragged {
            container = container.with_foreground_overlay(appearance.theme().surface_2())
        }

        let pane_id = self.pane_id;
        let indicator_enabled = *PaneSettings::as_ref(app).show_active_pane_indicator;
        let pane_is_active = pane_configuration.show_active_pane_indicator;
        let pane_contents = container.finish();
        let pane = Hoverable::new(self.hover_state.clone(), move |mouse_state| {
            let pane_contents = if let Some(opacity) =
                inactive_pane_overlay_opacity(pane_should_be_dimmed, mouse_state.is_hovered())
            {
                Container::new(pane_contents)
                    .with_foreground_overlay(
                        appearance
                            .theme()
                            .inactive_pane_overlay()
                            .with_opacity(opacity),
                    )
                    .finish()
            } else {
                pane_contents
            };
            let pane = DropTarget::new(pane_contents, PaneDropTargetData { id: pane_id }).finish();
            let mut stack = Stack::new().with_child(pane);
            if let Some(state) = pane_indicator_state(
                pane_supports_indicator,
                indicator_enabled,
                pane_is_active,
                mouse_state.is_hovered(),
            ) {
                add_pane_indicator_to_stack(&mut stack, state, appearance);
            }
            stack.finish()
        })
        .finish();

        SavePosition::new(pane, &self.pane_id.position_id()).finish()
    }

    fn keymap_context(&self, ctx: &AppContext) -> warpui::keymap::Context {
        let mut keymap_context = Self::default_keymap_context();
        if self.header.as_ref(ctx).is_sharing_dialog_enabled(ctx) {
            keymap_context.set.insert(HAS_SHARED_OBJECT_CONTEXT_KEY);
        }
        keymap_context
    }

    fn child_view_ids(&self, app: &AppContext) -> Vec<EntityId> {
        // The backing views are owned by the `pane_stack` model, and only the
        // active (topmost) one is ever rendered (see `render`), so the
        // non-active views — and even the active one in a window that never
        // laid out — are invisible to the render-time parent graph. Report
        // all of them plus the header so the entire pane moves together when
        // it is transferred between windows; otherwise a backing view would
        // be orphaned in the source window and later trip a "circular view
        // reference" panic when accessed from its new window.
        let mut ids = vec![self.header.id()];
        ids.extend(self.pane_stack.as_ref(app).views().map(|view| view.id()));
        ids
    }
}

impl<P: BackingView> TypedActionView for PaneView<P> {
    type Action = PaneAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        if !is_full_warp() {
            return;
        }

        match action {
            PaneAction::ShareContents => self.header.update(ctx, |header, ctx| {
                header.share_pane_contents(SharingDialogSource::CommandPalette, ctx);
            }),
        }
    }
}

impl DropTargetData for PaneDropTargetData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
