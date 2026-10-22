use super::*;
use crate::platform::ios::terminal_effects::EffectSample;
use crate::platform::ios::{
    TerminalCell, TerminalCursorStyle, TerminalDeletionEffect, TerminalEffectKind,
    TerminalFontWeight, TerminalInputEffect,
};

impl Renderer {
    pub(super) fn draw_terminal_frame(&self, scene: &mut Scene, now: Instant) {
        for cell in &self.terminal_frame.cells {
            let background = color(cell.background);
            if background != self.terminal_background() {
                self.draw_cell_background(
                    scene,
                    cell.row,
                    cell.column,
                    if cell.wide { 2 } else { 1 },
                    background,
                );
            }
        }

        if self.cursor_visible
            && let Some((row, column)) = self.terminal_frame.cursor
        {
            self.draw_cursor(scene, row, column, now);
        }

        for cell in &self.terminal_frame.cells {
            if let Some(sample) = self.effects.input_at(cell.row, cell.column, now) {
                self.draw_effect_cell(scene, sample);
            } else {
                self.draw_cell_text(scene, cell, Vector2F::zero(), 1.0, self.settings.font_size);
            }
            if cell.underline {
                let origin = self.cell_text_origin(cell.row, cell.column);
                scene
                    .draw_rect_without_hit_recording(RectF::new(
                        Vector2F::new(origin.x(), origin.y() + 2.0),
                        Vector2F::new(
                            self.metrics.cell_width * if cell.wide { 2.0 } else { 1.0 },
                            1.0,
                        ),
                    ))
                    .with_background(color(cell.foreground));
            }
        }

        for sample in self.effects.deletion_samples(now) {
            self.draw_effect_cell(scene, sample);
        }
    }

    fn draw_cell_background(
        &self,
        scene: &mut Scene,
        row: u16,
        column: u16,
        width_cells: u16,
        color: ColorU,
    ) {
        scene
            .draw_rect_without_hit_recording(RectF::new(
                Vector2F::new(
                    HORIZONTAL_PADDING + f32::from(column) * self.metrics.cell_width,
                    VERTICAL_PADDING + f32::from(row) * self.metrics.line_height,
                ),
                Vector2F::new(
                    self.metrics.cell_width * f32::from(width_cells),
                    self.metrics.line_height,
                ),
            ))
            .with_background(color);
    }

    fn draw_cursor(&self, scene: &mut Scene, row: u16, column: u16, now: Instant) {
        let sample = self.cursor_sample((row, column), now);
        let (offset, base_size) = match self.settings.cursor_style {
            TerminalCursorStyle::Block => (
                Vector2F::zero(),
                Vector2F::new(self.metrics.cell_width, self.metrics.line_height),
            ),
            TerminalCursorStyle::Bar => (
                Vector2F::zero(),
                Vector2F::new(1.5, self.metrics.line_height),
            ),
            TerminalCursorStyle::Underline => (
                Vector2F::new(0.0, self.metrics.line_height - 1.5),
                Vector2F::new(self.metrics.cell_width, 1.5),
            ),
        };
        if let Some((trail, opacity)) = sample.trail {
            self.draw_cursor_rect(scene, trail, offset, base_size, opacity, sample.scale);
        }
        self.draw_cursor_rect(scene, sample.position, offset, base_size, 1.0, sample.scale);
    }

    fn draw_cursor_rect(
        &self,
        scene: &mut Scene,
        position: Vector2F,
        offset: Vector2F,
        base_size: Vector2F,
        opacity: f32,
        scale: Vector2F,
    ) {
        let size = Vector2F::new(base_size.x() * scale.x(), base_size.y() * scale.y());
        let origin = Vector2F::new(
            HORIZONTAL_PADDING + position.x() * self.metrics.cell_width,
            VERTICAL_PADDING + position.y() * self.metrics.line_height,
        ) + offset
            + (base_size - size) * 0.5;
        let mut cursor = self.settings.cursor;
        if matches!(self.settings.cursor_style, TerminalCursorStyle::Block) {
            cursor[3] = cursor[3].min(170);
        }
        cursor[3] = (f32::from(cursor[3]) * opacity.clamp(0.0, 1.0)) as u8;
        scene
            .draw_rect_without_hit_recording(RectF::new(origin, size))
            .with_background(color(cursor));
    }

