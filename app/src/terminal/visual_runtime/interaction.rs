use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpui::terminal_visuals::{
    TERMINAL_CELL_FLAG_DIRECT_INPUT, TERMINAL_CELL_FLAG_ERASE_GHOST, TERMINAL_EFFECT_OFF,
    TerminalCellMotion, TerminalCellRef, TerminalCursorMotion, TerminalParticle,
};

use super::config::color_array;
use super::state::{ActiveCell, CellSlot, CellState, hash_to_unit, interpolated_cursor};
use super::{MAX_PARTICLE_DURATION_SCALE, MAX_PARTICLES, TerminalVisualRuntime};

const BASE_PARTICLE_SPREAD: f32 = 0.8;

fn scaled_particle_count(base_count: usize, spread: f32) -> u64 {
    ((base_count as f32 * spread * spread).round() as usize).clamp(1, MAX_PARTICLES) as u64
}

fn particle_grid_dimensions(count: u64, aspect_ratio: f32) -> (u64, u64) {
    let columns = ((count as f32 * aspect_ratio).sqrt().ceil() as u64).max(1);
    (columns, count.div_ceil(columns))
}

impl TerminalVisualRuntime {
    pub fn observe_static_cell(&mut self, origin: Vector2F, content_hash: u64) {
        let slot = CellSlot::from_origin(origin, self.cell_size);
        self.observed_slots.insert(slot);
        self.observe_row(slot, origin, content_hash);
        self.current_cells.insert(
            slot,
            CellState {
                content_hash,
                origin,
            },
        );
    }

    pub fn start_input_cell_motion(
        &mut self,
        origin: Vector2F,
        content_hash: u64,
    ) -> Option<TerminalCellRef> {
        let slot = CellSlot::from_origin(origin, self.cell_size);
        self.observe_static_cell(origin, content_hash);
        if !self.config.enabled
            || self.config.reduce_motion
            || self.config.profile.keypress == TERMINAL_EFFECT_OFF
        {
            return None;
        }

        let motion = TerminalCellMotion {
            current_origin: origin,
            previous_origin: origin,
            start_seconds: self.now_seconds,
            effect_kind: self.config.profile.keypress,
            seed: hash_to_unit(content_hash),
            flags: TERMINAL_CELL_FLAG_DIRECT_INPUT,
        };
        self.active_until_seconds = self
            .active_until_seconds
            .max(self.now_seconds + self.config.duration_seconds);
        self.active_cells.insert(
            slot,
            ActiveCell {
                content_hash,
                motion,
                end_seconds: self.now_seconds + self.config.duration_seconds,
            },
        );
        self.push_active_cell(slot, content_hash)
    }

    pub fn emit_input_erase_burst(
        &mut self,
        cell_origin: Vector2F,
        cursor_origin: Vector2F,
        content_hash: u64,
        color: ColorU,
    ) {
        self.emit_input_erase_burst_with_color(
            cell_origin,
            cursor_origin,
            content_hash,
            color_array(color),
        );
    }

