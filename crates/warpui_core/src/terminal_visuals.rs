use std::sync::Arc;

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;

pub const TERMINAL_EFFECT_OFF: u32 = 0;
pub const TERMINAL_EFFECT_NEW_LINE_BASE: u32 = 100;
pub const TERMINAL_EFFECT_COMMIT_BASE: u32 = 200;
pub const TERMINAL_EFFECT_KEYPRESS_BASE: u32 = 300;
pub const TERMINAL_EFFECT_CURSOR_BASE: u32 = 400;
pub const TERMINAL_EFFECT_ERASE_BASE: u32 = 500;
pub const TERMINAL_EFFECT_BLOCK_BASE: u32 = 600;
pub const TERMINAL_CELL_FLAG_DIRECT_INPUT: u32 = 1 << 0;
pub const TERMINAL_CELL_FLAG_ERASE_GHOST: u32 = 1 << 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalCellRef(pub u32);

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalMotionProfile {
    pub new_line: u32,
    pub commit: u32,
    pub keypress: u32,
    pub cursor: u32,
    pub erase: u32,
    pub block_fold: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalMaterialProfile {
    pub style: u32,
    pub substrate_enabled: bool,
    pub command_gutter: bool,
    pub block_depth: bool,
    pub tint: [f32; 4],
    pub accent: [f32; 4],
    pub foreground: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalCellMotion {
    pub current_origin: Vector2F,
    pub previous_origin: Vector2F,
    pub start_seconds: f32,
    pub effect_kind: u32,
    pub seed: f32,
    pub flags: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalRowMotion {
    pub row: i32,
    pub previous_row: i32,
    pub start_seconds: f32,
    pub effect_kind: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalBlockMotion {
    pub bounds: RectF,
    pub start_seconds: f32,
    pub effect_kind: u32,
    pub phase: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalCursorMotion {
    pub from: Vector2F,
    pub to: Vector2F,
    pub size: Vector2F,
    pub start_seconds: f32,
    pub duration_scale: f32,
    pub effect_kind: u32,
    pub color: [f32; 4],
}

impl TerminalCursorMotion {
    pub fn is_active_at(&self, elapsed_seconds: f32, duration_seconds: f32) -> bool {
        elapsed_seconds < self.start_seconds + duration_seconds * self.duration_scale.max(0.01)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalParticle {
    pub origin: Vector2F,
    pub velocity: Vector2F,
    pub size: Vector2F,
    pub start_seconds: f32,
    pub seed: f32,
    pub effect_kind: u32,
    pub color: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub struct TerminalVisualLayer {
    pub viewport: RectF,
    pub cell_size: Vector2F,
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub particle_spread: f32,
    pub alternate_screen: bool,
    pub reduce_motion: bool,
    pub profile: TerminalMotionProfile,
    pub material: TerminalMaterialProfile,
    pub cells: Arc<[TerminalCellMotion]>,
    pub rows: Arc<[TerminalRowMotion]>,
    pub blocks: Arc<[TerminalBlockMotion]>,
    pub particles: Arc<[TerminalParticle]>,
    pub cursor: Option<TerminalCursorMotion>,
    pub active_until_seconds: f32,
}

impl TerminalVisualLayer {
    pub fn is_active(&self) -> bool {
        !self.reduce_motion && self.elapsed_seconds < self.active_until_seconds
    }

    pub fn cell(&self, cell_ref: TerminalCellRef) -> Option<&TerminalCellMotion> {
        self.cells.get(cell_ref.0 as usize)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pathfinder_color::ColorU;
    use pathfinder_geometry::rect::RectF;
    use pathfinder_geometry::vector::vec2f;

    use super::{TerminalParticle, TerminalVisualLayer};
    use crate::fonts::FontId;
    use crate::rendering;
    use crate::scene::{ClipBounds, Layer, Scene};

    #[test]
    fn generic_scene_layers_keep_the_terminal_renderer_dormant() {
        assert!(Layer::default().terminal_visuals.is_none());
    }

    #[test]
    fn scene_cursor_visits_only_new_terminal_cursor_rects() {
        let mut scene = Scene::new(1., rendering::Config::default());
        scene.draw_rect_with_hit_recording(RectF::new(vec2f(0., 0.), vec2f(10., 10.)));
        let cursor = scene.glyph_cursor();
        scene
            .draw_rect_with_hit_recording(RectF::new(vec2f(10., 0.), vec2f(2., 16.)))
            .with_terminal_cursor();

        let mut visited = 0;
        scene.visit_rects_since(&cursor, |rect| {
            if rect.terminal_cursor {
                visited += 1;
            }
        });

        assert_eq!(visited, 1);
    }

    #[test]
    fn terminal_visuals_attach_to_all_new_glyph_layers() {
        let mut scene = Scene::new(1., rendering::Config::default());
        let cursor = scene.glyph_cursor();
        for x in [0., 10.] {
            scene.start_layer(ClipBounds::None);
            scene.draw_glyph(vec2f(x, 20.), 0, FontId(0), 13., ColorU::white());
            scene.stop_layer();
        }
        let glyph_frame = Arc::new(TerminalVisualLayer {
            particles: vec![TerminalParticle::default()].into(),
            ..Default::default()
        });

        assert!(scene.set_terminal_visuals_on_glyph_layers_since(&cursor, glyph_frame));

        let particle_counts = scene
            .layers()
            .filter_map(|layer| layer.terminal_visuals.as_deref())
            .map(|visuals| visuals.particles.len())
            .collect::<Vec<_>>();
        assert_eq!(particle_counts, vec![1, 1]);
    }

    #[test]
    fn terminal_overlays_are_not_clipped_to_the_post_edit_glyph_layer() {
        let mut scene = Scene::new(1., rendering::Config::default());
        let input_bounds = RectF::new(vec2f(0., 0.), vec2f(100., 30.));
        scene.start_layer(ClipBounds::BoundedBy(input_bounds));
        let cursor = scene.glyph_cursor();
        scene.start_layer(ClipBounds::BoundedByActiveLayerAnd(RectF::new(
            vec2f(0., 0.),
            vec2f(10., 30.),
        )));
        scene.draw_glyph(vec2f(0., 20.), 0, FontId(0), 13., ColorU::white());
        scene.stop_layer();

        let glyph_frame = Arc::new(TerminalVisualLayer::default());
        let overlay_frame = Arc::new(TerminalVisualLayer {
            particles: vec![TerminalParticle {
                origin: vec2f(20., 20.),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        });
        assert!(scene.set_terminal_visuals_on_glyph_layers_since(&cursor, glyph_frame));
        scene.start_layer(ClipBounds::BoundedByActiveLayerAnd(input_bounds));
        scene.set_terminal_visuals(overlay_frame);
        scene.stop_layer();

        let overlay_clip = scene
            .layers()
            .find(|layer| {
                layer
                    .terminal_visuals
                    .as_deref()
                    .is_some_and(|visuals| !visuals.particles.is_empty())
            })
            .and_then(|layer| layer.clip_bounds);
        assert_eq!(overlay_clip, Some(input_bounds));
    }
}