    fn draw_effect_cell(&self, scene: &mut Scene, sample: EffectSample<'_>) {
        let progress = ease_out(sample.progress.clamp(0.0, 1.0));
        let cell = &sample.effect.cell;
        match sample.effect.kind {
            TerminalEffectKind::Input(effect) => {
                let (offset, alpha, size) = match effect {
                    TerminalInputEffect::Off => return,
                    TerminalInputEffect::Fade => {
                        (Vector2F::zero(), progress, self.settings.font_size)
                    }
                    TerminalInputEffect::Rise => (
                        Vector2F::new(0.0, (1.0 - progress) * 4.0),
                        progress,
                        self.settings.font_size,
                    ),
                    TerminalInputEffect::Pop => (
                        Vector2F::zero(),
                        progress,
                        self.settings.font_size * (0.82 + progress * 0.18),
                    ),
                };
                self.draw_cell_text(scene, cell, offset, alpha, size);
            }
            TerminalEffectKind::Deletion(TerminalDeletionEffect::Shatter) => {
                self.draw_shatter(scene, cell, sample.progress);
            }
            TerminalEffectKind::Deletion(TerminalDeletionEffect::Dissolve) => {
                self.draw_dissolve(scene, cell, progress);
            }
            TerminalEffectKind::Deletion(effect) => {
                let offset = if matches!(effect, TerminalDeletionEffect::Recede) {
                    Vector2F::new(progress * 3.0, progress * 2.0)
                } else {
                    Vector2F::zero()
                };
                self.draw_cell_text(
                    scene,
                    cell,
                    offset,
                    1.0 - progress,
                    self.settings.font_size * (1.0 - progress * 0.1),
                );
            }
        }
    }

    fn draw_cell_text(
        &self,
        scene: &mut Scene,
        cell: &TerminalCell,
        offset: Vector2F,
        alpha: f32,
        font_size: f32,
    ) {
        let origin = self.cell_text_origin(cell.row, cell.column) + offset;
        for character in cell.text.chars() {
            if let Some((font_id, glyph_id)) = self.glyph_for_char(character) {
                for &(x, y, opacity) in font_weight_passes(self.settings.font_weight) {
                    scene.draw_glyph(
                        origin + Vector2F::new(x, y),
                        glyph_id,
                        font_id,
                        font_size,
                        color_with_alpha(cell.foreground, alpha * opacity),
                    );
                }
            }
        }
    }

