use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpui::elements::Point;
use warpui::prelude::{Border, Fill};
use warpui::terminal_visuals::{TerminalCellRef, TerminalVisualLayer};
use warpui::{
    AfterLayoutContext, AppContext, ClipBounds, Element, EventContext, LayoutContext, PaintContext,
    SingletonEntity, SizeConstraint,
};

use super::visual_runtime::{TerminalVisualConfig, TerminalVisualRuntime};
use crate::appearance::Appearance;
use crate::editor::PlainTextEditorViewAction;

#[path = "input_visual_reconciliation.rs"]
mod reconciliation;
#[cfg(test)]
use reconciliation::input_text_snapshot;
use reconciliation::{
    InputCursorSnapshot, InputEditDirection, InputGlyphSnapshot, InputTextSnapshot,
    input_edit_direction, input_glyph_before_cursor, input_text_snapshot_with_previous,
    is_single_glyph_removal, points_are_close, reconcile_glyph_change, same_glyph_sequence,
    sorted_glyphs,
};

#[derive(Clone, Copy, Debug)]
struct ActiveInputGlyph {
    hash: u64,
    position: Vector2F,
}

#[derive(Clone, Copy)]
struct PendingInputEdit {
    direction: InputEditDirection,
    target_text: InputTextSnapshot,
}

pub struct TerminalInputVisualRuntime {
    core: TerminalVisualRuntime,
    previous: Option<Vec<InputGlyphSnapshot>>,
    previous_text: Option<InputTextSnapshot>,
    active: Option<ActiveInputGlyph>,
    composition_active: bool,
    last_cursor: Option<InputCursorSnapshot>,
    last_cell_size: Option<Vector2F>,
    pending_edit: Option<PendingInputEdit>,
    last_edit_intent: Option<PendingInputEdit>,
}

impl TerminalInputVisualRuntime {
    pub fn new(config: TerminalVisualConfig) -> Self {
        Self {
            core: TerminalVisualRuntime::new_input(config),
            previous: None,
            previous_text: None,
            active: None,
            composition_active: false,
            last_cursor: None,
            last_cell_size: None,
            pending_edit: None,
            last_edit_intent: None,
        }
    }

