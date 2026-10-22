use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use instant::Instant;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpui::terminal_visuals::{
    TERMINAL_EFFECT_OFF, TerminalBlockMotion, TerminalCellMotion, TerminalCellRef,
    TerminalCursorMotion, TerminalParticle, TerminalVisualLayer,
};

mod block_tracking;
mod config;
mod interaction;
mod state;

pub use config::TerminalVisualConfig;
use state::{
    ActiveCell, CellSlot, CellState, RowState, RuntimeLane, SemanticPulse, hash_to_unit,
    interpolated_cell_origin,
};

const MAX_PARTICLES: usize = 192;
const MAX_PARTICLE_DURATION_SCALE: f32 = 2.0;
pub(super) const INPUT_ERASE_DEBRIS_COUNT: usize = 25;
pub(super) const INPUT_SHATTER_PARTICLE_COUNT: usize = 37;
const CLOCK_REBASE_AFTER: Duration = Duration::from_secs(60 * 30);

pub struct TerminalVisualRuntime {
    epoch: Instant,
    now_seconds: f32,
    config: TerminalVisualConfig,
    lane: RuntimeLane,
    alternate_screen: bool,
    command_running: bool,
    viewport: RectF,
    cell_size: Vector2F,
    previous_cells: HashMap<CellSlot, CellState>,
    current_cells: HashMap<CellSlot, CellState>,
    previous_rows: HashMap<i32, RowState>,
    current_rows: HashMap<i32, RowState>,
    active_cells: HashMap<CellSlot, ActiveCell>,
    active_rows: Vec<(warpui::terminal_visuals::TerminalRowMotion, f32)>,
    frame_cells: Vec<TerminalCellMotion>,
    previous_blocks: HashMap<u64, RectF>,
    current_blocks: Vec<(u64, RectF)>,
    active_blocks: HashMap<u64, (TerminalBlockMotion, f32)>,
    observed_slots: HashSet<CellSlot>,
    has_cell_snapshot: bool,
    particles: VecDeque<TerminalParticle>,
    pulse: Option<(SemanticPulse, f32)>,
    cursor: Option<TerminalCursorMotion>,
    pending_cursor_duration_scale: Option<f32>,
    active_until_seconds: f32,
}

impl TerminalVisualRuntime {
    pub fn begin_frame(
        &mut self,
        config: TerminalVisualConfig,
        viewport: RectF,
        cell_size: Vector2F,
        alternate_screen: bool,
    ) {
        self.update_clock();
        let duration_changed =
            (self.config.duration_seconds - config.duration_seconds).abs() > f32::EPSILON;
        self.config = config;
        if duration_changed {
            self.rescale_activity_deadlines();
        }
        self.viewport = viewport;
        self.cell_size = cell_size;
        self.alternate_screen = alternate_screen;
        self.current_cells.clear();
        self.current_rows.clear();
        self.frame_cells.clear();
        self.current_blocks.clear();
        self.observed_slots.clear();
        self.active_cells
            .retain(|_, active| active.end_seconds > self.now_seconds);
        self.active_rows
            .retain(|(_, end_seconds)| *end_seconds > self.now_seconds);
        self.active_blocks
            .retain(|_, (_, end_seconds)| *end_seconds > self.now_seconds);
        self.particles.retain(|particle| {
            particle.start_seconds + self.config.duration_seconds * MAX_PARTICLE_DURATION_SCALE
                > self.now_seconds
        });
        if !self.config.enabled {
            self.clear_motion();
        }
    }

