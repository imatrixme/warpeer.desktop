use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpeer_plugin_protocol::{ChromeVisibility, SurfaceBounds, SurfaceContainerMetrics};
use warpui::elements::{
    ChildAnchor, ChildView, Clipped, ConstrainedBox, Empty, Hoverable, OffsetPositioning,
    ParentAnchor, ParentOffsetBounds, PositionedElementAnchor, PositionedElementOffsetBounds,
    Stack,
};

use super::*;

impl TerminalView {
    pub(in crate::terminal::view) fn render_plugin_hover(
        &self,
        stack: &mut Stack,
        app: &AppContext,
    ) {
        let Some(active) = &self.plugin_hover.active else {
            return;
        };
        let container = active
            .pinned
            .then(|| self.plugin_hover.client.terminal_container(&active.context))
            .flatten();
        let legacy = container
            .is_none()
            .then(|| self.plugin_hover.client.terminal_surface(&active.context))
            .flatten();
        if let Some(snapshot) = container
            .as_ref()
            .map(|container| container.surface.clone())
            .or_else(|| legacy.clone())
        {
            let viewport = SurfaceBounds {
                x: active.context.viewport[0],
                y: active.context.viewport[1],
                width: active.context.viewport[2],
                height: active.context.viewport[3],
                scale: 1.,
            };
            let bounds = if let Some(container) = container.as_ref() {
                let page = container
                    .container
                    .active_page()
                    .expect("container active page exists");
                let metrics = container_metrics(page.chrome.visibility);
                let mut requested = container_bounds(snapshot.descriptor.bounds, metrics);
                requested.x = container.container.origin.x;
                requested.y = container.container.origin.y;
                let outgoing = container
                    .transition_page
                    .as_ref()
                    .zip(container.transition_surface.as_ref())
                    .map(|(page, surface)| {
                        container_bounds(
                            surface.descriptor.bounds,
                            container_metrics(page.chrome.visibility),
                        )
                    });
                let reduced_motion = SurfacePresentationOptions::standard(
                    app,
                    false,
                    warpui::elements::CornerRadius::with_all(warpui::elements::Radius::Pixels(0.0)),
                )
                .reduced_motion;
                self.plugin_hover.presentation.display_container_bounds(
                    requested,
                    viewport,
                    container.container.container_id,
                    container.container.origin,
                    container.container.revision,
                    container.container.transition,
                    reduced_motion,
                    outgoing,
                )
            } else {
                let requested = anchored_preview_bounds(
                    active,
                    container_bounds(
                        snapshot.descriptor.bounds,
                        plugin_window_container_metrics(),
                    ),
                );
                self.plugin_hover.presentation.display_bounds(
                    requested,
                    viewport,
                    snapshot.descriptor.id,
                    snapshot.descriptor.revision,
                )
            };
            let width = bounds
                .width
                .max(1.)
                .min(self.size_info.pane_width_px().as_f32());
            let height = bounds
                .height
                .max(1.)
                .min(self.size_info.pane_height_px().as_f32());
            let x = bounds
                .x
                .max(0.)
                .min((self.size_info.pane_width_px().as_f32() - width).max(0.));
            let y = bounds
                .y
                .max(0.)
                .min((self.size_info.pane_height_px().as_f32() - height).max(0.));
            let surface = Clipped::new(
                ConstrainedBox::new(if let Some(container) = container {
                    self.plugin_hover.render_container(container, app)
                } else {
                    self.plugin_hover.render(snapshot, app)
                })
                .with_width(width)
                .with_height(height)
                .finish(),
            )
            .finish();
            let surface = if active.pinned {
                surface
            } else {
                Hoverable::new(self.plugin_hover.surface_mouse_state.clone(), |_| surface)
                    .on_hover(|is_hovered, ctx, _, _| {
                        ctx.dispatch_typed_action(TerminalAction::PluginPreviewSurfaceHover {
                            is_hovered,
                        });
                    })
                    .on_mouse_down(|ctx, _, _| {
                        ctx.dispatch_typed_action(TerminalAction::PluginPreviewSurfaceHover {
                            is_hovered: true,
                        });
                    })
                    .with_skip_synthetic_hover_out()
                    .finish()
            };
            stack.add_positioned_overlay_child(
                surface,
                OffsetPositioning::offset_from_parent(
                    vec2f(x, y),
                    ParentOffsetBounds::ParentByPosition,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );
            if active.pinned {
                stack.add_positioned_overlay_child(
                    ConstrainedBox::new(ChildView::new(&self.plugin_hover_close_button).finish())
                        .with_width(18.)
                        .with_height(18.)
                        .finish(),
                    OffsetPositioning::offset_from_parent(
                        vec2f(x + width - 22., y + 3.),
                        ParentOffsetBounds::ParentByPosition,
                        ParentAnchor::TopLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
            if let Some(position_id) = &self.plugin_hover.open_action_menu_position {
                stack.add_positioned_overlay_child(
                    ChildView::new(&self.plugin_hover.action_menu).finish(),
                    OffsetPositioning::offset_from_save_position_element(
                        position_id.clone(),
                        vec2f(0.0, 4.0),
                        PositionedElementOffsetBounds::WindowByPosition,
                        PositionedElementAnchor::BottomLeft,
                        ChildAnchor::TopLeft,
                    ),
                );
            }
        } else {
            stack.add_positioned_child(
                ConstrainedBox::new(Empty::new().finish())
                    .with_width(1.)
                    .with_height(1.)
                    .finish(),
                OffsetPositioning::offset_from_parent(
                    Vector2F::zero(),
                    ParentOffsetBounds::ParentByPosition,
                    ParentAnchor::TopLeft,
                    ChildAnchor::TopLeft,
                ),
            );
        }
    }
}

fn container_metrics(visibility: ChromeVisibility) -> SurfaceContainerMetrics {
    match visibility {
        ChromeVisibility::Visible => plugin_window_container_metrics(),
        ChromeVisibility::Hidden => SurfaceContainerMetrics {
            border_width: plugin_window_container_metrics().border_width,
            titlebar_height: 0.0,
        },
    }
}

fn anchored_preview_bounds(active: &ActiveHover, mut bounds: SurfaceBounds) -> SurfaceBounds {
    const MARGIN: f32 = 8.0;
    const ANCHOR_GAP: f32 = 10.0;
    let [viewport_x, viewport_y, viewport_width, viewport_height] = active.context.viewport;
    let [anchor_x, anchor_y, anchor_width, anchor_height] = active.context.anchor;
    let max_x = (viewport_x + viewport_width - bounds.width - MARGIN).max(viewport_x + MARGIN);
    bounds.x =
        (anchor_x + anchor_width * 0.5 - bounds.width * 0.5).clamp(viewport_x + MARGIN, max_x);
    let below = anchor_y + anchor_height + ANCHOR_GAP;
    bounds.y = if below + bounds.height <= viewport_y + viewport_height - MARGIN {
        below
    } else {
        (anchor_y - bounds.height - ANCHOR_GAP).max(viewport_y + MARGIN)
    };
    bounds
}