    fn render_frame(
        &mut self,
        input: InputVisualFrame<'_>,
    ) -> (Arc<TerminalVisualLayer>, Option<(usize, TerminalCellRef)>) {
        let InputVisualFrame {
            mut config,
            bounds,
            cell_size,
            composition_active,
            text,
            glyphs,
            cursor,
            cursor_visible,
        } = input;
        config.material.substrate_enabled = false;
        config.material.command_gutter = false;
        config.material.block_depth = false;
        self.core.begin_frame(config, bounds, cell_size, true);
        self.last_cell_size = Some(cell_size);
        let current = sorted_glyphs(glyphs);
        let composition_committed = self.composition_active && !composition_active;
        if let Some(cursor) = cursor {
            self.last_cursor = Some(cursor);
        }
        let implicit_single_glyph_removal = self
            .previous
            .as_deref()
            .is_some_and(|previous| is_single_glyph_removal(previous, &current));
        let candidate_direction = if !composition_active {
            input_edit_direction(
                self.previous_text,
                text,
                composition_committed,
                implicit_single_glyph_removal,
            )
        } else {
            InputEditDirection::Stationary
        };
        let observed_direction = if candidate_direction != InputEditDirection::Stationary
            && self.last_edit_intent.is_some_and(|intent| {
                intent.direction == candidate_direction
                    && intent.target_text.has_same_buffer_state(text)
            }) {
            InputEditDirection::Stationary
        } else {
            candidate_direction
        };
        if observed_direction != InputEditDirection::Stationary {
            let edit = PendingInputEdit {
                direction: observed_direction,
                target_text: text,
            };
            if observed_direction == InputEditDirection::Backward {
                self.pending_edit = None;
            } else {
                self.pending_edit = Some(edit);
            }
            self.last_edit_intent = Some(edit);
        } else if self
            .pending_edit
            .is_some_and(|pending| !pending.target_text.has_same_buffer_state(text))
        {
            self.pending_edit = None;
        }
        if composition_active {
            self.pending_edit = None;
        }
        let mut direction = observed_direction;
        let mut effective_cursor = cursor_visible.then_some(cursor).flatten();

        let mut inserted_scene_index = None;
        if composition_active {
            self.active = None;
            self.core.clear_cell_motion();
        } else if let Some(previous) = self.previous.as_deref() {
            if !same_glyph_sequence(previous, &current) {
                direction = self
                    .pending_edit
                    .map_or(observed_direction, |pending| pending.direction);
                let reconciliation_cursor = if direction == InputEditDirection::Backward {
                    effective_cursor
                } else {
                    cursor
                };
                let change =
                    reconcile_glyph_change(previous, &current, reconciliation_cursor, direction);
                let consumes_pending_edit = match direction {
                    InputEditDirection::Forward => change.inserted_scene_index.is_some(),
                    InputEditDirection::Backward => !change.removed.is_empty(),
                    InputEditDirection::Stationary => false,
                };
                if cursor.is_none()
                    && direction == InputEditDirection::Backward
                    && let (Some(previous_cursor), Some(removed)) =
                        (self.last_cursor, change.removed.first())
                {
                    let inferred = InputCursorSnapshot {
                        bounds: RectF::new(
                            Vector2F::new(removed.position.x(), previous_cursor.bounds.origin_y()),
                            previous_cursor.bounds.size(),
                        ),
                        color: previous_cursor.color,
                    };
                    self.last_cursor = Some(inferred);
                    effective_cursor = cursor_visible.then_some(inferred);
                }
                self.core.clear_cell_motion();
                self.active = change.inserted_scene_index.and_then(|scene_index| {
                    current
                        .iter()
                        .find(|glyph| glyph.scene_index == scene_index)
                        .map(|glyph| ActiveInputGlyph {
                            hash: glyph.hash,
                            position: glyph.position,
                        })
                });
                inserted_scene_index = change.inserted_scene_index;
                if consumes_pending_edit {
                    self.pending_edit = None;
                }
            } else if composition_committed && direction == InputEditDirection::Forward {
                inserted_scene_index =
                    cursor.and_then(|cursor| input_glyph_before_cursor(&current, cursor));
                self.core.clear_cell_motion();
                self.active = inserted_scene_index.and_then(|scene_index| {
                    current
                        .iter()
                        .find(|glyph| glyph.scene_index == scene_index)
                        .map(|glyph| ActiveInputGlyph {
                            hash: glyph.hash,
                            position: glyph.position,
                        })
                });
            }
            self.previous = Some(current.clone());
        } else {
            self.previous = Some(current.clone());
        }
        if let Some(cursor) = effective_cursor {
            let duration_scale = if direction == InputEditDirection::Backward {
                0.25
            } else {
                1.
            };
            self.core.observe_cursor_with_duration_scale(
                cursor.bounds,
                cursor.color,
                duration_scale,
            );
        } else {
            self.core.clear_cursor_motion();
        }
        if !composition_active {
            self.previous_text = Some(text);
        }
        self.composition_active = composition_active;

        for glyph in &current {
            self.core.observe_static_cell(glyph.position, glyph.hash);
        }

        let active_scene_index = inserted_scene_index.or_else(|| {
            let active = self.active?;
            current
                .iter()
                .find(|glyph| {
                    glyph.hash == active.hash && points_are_close(glyph.position, active.position)
                })
                .map(|glyph| glyph.scene_index)
        });
        let cell_ref = active_scene_index.and_then(|scene_index| {
            let glyph = current
                .iter()
                .find(|glyph| glyph.scene_index == scene_index)?;
            if inserted_scene_index == Some(scene_index) {
                self.core
                    .start_input_cell_motion(glyph.position, glyph.hash)
            } else {
                self.core.observe_cell(glyph.position, glyph.hash, false)
            }
        });
        if active_scene_index.is_some() && cell_ref.is_none() {
            self.active = None;
        }

        let frame = self.core.finish_frame();
        (frame, active_scene_index.zip(cell_ref))
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn render_test_frame(
        &mut self,
        config: TerminalVisualConfig,
        bounds: RectF,
        cell_size: Vector2F,
        composition_active: bool,
        text: InputTextSnapshot,
        glyphs: &[InputGlyphSnapshot],
        cursor: Option<InputCursorSnapshot>,
    ) -> (Arc<TerminalVisualLayer>, Option<(usize, TerminalCellRef)>) {
        self.render_frame(InputVisualFrame {
            config,
            bounds,
            cell_size,
            composition_active,
            text,
            glyphs,
            cursor,
            cursor_visible: cursor.is_some(),
        })
    }

    pub fn input_backspace(&mut self) {
        let Some(cell_size) = self.last_cell_size else {
            return;
        };
        self.emit_erase_effect(None, self.last_cursor, cell_size);
    }

    fn emit_erase_effect(
        &mut self,
        cursor: Option<InputCursorSnapshot>,
        previous_cursor: Option<InputCursorSnapshot>,
        cell_size: Vector2F,
    ) {
        let Some(anchor) = cursor.or(previous_cursor) else {
            return;
        };
        let origin = cursor.map_or_else(
            || {
                Vector2F::new(
                    anchor.bounds.origin_x() - cell_size.x(),
                    anchor.bounds.origin_y(),
                )
            },
            |cursor| cursor.bounds.origin(),
        );
        let seed = ((origin.x().to_bits() as u64) << 32) ^ origin.y().to_bits() as u64;
        self.core
            .emit_input_erase_burst(origin, origin, seed, anchor.color);
    }
}

struct InputVisualFrame<'a> {
    config: TerminalVisualConfig,
    bounds: RectF,
    cell_size: Vector2F,
    composition_active: bool,
    text: InputTextSnapshot,
    glyphs: &'a [InputGlyphSnapshot],
    cursor: Option<InputCursorSnapshot>,
    cursor_visible: bool,
}