    fn emit_input_erase_burst_with_color(
        &mut self,
        cell_origin: Vector2F,
        cursor_origin: Vector2F,
        content_hash: u64,
        color: [f32; 4],
    ) {
        if !self.config.enabled
            || self.config.reduce_motion
            || self.config.profile.erase == TERMINAL_EFFECT_OFF
        {
            return;
        }

        if self.config.profile.erase % 100 == 18 {
            self.emit_space_ripple_burst(cell_origin, cursor_origin, color);
            return;
        }

        let debris_count =
            scaled_particle_count(super::INPUT_ERASE_DEBRIS_COUNT, self.config.particle_spread);
        let (debris_columns, debris_rows) = particle_grid_dimensions(debris_count, 1.0);
        let fragment_size = vec2f(
            (self.cell_size.x() / 5. * 1.32).clamp(1.5, 3.4),
            (self.cell_size.y() / 5. * 0.9).clamp(1.5, 3.5),
        );
        let spatial_scale = self.config.particle_spread * BASE_PARTICLE_SPREAD;
        let trail_start_x = cell_origin.x().max(cursor_origin.x());
        let trail_width = (self.cell_size.x() * 1.32 - fragment_size.x()).max(fragment_size.x());
        let glyph_top = cell_origin.y();
        for index in 0..debris_count {
            if self.particles.len() >= MAX_PARTICLES {
                self.particles.pop_front();
            }
            let mixed_hash = content_hash
                .wrapping_add(index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
                .rotate_left((index * 7) as u32);
            let seed = hash_to_unit(mixed_hash) * 0.78;
            let jitter_x = hash_to_unit(mixed_hash.rotate_left(17));
            let jitter_y = hash_to_unit(mixed_hash.rotate_left(31));
            let column = index % debris_columns;
            let row = index / debris_columns;
            let local_x = self.cell_size.x() * 0.7
                + (column as f32 + 0.08 + jitter_x * 0.2) * trail_width / debris_columns as f32
                + (jitter_x - 0.5) * self.cell_size.x() * 0.34;
            let local_y = (row as f32 + 0.2 + jitter_y * 0.24) * self.cell_size.y()
                / debris_rows as f32
                + (jitter_y - 0.5) * self.cell_size.y() * 0.22;
            let x = trail_start_x + local_x * spatial_scale;
            let y = glyph_top + local_y * spatial_scale;
            let delay =
                (jitter_x * 0.012 + jitter_y * 0.008).min(self.config.duration_seconds * 0.12);
            let mut particle_color = color;
            particle_color[3] *= 0.68 + jitter_y * 0.32;
            self.particles.push_back(TerminalParticle {
                origin: vec2f(x, y),
                velocity: vec2f(1.5 + jitter_x * 19.0, -5.0 - jitter_y * 19.0),
                size: fragment_size * (0.9 + jitter_x * 0.9),
                start_seconds: self.now_seconds + delay,
                seed,
                effect_kind: self.config.profile.erase,
                color: particle_color,
            });
        }

        if self.config.profile.erase % 100 == 12 {
            let ghost_count = scaled_particle_count(
                super::INPUT_SHATTER_PARTICLE_COUNT - super::INPUT_ERASE_DEBRIS_COUNT,
                self.config.particle_spread,
            );
            let (ghost_columns, ghost_rows) = particle_grid_dimensions(ghost_count, 4. / 3.);
            let mut ghost_color = color;
            ghost_color[3] *= 0.29;
            for index in 0..ghost_count {
                if self.particles.len() >= MAX_PARTICLES {
                    self.particles.pop_front();
                }
                let mixed_hash = content_hash
                    .wrapping_add(index.wrapping_mul(0x517c_c1b7_2722_0a95))
                    .rotate_left((index * 5) as u32);
                let unit = hash_to_unit(mixed_hash);
                let seed = 0.84 + unit * 0.15;
                let column = index % ghost_columns;
                let row = index / ghost_columns;
                let local_x = self.cell_size.x() * 0.6
                    + (column as f32 + 0.16 + unit * 0.18) * trail_width / ghost_columns as f32;
                let local_y =
                    (row as f32 + 0.34 + unit * 0.22) * self.cell_size.y() / ghost_rows as f32;
                let x = trail_start_x + local_x * spatial_scale;
                let y = glyph_top + local_y * spatial_scale;
                self.particles.push_back(TerminalParticle {
                    origin: vec2f(x, y),
                    velocity: vec2f(1.0 + unit * 3.0, -1.4 - unit * 3.2),
                    size: fragment_size * (1.08 + unit * 0.78),
                    start_seconds: self.now_seconds,
                    seed,
                    effect_kind: self.config.profile.erase,
                    color: ghost_color,
                });
            }
        }
        self.active_until_seconds = self.active_until_seconds.max(
            self.now_seconds + self.config.duration_seconds * (MAX_PARTICLE_DURATION_SCALE + 0.12),
        );
    }

    fn emit_space_ripple_burst(
        &mut self,
        cell_origin: Vector2F,
        cursor_origin: Vector2F,
        color: [f32; 4],
    ) {
        let origin = vec2f(
            cell_origin.x().max(cursor_origin.x()),
            cell_origin.y()
                + self.cell_size.y() * 0.18 * self.config.particle_spread * BASE_PARTICLE_SPREAD,
        );
        let particle_count = scaled_particle_count(3, self.config.particle_spread);
        for index in 0..particle_count {
            if self.particles.len() >= MAX_PARTICLES {
                self.particles.pop_front();
            }
            let phase = index as f32 / particle_count as f32;
            self.particles.push_back(TerminalParticle {
                origin,
                velocity: Vector2F::zero(),
                size: self.cell_size * (0.42 + phase * 0.16),
                start_seconds: self.now_seconds + phase * self.config.duration_seconds * 0.16,
                seed: 0.2 + phase * 0.6,
                effect_kind: self.config.profile.erase,
                color,
            });
        }
        self.active_until_seconds = self.active_until_seconds.max(
            self.now_seconds + self.config.duration_seconds * (MAX_PARTICLE_DURATION_SCALE + 0.16),
        );
    }

    pub fn input_backspace(&mut self) -> bool {
        if !self.config.enabled || self.config.reduce_motion {
            return false;
        }
        let Some(cursor) = self.cursor else {
            return false;
        };
        self.update_clock();
        let cursor_origin =
            interpolated_cursor(cursor, self.now_seconds, self.config.duration_seconds);
        let origin = Vector2F::new(
            (cursor_origin.x() - self.cell_size.x()).max(self.viewport.origin_x()),
            cursor_origin.y(),
        );
        let seed = ((origin.x().to_bits() as u64) << 32) ^ origin.y().to_bits() as u64;
        self.pending_cursor_duration_scale = Some(0.25);
        self.emit_input_erase_burst_with_color(origin, origin, seed, cursor.color);
        true
    }

    pub fn start_input_erase_ghost(&self) -> Option<f32> {
        (self.config.enabled
            && !self.config.reduce_motion
            && self.config.profile.erase != TERMINAL_EFFECT_OFF)
            .then_some(self.now_seconds)
    }

    pub fn continue_input_erase_ghost(
        &mut self,
        origin: Vector2F,
        content_hash: u64,
        start_seconds: f32,
    ) -> Option<TerminalCellRef> {
        let end_seconds = start_seconds + self.config.duration_seconds;
        if self.now_seconds >= end_seconds {
            return None;
        }
        let cell_ref = TerminalCellRef(self.frame_cells.len() as u32);
        self.frame_cells.push(TerminalCellMotion {
            current_origin: origin,
            previous_origin: origin,
            start_seconds,
            effect_kind: self.config.profile.erase,
            seed: hash_to_unit(content_hash),
            flags: TERMINAL_CELL_FLAG_ERASE_GHOST,
        });
        self.active_until_seconds = self.active_until_seconds.max(end_seconds);
        Some(cell_ref)
    }

    pub fn clear_cell_motion(&mut self) {
        self.active_cells.clear();
        self.frame_cells.clear();
    }

    pub fn observe_cursor(&mut self, bounds: RectF, color: ColorU) -> bool {
        let duration_scale = self.pending_cursor_duration_scale.take().unwrap_or(1.);
        self.observe_cursor_with_duration_scale(bounds, color, duration_scale)
    }

    pub fn observe_cursor_with_duration_scale(
        &mut self,
        bounds: RectF,
        color: ColorU,
        duration_scale: f32,
    ) -> bool {
        let duration_scale = duration_scale.clamp(0.05, 1.);
        if !self.config.enabled || self.config.reduce_motion {
            self.cursor = None;
            return false;
        }
        let origin = bounds.origin();
        let from = self
            .cursor
            .map(|cursor| {
                interpolated_cursor(cursor, self.now_seconds, self.config.duration_seconds)
            })
            .unwrap_or(origin);
        if from == origin
            && self.cursor.is_some_and(|cursor| {
                cursor.size == bounds.size() && cursor.color == color_array(color)
            })
        {
            return self.cursor_motion_is_active();
        }
        self.cursor = Some(TerminalCursorMotion {
            from,
            to: origin,
            size: bounds.size(),
            start_seconds: self.now_seconds,
            duration_scale,
            effect_kind: self.config.profile.cursor,
            color: color_array(color),
        });
        self.active_until_seconds = self
            .active_until_seconds
            .max(self.now_seconds + self.config.duration_seconds * duration_scale);
        self.cursor_motion_is_active()
    }

    fn cursor_motion_is_active(&self) -> bool {
        self.cursor.is_some_and(|cursor| {
            cursor.is_active_at(self.now_seconds, self.config.duration_seconds)
        })
    }

    pub fn clear_cursor_motion(&mut self) {
        self.cursor = None;
    }
}