    fn draw_shatter(&self, scene: &mut Scene, cell: &TerminalCell, elapsed: f32) {
        let glyph_progress = ease_out(elapsed.clamp(0.0, 1.0));
        self.draw_cell_text(
            scene,
            cell,
            Vector2F::new(glyph_progress * 0.8, -glyph_progress * 0.5),
            (0.29 * (1.0 - glyph_progress * 1.35)).max(0.0),
            self.settings.font_size,
        );

        let origin = Vector2F::new(
            HORIZONTAL_PADDING + f32::from(cell.column) * self.metrics.cell_width,
            VERTICAL_PADDING + f32::from(cell.row) * self.metrics.line_height,
        );
        let seed = shatter_seed(cell);
        let fragment_size = Vector2F::new(
            (self.metrics.cell_width / 5.0 * 1.32).clamp(1.5, 3.4),
            (self.metrics.line_height / 5.0 * 0.9).clamp(1.5, 3.5),
        );
        let trail_width =
            (self.metrics.cell_width * 1.32 - fragment_size.x()).max(fragment_size.x());
        let duration_seconds = (self.settings.effect_duration_ms / 1_000.0).max(0.001);

        for index in 0..25_u64 {
            let jitter_x = particle_unit(seed, index, 17);
            let jitter_y = particle_unit(seed, index, 31);
            let particle_seed = particle_unit(seed, index, 7) * 0.78;
            let life_seed = particle_unit(seed ^ 0xa24b_aed4_963e_e407, index, 13);
            let delay = (jitter_x * 0.012 + jitter_y * 0.008).min(duration_seconds * 0.12)
                / duration_seconds;
            let progress = ((elapsed - delay) / mix(1.35, 2.0, life_seed)).clamp(0.0, 1.0);
            let motion = 1.0 - (-3.8 * progress).exp();
            let column = index % 5;
            let row = index / 5;
            let local_x = self.metrics.cell_width * 0.7
                + (column as f32 + 0.08 + jitter_x * 0.2) * trail_width / 5.0
                + (jitter_x - 0.5) * self.metrics.cell_width * 0.34;
            let local_y = (row as f32 + 0.2 + jitter_y * 0.24) * self.metrics.line_height / 5.0
                + (jitter_y - 0.5) * self.metrics.line_height * 0.22;
            let velocity = Vector2F::new(1.5 + jitter_x * 19.0, -5.0 - jitter_y * 19.0);
            let envelope = (progress * std::f32::consts::PI).sin();
            let turbulence = (progress * (7.0 + life_seed * 11.0) + particle_seed * 31.0).sin();
            let adjustment = Vector2F::new(
                turbulence * (0.7 + life_seed * 1.9),
                (life_seed - 0.5) * 2.2,
            ) * envelope;
            let drift = (velocity * motion + adjustment) * 0.8;
            let fade_seed = particle_unit(seed ^ 0x9e37_79b9_7f4a_7c15, index, 23);
            let reveal = smoothstep(0.0, mix(0.025, 0.085, fade_seed), progress);
            let fade = 1.0
                - smoothstep(
                    mix(0.16, 0.34, fade_seed),
                    mix(0.84, 1.0, life_seed),
                    progress,
                );
            let opacity_motion =
                0.92 + (progress * (5.0 + life_seed * 4.0) + particle_seed * 17.0).sin() * 0.08;
            let alpha = (0.68 + jitter_y * 0.32)
                * reveal
                * fade
                * mix(0.58, 1.0, fade_seed)
                * opacity_motion;
            let size = fragment_size * (0.9 + jitter_x * 0.9) * (1.0 - progress * 0.38);
            scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + Vector2F::new(local_x, local_y) + drift,
                    size,
                ))
                .with_background(color_with_alpha(cell.foreground, alpha));
        }

        for index in 0..12_u64 {
            let unit = particle_unit(seed ^ 0x517c_c1b7_2722_0a95, index, 5);
            let particle_seed = 0.84 + unit * 0.15;
            let life_seed = particle_unit(seed ^ 0x94d0_49bb_1331_11eb, index, 19);
            let progress = (elapsed / mix(1.35, 2.0, life_seed)).clamp(0.0, 1.0);
            let motion = 1.0 - (-3.8 * progress).exp();
            let column = index % 4;
            let row = index / 4;
            let local_x = self.metrics.cell_width * 0.6
                + (column as f32 + 0.16 + unit * 0.18) * trail_width / 4.0;
            let local_y = (row as f32 + 0.34 + unit * 0.22) * self.metrics.line_height / 3.0;
            let velocity = Vector2F::new(1.0 + unit * 3.0, -1.4 - unit * 3.2) * 0.7;
            let envelope = (progress * std::f32::consts::PI).sin();
            let turbulence = (progress * (7.0 + life_seed * 11.0) + particle_seed * 31.0).sin();
            let adjustment = Vector2F::new(
                turbulence * (0.7 + life_seed * 1.9),
                (life_seed - 0.5) * 2.2,
            ) * envelope;
            let drift = (velocity * motion + adjustment) * 0.8;
            let fade_seed = particle_unit(seed ^ 0xd6e8_feb8_6659_fd93, index, 29);
            let reveal = smoothstep(0.0, mix(0.025, 0.085, fade_seed), progress);
            let fade = 1.0 - smoothstep(mix(0.42, 0.62, fade_seed), 1.0, progress);
            let opacity_motion =
                0.92 + (progress * (5.0 + life_seed * 4.0) + particle_seed * 17.0).sin() * 0.08;
            let alpha = 0.29 * reveal * fade * mix(0.58, 1.0, fade_seed) * opacity_motion;
            let size = fragment_size * (1.08 + unit * 0.78) * (0.96 - progress * 0.18);
            scene
                .draw_rect_without_hit_recording(RectF::new(
                    origin + Vector2F::new(local_x, local_y) + drift,
                    size,
                ))
                .with_background(color_with_alpha(cell.foreground, alpha));
        }
    }

    fn draw_dissolve(&self, scene: &mut Scene, cell: &TerminalCell, progress: f32) {
        self.draw_cell_text(
            scene,
            cell,
            Vector2F::zero(),
            (1.0 - progress * 1.8).max(0.0),
            self.settings.font_size,
        );
        let origin = self.cell_text_origin(cell.row, cell.column)
            - Vector2F::new(0.0, self.metrics.baseline * 0.72);
        let foreground = color_with_alpha(cell.foreground, 1.0 - progress);
        for (index, (x, y)) in [
            (0.1, 0.15),
            (0.38, 0.08),
            (0.72, 0.2),
            (0.9, 0.45),
            (0.2, 0.58),
            (0.55, 0.52),
            (0.78, 0.82),
            (0.32, 0.88),
        ]
        .into_iter()
        .enumerate()
        {
            let phase = (index as f32 * 0.73).sin();
            let drift = Vector2F::new(phase * progress * 4.5, progress * (4.0 + y * 5.0));
            let point = origin
                + Vector2F::new(x * self.metrics.cell_width, y * self.metrics.line_height)
                + drift;
            let size = 1.7 + f32::from((index % 2) as u8) * 0.5;
            scene
                .draw_rect_without_hit_recording(RectF::new(point, Vector2F::new(size, size)))
                .with_background(foreground);
        }
    }

    fn cell_text_origin(&self, row: u16, column: u16) -> Vector2F {
        Vector2F::new(
            HORIZONTAL_PADDING + f32::from(column) * self.metrics.cell_width,
            VERTICAL_PADDING + f32::from(row) * self.metrics.line_height + self.metrics.baseline,
        )
    }

    pub(super) fn terminal_background(&self) -> ColorU {
        color(self.settings.background)
    }
}

