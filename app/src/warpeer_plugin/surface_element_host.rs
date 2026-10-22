use std::cell::RefCell;
use std::rc::Rc;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpeer_plugin_protocol::{SurfaceBounds, SurfaceInput, SurfacePoint};
use warpui::elements::{
    AfterLayoutContext, AppContext, Border, Element, Event, EventContext, LayoutContext,
    PaintContext, Point, SizeConstraint,
};
use warpui::event::DispatchedEvent;
use warpui::platform::Cursor;

use super::SurfacePresentationOptions;
use super::support::{color, content_size, modifier_bits};
use crate::warpeer_plugin::host::PluginHostClient;
use crate::warpeer_plugin::surface_drag::{
    SurfaceDragKey, SurfaceDragStart, SurfaceDragState, bounds_at_origin,
};
use crate::warpeer_plugin::surface_element::container_bounds;
use crate::warpeer_plugin::surface_store::SurfaceSnapshot;

pub(super) struct PluginSurfaceElement {
    pub client: PluginHostClient,
    pub snapshot: SurfaceSnapshot,
    pub options: SurfacePresentationOptions,
    pub drag: Rc<RefCell<SurfaceDragState>>,
    pub child: Option<Box<dyn Element>>,
    pub size: Option<Vector2F>,
    pub origin: Option<Point>,
    pub content_origin: Option<Point>,
}

impl Element for PluginSurfaceElement {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let size = constraint.max;
        let content_size = content_size(size, self.options.container);
        if let Some(child) = &mut self.child {
            child.layout(
                SizeConstraint {
                    min: content_size,
                    max: content_size,
                },
                ctx,
                app,
            );
        }
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        if let Some(child) = &mut self.child {
            child.after_layout(ctx, app);
        }
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let Some(size) = self.size else {
            return;
        };
        let point = Point::from_vec2f(origin, ctx.scene.z_index());
        self.origin = Some(point);
        let content_size = content_size(size, self.options.container);
        let content_origin = origin
            + Vector2F::new(
                self.options.container.border_width,
                self.options.container.border_width + self.options.container.titlebar_height,
            );
        self.content_origin = Some(Point::from_vec2f(content_origin, ctx.scene.z_index()));
        let visible = ctx
            .scene
            .visible_rect(point, size)
            .is_some_and(|bounds| bounds.size().x() > 0. && bounds.size().y() > 0.);
        self.client.present_surface(
            self.snapshot.descriptor.id,
            SurfaceBounds {
                x: self.snapshot.descriptor.bounds.x,
                y: self.snapshot.descriptor.bounds.y,
                width: content_size.x(),
                height: content_size.y(),
                scale: ctx.scene.scale_factor(),
            },
            self.options.container,
            self.options.appearance,
            self.options.palette,
            self.options.reduced_motion,
            self.options.focused,
            visible,
            !visible,
        );
        if self.options.container.border_width > 0.0 {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(origin, size))
                .with_background(color(self.options.palette.background));
        }
        if self.options.container.titlebar_height > 0.0 {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(
                    origin
                        + Vector2F::new(
                            self.options.container.border_width,
                            self.options.container.border_width,
                        ),
                    Vector2F::new(content_size.x(), self.options.container.titlebar_height),
                ))
                .with_background(color(self.options.palette.surface));
        }
        if let Some(child) = &mut self.child {
            child.paint(content_origin, ctx, app);
        }
        if self.options.container.border_width > 0.0 {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(origin, size))
                .with_border(
                    Border::all(self.options.container.border_width)
                        .with_border_fill(color(self.options.palette.border)),
                );
        }
        if let Some(region) = self.options.drag_region {
            ctx.scene.draw_rect_with_hit_recording(RectF::new(
                origin,
                Vector2F::new(size.x(), region.height.min(size.y())),
            ));
        } else if self.child.is_none()
            && (self.snapshot.descriptor.accepts_pointer
                || self.snapshot.descriptor.accepts_keyboard)
        {
            ctx.scene
                .draw_rect_with_hit_recording(RectF::new(origin, size));
        }
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        _app: &AppContext,
    ) -> bool {
        let (Some(origin), Some(size)) = (self.origin, self.size) else {
            return false;
        };
        if let Some((key, requested, viewport)) = self.drag_geometry() {
            match event.raw_event() {
                Event::LeftMouseDragged { position, .. } => {
                    let mut drag = self.drag.borrow_mut();
                    drag.prepare(key, Vector2F::new(requested.x, requested.y));
                    if let Some(start) = drag.start {
                        let proposed = start.origin + (*position - start.pointer);
                        let clamped = bounds_at_origin(requested, proposed, viewport);
                        drag.origin = Some(Vector2F::new(clamped.x, clamped.y));
                        if let SurfaceDragKey::Container(id) = key {
                            self.client.move_container(
                                id,
                                SurfacePoint {
                                    x: clamped.x,
                                    y: clamped.y,
                                },
                            );
                        }
                        ctx.set_cursor(Cursor::ClosedHand, origin.z_index());
                        ctx.notify();
                        return true;
                    }
                }
                Event::LeftMouseUp { .. } => {
                    let mut drag = self.drag.borrow_mut();
                    drag.prepare(key, Vector2F::new(requested.x, requested.y));
                    if drag.start.take().is_some() {
                        if let SurfaceDragKey::Container(id) = key {
                            self.client.set_container_drag_capture(id, false);
                        }
                        ctx.reset_cursor();
                        ctx.notify();
                        return true;
                    }
                }
                _ => {}
            }
        }
        let Some(event) = event.at_z_index(origin.z_index(), ctx) else {
            return false;
        };
        if let Some((key, requested, _)) = self.drag_geometry() {
            let pointer = match event {
                Event::MouseMoved { position, .. } | Event::LeftMouseDown { position, .. } => {
                    Some(*position)
                }
                _ => None,
            };
            if let Some(pointer) = pointer {
                let local = pointer - origin.xy();
                let inside_title = self.is_drag_point(local, size);
                match event {
                    Event::LeftMouseDown { .. } if inside_title => {
                        let mut drag = self.drag.borrow_mut();
                        drag.prepare(key, Vector2F::new(requested.x, requested.y));
                        drag.start = Some(SurfaceDragStart {
                            pointer,
                            origin: drag
                                .origin
                                .unwrap_or_else(|| Vector2F::new(requested.x, requested.y)),
                        });
                        if let SurfaceDragKey::Container(id) = key {
                            self.client.set_container_drag_capture(id, true);
                        }
                        ctx.set_cursor(Cursor::ClosedHand, origin.z_index());
                        return true;
                    }
                    Event::MouseMoved { .. } if inside_title => {
                        ctx.set_cursor(Cursor::OpenHand, origin.z_index());
                    }
                    Event::MouseMoved { .. } => ctx.reset_cursor(),
                    _ => {}
                }
            }
        }
        let content_origin = self
            .content_origin
            .expect("content origin follows outer origin");
        let content_size = content_size(size, self.options.container);
        let send_pointer = |point: Vector2F| SurfacePoint {
            x: point.x() - content_origin.x(),
            y: point.y() - content_origin.y(),
        };
        let input = match event {
            Event::MouseMoved { position, .. } => Some(SurfaceInput::PointerMove {
                point: send_pointer(*position),
            }),
            Event::LeftMouseDown { position, .. } => Some(SurfaceInput::PointerDown {
                point: send_pointer(*position),
                button: 0,
            }),
            Event::LeftMouseUp { position, .. } => Some(SurfaceInput::PointerUp {
                point: send_pointer(*position),
                button: 0,
            }),
            Event::ScrollWheel { delta, .. } => Some(SurfaceInput::Scroll {
                delta_x: delta.x(),
                delta_y: delta.y(),
            }),
            Event::KeyDown { keystroke, .. }
                if self.options.focused && keystroke.key.eq_ignore_ascii_case("escape") =>
            {
                Some(SurfaceInput::Dismiss)
            }
            Event::KeyDown { keystroke, .. } if self.options.focused => Some(SurfaceInput::Key {
                key: keystroke.key.clone(),
                modifiers: modifier_bits(keystroke),
                down: true,
            }),
            Event::TypedCharacters { chars } if self.options.focused => Some(SurfaceInput::Text {
                text: chars.clone(),
            }),
            _ => None,
        };
        input.is_some_and(|input| {
            let outside_content = match &input {
                SurfaceInput::PointerMove { point }
                | SurfaceInput::PointerDown { point, .. }
                | SurfaceInput::PointerUp { point, .. } => {
                    point.x < 0.0
                        || point.y < 0.0
                        || point.x >= content_size.x()
                        || point.y >= content_size.y()
                }
                _ => false,
            };
            if outside_content {
                return false;
            }
            let handled = self
                .client
                .surface_input(self.snapshot.descriptor.id, input);
            handled
                && (self.snapshot.descriptor.accepts_pointer
                    || self.snapshot.descriptor.accepts_keyboard)
                && size.x() > 0.
                && size.y() > 0.
        })
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }
}