    pub fn observe_cell(
        &mut self,
        origin: Vector2F,
        content_hash: u64,
        is_empty: bool,
    ) -> Option<TerminalCellRef> {
        let slot = CellSlot::from_origin(origin, self.cell_size);
        self.observed_slots.insert(slot);
        self.observe_row(slot, origin, content_hash);
        if is_empty {
            return None;
        }

        self.current_cells.insert(
            slot,
            CellState {
                content_hash,
                origin,
            },
        );
        if self.lane == RuntimeLane::StaticOutput
            || !self.config.enabled
            || self.config.reduce_motion
        {
            return None;
        }

        let unchanged = self
            .previous_cells
            .get(&slot)
            .is_some_and(|previous| previous.content_hash == content_hash);
        if unchanged {
            return self.push_active_cell(slot, content_hash);
        }

        let previous_origin = self
            .active_cells
            .values()
            .find(|active| active.content_hash == content_hash)
            .map(|active| {
                interpolated_cell_origin(
                    active.motion,
                    self.now_seconds,
                    self.config.duration_seconds,
                )
            })
            .or_else(|| {
                self.previous_cells
                    .values()
                    .find(|state| state.content_hash == content_hash)
                    .map(|state| state.origin)
            })
            .unwrap_or(origin);
        if !self.has_cell_snapshot
            || self
                .previous_cells
                .values()
                .any(|state| state.content_hash == content_hash)
        {
            return None;
        }
        let effect_kind = self.cell_effect(slot, previous_origin);
        if effect_kind == TERMINAL_EFFECT_OFF {
            return None;
        }

        let motion = TerminalCellMotion {
            current_origin: origin,
            previous_origin,
            start_seconds: self.now_seconds,
            effect_kind,
            seed: hash_to_unit(content_hash),
            flags: 0,
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

    pub fn observe_block(&mut self, id: u64, bounds: RectF) {
        if self.alternate_screen || bounds.is_empty() {
            return;
        }
        if let Some(bounds) = bounds.intersection(self.viewport) {
            self.current_blocks.push((id, bounds));
        }
    }

    pub fn command_started(&mut self) {
        if self.lane == RuntimeLane::StaticOutput {
            return;
        }
        self.command_running = true;
        self.set_pulse(SemanticPulse::Commit);
    }

    pub fn command_completed(&mut self) {
        if self.lane == RuntimeLane::StaticOutput {
            return;
        }
        self.command_running = false;
        self.set_pulse(SemanticPulse::NewLine);
    }

    pub fn block_fold_changed(&mut self, bounds: RectF, expanding: bool) {
        if self.lane == RuntimeLane::StaticOutput {
            return;
        }
        self.set_pulse(SemanticPulse::BlockFold { bounds, expanding });
    }

    pub fn terminal_cleared(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
        self.previous_rows.clear();
        self.current_rows.clear();
        self.has_cell_snapshot = false;
        self.previous_blocks.clear();
        self.current_blocks.clear();
        self.clear_motion();
    }

    pub fn finish_frame(&mut self) -> Arc<TerminalVisualLayer> {
        self.emit_erased_cells();
        self.update_row_motions();
        self.update_block_motions();
        let blocks = self.frame_blocks();
        self.previous_cells.clone_from(&self.current_cells);
        self.previous_rows.clone_from(&self.current_rows);
        self.has_cell_snapshot = true;
        self.commit_block_snapshot();
        Arc::new(TerminalVisualLayer {
            viewport: self.viewport,
            cell_size: self.cell_size,
            elapsed_seconds: self.now_seconds,
            duration_seconds: self.config.duration_seconds,
            particle_spread: self.config.particle_spread,
            alternate_screen: self.alternate_screen,
            reduce_motion: self.config.reduce_motion,
            profile: self.config.profile,
            material: self.config.material,
            cells: self.frame_cells.clone().into(),
            rows: self
                .active_rows
                .iter()
                .map(|(motion, _)| *motion)
                .collect::<Vec<_>>()
                .into(),
            blocks: blocks.into(),
            particles: self.particles.iter().copied().collect::<Vec<_>>().into(),
            cursor: self.cursor,
            active_until_seconds: self.active_until_seconds,
        })
    }

    fn push_active_cell(&mut self, slot: CellSlot, content_hash: u64) -> Option<TerminalCellRef> {
        let active = self.active_cells.get(&slot)?;
        if active.content_hash != content_hash {
            return None;
        }
        let cell_ref = TerminalCellRef(self.frame_cells.len() as u32);
        self.frame_cells.push(active.motion);
        Some(cell_ref)
    }

    fn cell_effect(&self, slot: CellSlot, previous_origin: Vector2F) -> u32 {
        if self.alternate_screen {
            return TERMINAL_EFFECT_OFF;
        }
        if let Some((pulse, started)) = self.pulse
            && self.now_seconds - started <= self.config.duration_seconds
        {
            return match pulse {
                SemanticPulse::Commit => self.config.profile.commit,
                SemanticPulse::NewLine => self.config.profile.new_line,
                SemanticPulse::BlockFold { .. } => self.config.profile.block_fold,
            };
        }
        if self.command_running {
            return self.config.profile.new_line;
        }
        let _ = (slot, previous_origin);
        TERMINAL_EFFECT_OFF
    }

    fn emit_erased_cells(&mut self) {
        if self.lane == RuntimeLane::StaticOutput
            || !self.config.enabled
            || self.config.reduce_motion
            || self.config.profile.erase == TERMINAL_EFFECT_OFF
        {
            return;
        }
        for (slot, previous) in &self.previous_cells {
            if self.current_cells.contains_key(slot) || !self.observed_slots.contains(slot) {
                continue;
            }
            if self.particles.len() == MAX_PARTICLES {
                self.particles.pop_front();
            }
            let seed = hash_to_unit(previous.content_hash);
            self.particles.push_back(TerminalParticle {
                origin: previous.origin,
                velocity: vec2f((seed - 0.5) * 26., -10. - seed * 18.),
                size: vec2f(1.5, 1.5),
                start_seconds: self.now_seconds,
                seed,
                effect_kind: self.config.profile.erase,
                color: self.config.material.accent,
            });
            self.active_until_seconds = self
                .active_until_seconds
                .max(self.now_seconds + self.config.duration_seconds * MAX_PARTICLE_DURATION_SCALE);
        }
    }

    fn set_pulse(&mut self, pulse: SemanticPulse) {
        self.update_clock();
        let effect_kind = match pulse {
            SemanticPulse::Commit => self.config.profile.commit,
            SemanticPulse::NewLine => self.config.profile.new_line,
            SemanticPulse::BlockFold { .. } => self.config.profile.block_fold,
        };
        if effect_kind == TERMINAL_EFFECT_OFF {
            self.pulse = None;
            return;
        }
        self.pulse = Some((pulse, self.now_seconds));
        self.active_until_seconds = self
            .active_until_seconds
            .max(self.now_seconds + self.config.duration_seconds);
    }

    fn rescale_activity_deadlines(&mut self) {
        let duration = self.config.duration_seconds;
        for active in self.active_cells.values_mut() {
            active.end_seconds = active.motion.start_seconds + duration;
        }
        for (motion, end_seconds) in &mut self.active_rows {
            *end_seconds = motion.start_seconds + duration;
        }
        for (motion, end_seconds) in self.active_blocks.values_mut() {
            *end_seconds = motion.start_seconds + duration;
        }
        self.active_until_seconds =
            self.active_cells
                .values()
                .map(|active| active.end_seconds)
                .chain(self.active_rows.iter().map(|(_, end)| *end))
                .chain(self.active_blocks.values().map(|(_, end)| *end))
                .chain(self.particles.iter().map(|particle| {
                    particle.start_seconds + duration * MAX_PARTICLE_DURATION_SCALE
                }))
                .chain(
                    self.cursor
                        .iter()
                        .map(|cursor| cursor.start_seconds + duration * cursor.duration_scale),
                )
                .chain(self.pulse.iter().map(|(_, started)| *started + duration))
                .fold(self.now_seconds, f32::max);
    }

    fn observe_row(&mut self, slot: CellSlot, origin: Vector2F, content_hash: u64) {
        let cell_hash = content_hash
            ^ (slot.x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ 0x517c_c1b7_2722_0a95;
        self.current_rows
            .entry(slot.y)
            .and_modify(|row| {
                row.content_hash = row.content_hash.rotate_left(7) ^ cell_hash;
                row.cell_count += 1;
                row.origin_y = row.origin_y.min(origin.y());
            })
            .or_insert(RowState {
                content_hash: cell_hash,
                cell_count: 1,
                origin_y: origin.y(),
            });
    }

    fn update_row_motions(&mut self) {
        if self.lane == RuntimeLane::StaticOutput
            || !self.config.enabled
            || self.config.reduce_motion
            || self.alternate_screen
        {
            return;
        }
        for (&row, current) in &self.current_rows {
            let previous = self.previous_rows.iter().find(|(_, candidate)| {
                candidate.content_hash == current.content_hash
                    && candidate.cell_count == current.cell_count
            });
            let Some((&previous_row, previous)) = previous else {
                continue;
            };
            if previous_row == row
                || self
                    .active_rows
                    .iter()
                    .any(|(motion, _)| motion.row == row && motion.previous_row == previous_row)
            {
                continue;
            }
            let motion = warpui::terminal_visuals::TerminalRowMotion {
                row,
                previous_row,
                start_seconds: self.now_seconds,
                effect_kind: self.config.profile.new_line,
            };
            self.active_rows
                .push((motion, self.now_seconds + self.config.duration_seconds));
            self.active_until_seconds = self
                .active_until_seconds
                .max(self.now_seconds + self.config.duration_seconds);
            let _ = previous.origin_y;
        }
    }

    fn update_clock(&mut self) {
        let elapsed = self.epoch.elapsed();
        if elapsed >= CLOCK_REBASE_AFTER {
            let shift = elapsed.as_secs_f32();
            self.rebase(shift);
            self.epoch = Instant::now();
            self.now_seconds = 0.;
        } else {
            self.now_seconds = elapsed.as_secs_f32();
        }
    }

    fn rebase(&mut self, shift: f32) {
        for active in self.active_cells.values_mut() {
            active.motion.start_seconds -= shift;
            active.end_seconds -= shift;
        }
        for particle in &mut self.particles {
            particle.start_seconds -= shift;
        }
        if let Some(cursor) = &mut self.cursor {
            cursor.start_seconds -= shift;
        }
        if let Some((_, started)) = &mut self.pulse {
            *started -= shift;
        }
        self.active_until_seconds -= shift;
    }

    fn clear_motion(&mut self) {
        self.active_cells.clear();
        self.active_rows.clear();
        self.active_blocks.clear();
        self.particles.clear();
        self.cursor = None;
        self.pending_cursor_duration_scale = None;
        self.pulse = None;
        self.command_running = false;
        self.active_until_seconds = self.now_seconds;
    }
}

#[cfg(test)]
mod tests;