fn should_replace_native_cursor(frame: &TerminalVisualLayer) -> bool {
    frame
        .cursor
        .is_some_and(|cursor| cursor.is_active_at(frame.elapsed_seconds, frame.duration_seconds))
}

pub struct TerminalInputVisualElement {
    child: Box<dyn Element>,
    runtime: Rc<RefCell<TerminalInputVisualRuntime>>,
    cell_size: Vector2F,
    composition_active: bool,
    input_text: String,
    previous_input_text: String,
    cursor_byte_offset: usize,
    last_input_action: Option<PlainTextEditorViewAction>,
    cursor_visible: bool,
    size: Option<Vector2F>,
    origin: Option<Point>,
}

pub(crate) struct TerminalInputVisualOptions {
    pub(crate) cell_size: Vector2F,
    pub(crate) composition_active: bool,
    pub(crate) input_text: String,
    pub(crate) previous_input_text: String,
    pub(crate) cursor_byte_offset: usize,
    pub(crate) last_input_action: Option<PlainTextEditorViewAction>,
    pub(crate) cursor_visible: bool,
}

impl TerminalInputVisualElement {
    pub fn new(
        child: Box<dyn Element>,
        runtime: Rc<RefCell<TerminalInputVisualRuntime>>,
        options: TerminalInputVisualOptions,
    ) -> Self {
        Self {
            child,
            runtime,
            cell_size: options.cell_size,
            composition_active: options.composition_active,
            input_text: options.input_text,
            previous_input_text: options.previous_input_text,
            cursor_byte_offset: options.cursor_byte_offset,
            last_input_action: options.last_input_action,
            cursor_visible: options.cursor_visible,
            size: None,
            origin: None,
        }
    }

    pub fn finish(self) -> Box<dyn Element> {
        Box::new(self)
    }
}