fn color_with_alpha([red, green, blue, alpha]: [u8; 4], opacity: f32) -> ColorU {
    ColorU::new(
        red,
        green,
        blue,
        (f32::from(alpha) * opacity.clamp(0.0, 1.0)) as u8,
    )
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

fn mix(start: f32, end: f32, amount: f32) -> f32 {
    start + (end - start) * amount
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let amount = ((value - start) / (end - start).max(f32::EPSILON)).clamp(0.0, 1.0);
    amount * amount * (3.0 - 2.0 * amount)
}

fn shatter_seed(cell: &TerminalCell) -> u64 {
    cell.text.bytes().fold(
        (u64::from(cell.row) << 32) | u64::from(cell.column),
        |hash, byte| hash.wrapping_mul(0x100_0000_01b3) ^ u64::from(byte),
    )
}

fn particle_unit(seed: u64, index: u64, rotation: u32) -> f32 {
    let mixed = seed
        .wrapping_add(index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .rotate_left(rotation);
    (mixed as u32) as f32 / u32::MAX as f32
}

fn font_weight_passes(weight: TerminalFontWeight) -> &'static [(f32, f32, f32)] {
    match weight {
        TerminalFontWeight::Thin => &[(0.0, 0.0, 0.62)],
        TerminalFontWeight::ExtraLight => &[(0.0, 0.0, 0.74)],
        TerminalFontWeight::Light => &[(0.0, 0.0, 0.86)],
        TerminalFontWeight::Normal => &[(0.0, 0.0, 1.0)],
        TerminalFontWeight::Medium => &[(0.0, 0.0, 0.9), (0.22, 0.0, 0.28)],
        TerminalFontWeight::Semibold => &[(0.0, 0.0, 0.88), (0.28, 0.0, 0.34), (0.0, 0.2, 0.12)],
        TerminalFontWeight::Bold => &[(0.0, 0.0, 0.9), (0.34, 0.0, 0.4), (0.0, 0.26, 0.18)],
        TerminalFontWeight::ExtraBold => &[
            (0.0, 0.0, 0.92),
            (0.42, 0.0, 0.44),
            (0.0, 0.32, 0.22),
            (0.32, 0.26, 0.14),
        ],
        TerminalFontWeight::Black => &[
            (0.0, 0.0, 0.94),
            (0.5, 0.0, 0.48),
            (0.0, 0.4, 0.28),
            (0.4, 0.34, 0.2),
        ],
    }
}
