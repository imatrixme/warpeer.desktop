use instant::Instant;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use warpui::terminal_visuals::{TerminalCellMotion, TerminalCursorMotion};

use super::{TerminalVisualConfig, TerminalVisualRuntime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RuntimeLane {
    Semantic,
    StaticOutput,
    Input,
}

impl TerminalVisualRuntime {
    pub fn new(config: TerminalVisualConfig) -> Self {
        Self::with_lane(config, RuntimeLane::Semantic)
    }

    pub fn new_static_output(config: TerminalVisualConfig) -> Self {
        Self::with_lane(config, RuntimeLane::StaticOutput)
    }

    pub fn new_input(config: TerminalVisualConfig) -> Self {
        Self::with_lane(config, RuntimeLane::Input)
    }

    fn with_lane(config: TerminalVisualConfig, lane: RuntimeLane) -> Self {
        Self {
            epoch: Instant::now(),
            now_seconds: 0.,
            config,
            lane,
            alternate_screen: false,
            command_running: false,
            viewport: RectF::default(),
            cell_size: Vector2F::zero(),
            previous_cells: Default::default(),
            current_cells: Default::default(),
            previous_rows: Default::default(),
            current_rows: Default::default(),
            active_cells: Default::default(),
            active_rows: Default::default(),
            frame_cells: Default::default(),
            previous_blocks: Default::default(),
            current_blocks: Default::default(),
            active_blocks: Default::default(),
            observed_slots: Default::default(),
            has_cell_snapshot: false,
            particles: Default::default(),
            pulse: None,
            cursor: None,
            pending_cursor_duration_scale: None,
            active_until_seconds: 0.,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct CellSlot {
    pub(super) x: i32,
    pub(super) y: i32,
}

impl CellSlot {
    pub(super) fn from_origin(origin: Vector2F, cell_size: Vector2F) -> Self {
        Self {
            x: (origin.x() / cell_size.x().max(1.)).round() as i32,
            y: (origin.y() / cell_size.y().max(1.)).round() as i32,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CellState {
    pub(super) content_hash: u64,
    pub(super) origin: Vector2F,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct RowState {
    pub(super) content_hash: u64,
    pub(super) cell_count: u32,
    pub(super) origin_y: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ActiveCell {
    pub(super) content_hash: u64,
    pub(super) motion: TerminalCellMotion,
    pub(super) end_seconds: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SemanticPulse {
    Commit,
    NewLine,
    BlockFold { bounds: RectF, expanding: bool },
}

pub(super) fn interpolated_cursor(
    cursor: TerminalCursorMotion,
    now: f32,
    duration: f32,
) -> Vector2F {
    let duration = duration * cursor.duration_scale.max(0.01);
    let progress = ((now - cursor.start_seconds) / duration.max(0.001)).clamp(0., 1.);
    let eased = 1. - (1. - progress).powi(3);
    cursor.from + (cursor.to - cursor.from) * eased
}

pub(super) fn interpolated_cell_origin(
    cell: TerminalCellMotion,
    now: f32,
    duration: f32,
) -> Vector2F {
    let progress = ((now - cell.start_seconds) / duration.max(0.001)).clamp(0., 1.);
    let eased = 1. - (1. - progress).powi(3);
    cell.previous_origin + (cell.current_origin - cell.previous_origin) * eased
}

pub(super) fn hash_to_unit(hash: u64) -> f32 {
    (hash as u32) as f32 / u32::MAX as f32
}