impl Element for TerminalInputVisualElement {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        let size = self.child.layout(constraint, ctx, app);
        self.size = Some(size);
        size
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        let size = self
            .size
            .expect("input visual element laid out before paint");
        let bounds = RectF::new(origin, size);
        self.origin = Some(Point::from_vec2f(origin, ctx.scene.z_index()));
        ctx.scene
            .start_layer(ClipBounds::BoundedByActiveLayerAnd(bounds));

        let theme = Appearance::as_ref(app).theme();
        let config = TerminalVisualConfig::from_app(
            app,
            theme.background().into(),
            theme.accent().into(),
            theme.foreground().into(),
        );

        let glyph_cursor = ctx.scene.glyph_cursor();
        self.child.paint(origin, ctx, app);
        let mut glyphs = Vec::new();
        ctx.scene.visit_glyphs_since(&glyph_cursor, |glyph| {
            let mut hasher = DefaultHasher::new();
            glyph.glyph_key.hash(&mut hasher);
            glyphs.push(InputGlyphSnapshot {
                hash: hasher.finish(),
                position: glyph.position,
                scene_index: glyphs.len(),
            });
        });
        let mut cursor_rects = Vec::new();
        ctx.scene.visit_rects_since(&glyph_cursor, |rect| {
            if rect.terminal_cursor {
                let background = rect.background.start_color();
                let color = if background == ColorU::transparent_black() {
                    rect.border.color.start_color()
                } else {
                    background
                };
                cursor_rects.push(InputCursorSnapshot {
                    bounds: rect.bounds,
                    color,
                });
            }
        });
        let cursor = cursor_rects.last().copied();

        let (frame, animated_glyph) = self.runtime.borrow_mut().render_frame(InputVisualFrame {
            config,
            bounds,
            cell_size: self.cell_size,
            composition_active: self.composition_active,
            text: input_text_snapshot_with_previous(
                &self.input_text,
                self.cursor_byte_offset,
                self.last_input_action,
                &self.previous_input_text,
            ),
            glyphs: &glyphs,
            cursor,
            cursor_visible: self.cursor_visible,
        });
        if should_replace_native_cursor(&frame)
            && let Some(animated_cursor_index) = cursor_rects.len().checked_sub(1)
        {
            let mut cursor_index = 0;
            ctx.scene.visit_rects_since(&glyph_cursor, |rect| {
                if rect.terminal_cursor {
                    if cursor_index == animated_cursor_index {
                        rect.background = Fill::None;
                        rect.border = Border::default();
                    }
                    cursor_index += 1;
                }
            });
        }
        if let Some((animated_scene_index, cell_ref)) = animated_glyph {
            let mut scene_index = 0;
            ctx.scene.visit_glyphs_since(&glyph_cursor, |glyph| {
                glyph.terminal_cell = None;
                if scene_index == animated_scene_index {
                    glyph.with_terminal_cell(cell_ref);
                }
                scene_index += 1;
            });
        }
        let active = frame.is_active();
        let glyph_frame = Arc::new(TerminalVisualLayer {
            particles: Arc::default(),
            cursor: None,
            blocks: Arc::default(),
            ..(*frame).clone()
        });
        ctx.scene
            .set_terminal_visuals_on_glyph_layers_since(&glyph_cursor, glyph_frame);
        ctx.scene
            .start_layer(ClipBounds::BoundedByActiveLayerAnd(bounds));
        ctx.scene.set_active_layer_click_through();
        ctx.scene.set_terminal_visuals(frame);
        ctx.scene.stop_layer();
        if active {
            ctx.repaint_after(std::time::Duration::from_micros(8_333));
        }
        ctx.scene.stop_layer();
    }

    fn size(&self) -> Option<Vector2F> {
        self.size
    }

    fn origin(&self) -> Option<Point> {
        self.origin
    }

    fn dispatch_event(
        &mut self,
        event: &warpui::event::DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }
}

#[cfg(test)]
#[path = "input_visual_element_tests.rs"]
mod tests;