impl PluginSurfaceElement {
    fn drag_geometry(&self) -> Option<(SurfaceDragKey, SurfaceBounds, SurfaceBounds)> {
        if let Some(container) = &self.options.container_drag {
            let mut requested =
                container_bounds(self.snapshot.descriptor.bounds, self.options.container);
            requested.x = container.origin.x;
            requested.y = container.origin.y;
            return Some((
                SurfaceDragKey::Container(container.id),
                requested,
                container.viewport,
            ));
        }
        self.options.drag_region.map(|region| {
            (
                SurfaceDragKey::Legacy {
                    id: self.snapshot.descriptor.id,
                    revision: self.snapshot.descriptor.revision,
                },
                container_bounds(self.snapshot.descriptor.bounds, self.options.container),
                region.viewport,
            )
        })
    }

    fn is_drag_point(&self, local: Vector2F, size: Vector2F) -> bool {
        if let Some(container) = &self.options.container_drag {
            if self.options.container.titlebar_height > 0.0 {
                return local.x() >= 0.0
                    && local.x() < size.x() - 40.0
                    && local.y() >= 0.0
                    && local.y() < self.options.container.titlebar_height.min(size.y());
            }
            let x = local.x() - self.options.container.border_width;
            let y = local.y() - self.options.container.border_width;
            return container.regions.iter().any(|region| {
                x >= region.x
                    && y >= region.y
                    && x < region.x + region.width
                    && y < region.y + region.height
            });
        }
        self.options.drag_region.is_some_and(|region| {
            local.x() >= 0.0
                && local.x() < size.x() - 40.0
                && local.y() >= 0.0
                && local.y() < region.height.min(size.y())
        })
    }
}
