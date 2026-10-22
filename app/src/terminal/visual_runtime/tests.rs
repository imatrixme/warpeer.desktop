use std::thread;
use std::time::Duration;

use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::{Vector2F, vec2f};
use warpui::terminal_visuals::{
    TERMINAL_CELL_FLAG_ERASE_GHOST, TerminalMaterialProfile, TerminalMotionProfile,
};

use super::config::{block_effect, commit_effect, erase_effect, keypress_effect, new_line_effect};
use super::*;
use crate::settings::{BlockFoldMotion, CommitMotion, EraseMotion, KeypressMotion, NewLineMotion};

fn config() -> TerminalVisualConfig {
    TerminalVisualConfig {
        enabled: true,
        duration_seconds: 0.08,
        particle_spread: 1.0,
        reduce_motion: false,
        profile: TerminalMotionProfile {
            new_line: 101,
            commit: 201,
            keypress: 301,
            cursor: 401,
            erase: 501,
            block_fold: 601,
        },
        material: TerminalMaterialProfile::default(),
    }
}

fn begin(runtime: &mut TerminalVisualRuntime, config: TerminalVisualConfig) {
    runtime.begin_frame(
        config,
        RectF::new(Vector2F::zero(), vec2f(800., 600.)),
        vec2f(10., 20.),
        false,
    );
}

#[test]
fn unchanged_cells_keep_the_same_active_track() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    let first = runtime.start_input_cell_motion(vec2f(10., 20.), 7).unwrap();
    runtime.finish_frame();
    begin(&mut runtime, config());
    let second = runtime.observe_cell(vec2f(10., 20.), 7, false).unwrap();
    assert_eq!(first, second);
}

#[test]
fn first_grid_snapshot_does_not_animate_restored_content() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());

    assert!(runtime.observe_cell(vec2f(10., 20.), 7, false).is_none());
    assert!(runtime.observe_cell(vec2f(20., 20.), 8, false).is_none());
    assert!(runtime.finish_frame().cells.is_empty());
}

#[test]
fn existing_glyph_movement_does_not_replay_a_birth_effect() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.finish_frame();

    begin(&mut runtime, config());
    assert!(runtime.observe_cell(vec2f(20., 20.), 7, false).is_none());
    assert!(runtime.finish_frame().cells.is_empty());
}

#[test]
fn cursor_retarget_starts_from_the_interpolated_position() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cursor(RectF::new(vec2f(10., 20.), vec2f(2., 16.)), ColorU::white());
    thread::sleep(Duration::from_millis(10));
    begin(&mut runtime, config());
    runtime.observe_cursor(RectF::new(vec2f(80., 20.), vec2f(2., 16.)), ColorU::white());
    let cursor = runtime.finish_frame().cursor.unwrap();
    assert!(cursor.from.x() >= 10. && cursor.from.x() < 80.);
    assert_eq!(cursor.size, vec2f(2., 16.));
    assert_eq!(cursor.duration_scale, 1.);
}

#[test]
fn cursor_motion_reports_native_cursor_ownership_only_while_active() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    assert!(runtime.observe_cursor(RectF::new(vec2f(10., 20.), vec2f(8., 16.)), ColorU::white(),));

    runtime.config.enabled = false;
    assert!(!runtime.observe_cursor(RectF::new(vec2f(20., 20.), vec2f(8., 16.)), ColorU::white(),));
    assert!(runtime.finish_frame().cursor.is_none());
}

#[test]
fn cursor_reappears_at_current_position_after_blink_clears_motion() {
    let mut runtime = TerminalVisualRuntime::new(config());
    let old_bounds = RectF::new(vec2f(10., 20.), vec2f(8., 16.));
    let current_bounds = RectF::new(vec2f(80., 20.), vec2f(8., 16.));

    begin(&mut runtime, config());
    runtime.observe_cursor(old_bounds, ColorU::white());
    runtime.finish_frame();

    begin(&mut runtime, config());
    runtime.clear_cursor_motion();
    assert!(runtime.finish_frame().cursor.is_none());

    begin(&mut runtime, config());
    runtime.observe_cursor(current_bounds, ColorU::white());
    let cursor = runtime.finish_frame().cursor.unwrap();

    assert_eq!(cursor.from, current_bounds.origin());
    assert_eq!(cursor.to, current_bounds.origin());
}

#[test]
fn delete_cursor_motion_uses_the_shortened_duration() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cursor(RectF::new(vec2f(20., 20.), vec2f(2., 16.)), ColorU::white());
    runtime.finish_frame();

    begin(&mut runtime, config());
    runtime.observe_cursor_with_duration_scale(
        RectF::new(vec2f(10., 20.), vec2f(2., 16.)),
        ColorU::white(),
        0.25,
    );

    assert_eq!(runtime.finish_frame().cursor.unwrap().duration_scale, 0.25);
}

#[test]
fn static_output_lane_keeps_grid_content_static_but_allows_cursor_motion() {
    let mut runtime = TerminalVisualRuntime::new_static_output(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.observe_block(7, RectF::new(vec2f(0., 10.), vec2f(800., 40.)));
    runtime.observe_cursor(RectF::new(vec2f(10., 20.), vec2f(2., 16.)), ColorU::white());
    runtime.command_started();
    runtime.block_fold_changed(RectF::new(vec2f(0., 10.), vec2f(800., 40.)), true);
    let first = runtime.finish_frame();
    assert!(first.cells.is_empty());
    assert!(first.rows.is_empty());
    assert!(first.cursor.is_some());

    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(20., 40.), 8, false);
    runtime.observe_block(7, RectF::new(vec2f(0., 10.), vec2f(800., 80.)));
    runtime.command_completed();
    let second = runtime.finish_frame();
    assert!(second.cells.is_empty());
    assert!(second.rows.is_empty());
    assert!(second.particles.is_empty());
    assert!(second.cursor.is_some());
    assert!(second.blocks.iter().all(|block| block.effect_kind == 0));
}

#[test]
fn input_erase_emits_a_bounded_particle_burst() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());

    runtime.emit_input_erase_burst(vec2f(30., 40.), vec2f(30., 40.), 17, ColorU::white());
    let frame = runtime.finish_frame();

    assert_eq!(frame.particles.len(), INPUT_ERASE_DEBRIS_COUNT);
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.origin.x() > 30.)
    );
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.origin.y() >= 40. - frame.cell_size.y() * 0.12)
    );
    assert!(
        frame.active_until_seconds - frame.elapsed_seconds
            >= frame.duration_seconds * MAX_PARTICLE_DURATION_SCALE,
        "erase particles should retain their extended particle duration"
    );
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.velocity.x() >= 0.)
    );
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.velocity.y() < 0.)
    );
    assert!(frame.particles.iter().all(|particle| particle.seed < 0.82));
    let (min_alpha, max_alpha) = frame
        .particles
        .iter()
        .map(|particle| particle.color[3])
        .fold((1.0_f32, 0.0_f32), |(min, max), alpha| {
            (min.min(alpha), max.max(alpha))
        });
    assert!(max_alpha - min_alpha > 0.1);
    assert!(frame.is_active());
}

#[test]
fn particle_spread_scales_range_and_preserves_approximate_density() {
    let mut compact_config = config();
    compact_config.particle_spread = 0.5;
    let mut compact = TerminalVisualRuntime::new(compact_config);
    begin(&mut compact, compact_config);
    compact.emit_input_erase_burst(vec2f(30., 40.), vec2f(30., 40.), 17, ColorU::white());
    let compact_frame = compact.finish_frame();

    let mut expansive_config = config();
    expansive_config.particle_spread = 2.0;
    let mut expansive = TerminalVisualRuntime::new(expansive_config);
    begin(&mut expansive, expansive_config);
    expansive.emit_input_erase_burst(vec2f(30., 40.), vec2f(30., 40.), 17, ColorU::white());
    let expansive_frame = expansive.finish_frame();

    assert_eq!(compact_frame.particles.len(), 6);
    assert_eq!(expansive_frame.particles.len(), 100);
    let compact_density = compact_frame.particles.len() as f32 / 0.5_f32.powi(2);
    let expansive_density = expansive_frame.particles.len() as f32 / 2.0_f32.powi(2);
    assert!((compact_density - expansive_density).abs() <= 1.0);
    assert!(
        expansive_frame
            .particles
            .iter()
            .map(|particle| particle.origin.x())
            .reduce(f32::max)
            .unwrap()
            > compact_frame
                .particles
                .iter()
                .map(|particle| particle.origin.x())
                .reduce(f32::max)
                .unwrap()
    );
}

#[test]
fn shatter_adds_a_subtle_residual_layer_behind_upward_debris() {
    let mut shatter = config();
    shatter.profile.erase = 512;
    let mut runtime = TerminalVisualRuntime::new(shatter);
    begin(&mut runtime, shatter);

    runtime.emit_input_erase_burst(vec2f(30., 40.), vec2f(30., 40.), 17, ColorU::white());
    let frame = runtime.finish_frame();

    assert_eq!(frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);
    assert_eq!(
        frame
            .particles
            .iter()
            .filter(|particle| particle.seed >= 0.82)
            .count(),
        12
    );
    assert!(
        frame
            .particles
            .iter()
            .filter(|particle| particle.seed >= 0.82)
            .all(|particle| particle.color[3] < 0.3 && particle.velocity.y() < 0.)
    );
    assert!(
        frame
            .particles
            .iter()
            .filter(|particle| particle.seed < 0.82)
            .all(|particle| particle.velocity.y() < 0.)
    );
}

#[test]
fn static_output_backspace_emits_particles_and_retargets_the_grid_cursor() {
    let mut runtime = TerminalVisualRuntime::new_static_output(config());
    runtime.begin_frame(
        config(),
        RectF::new(Vector2F::zero(), vec2f(800., 600.)),
        vec2f(10., 20.),
        true,
    );
    runtime.observe_cursor(RectF::new(vec2f(30., 40.), vec2f(2., 16.)), ColorU::white());
    runtime.finish_frame();

    assert!(runtime.input_backspace());
    runtime.begin_frame(
        config(),
        RectF::new(Vector2F::zero(), vec2f(800., 600.)),
        vec2f(10., 20.),
        true,
    );
    runtime.observe_cursor(RectF::new(vec2f(20., 40.), vec2f(2., 16.)), ColorU::white());
    let frame = runtime.finish_frame();

    assert!(frame.cells.is_empty());
    assert_eq!(frame.particles.len(), INPUT_ERASE_DEBRIS_COUNT);
    assert_eq!(frame.cursor.unwrap().duration_scale, 0.25);
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.origin.x() > 20.)
    );
}

#[test]
fn erased_input_glyph_stays_in_the_visual_lane_until_its_dissolve_finishes() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    let started = runtime.start_input_erase_ghost().unwrap();
    let cell = runtime
        .continue_input_erase_ghost(vec2f(30., 40.), 17, started)
        .unwrap();
    let frame = runtime.finish_frame();

    assert_eq!(
        frame.cell(cell).unwrap().flags,
        TERMINAL_CELL_FLAG_ERASE_GHOST
    );
    assert!(frame.is_active());
}

#[test]
fn reduced_motion_disables_tracks_but_keeps_material_snapshot() {
    let mut reduced = config();
    reduced.reduce_motion = true;
    let mut runtime = TerminalVisualRuntime::new(reduced);
    begin(&mut runtime, reduced);
    assert!(runtime.observe_cell(vec2f(10., 20.), 7, false).is_none());
    assert!(!runtime.finish_frame().is_active());
}

#[test]
fn alternate_screen_suppresses_grid_glyph_motion() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.finish_frame();
    runtime.command_started();
    runtime.begin_frame(
        config(),
        RectF::new(Vector2F::zero(), vec2f(800., 600.)),
        vec2f(10., 20.),
        true,
    );
    runtime.observe_cell(vec2f(10., 20.), 8, false);
    let frame = runtime.finish_frame();
    assert!(frame.blocks.is_empty());
    assert!(frame.cells.is_empty());
}

#[test]
fn idle_grid_changes_do_not_compete_with_the_input_lane() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.finish_frame();

    begin(&mut runtime, config());
    assert!(runtime.observe_cell(vec2f(20., 20.), 8, false).is_none());
    assert!(runtime.finish_frame().cells.is_empty());
}

#[test]
fn expired_tracks_stop_requesting_frames() {
    let mut short = config();
    short.duration_seconds = 0.005;
    let mut runtime = TerminalVisualRuntime::new(short);
    begin(&mut runtime, short);
    runtime.start_input_cell_motion(vec2f(10., 20.), 7);
    runtime.finish_frame();
    thread::sleep(Duration::from_millis(12));
    begin(&mut runtime, short);
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    assert!(!runtime.finish_frame().is_active());
}

#[test]
fn off_variants_emit_the_renderer_off_identifier() {
    assert_eq!(new_line_effect(NewLineMotion::Off), TERMINAL_EFFECT_OFF);
    assert_eq!(commit_effect(CommitMotion::Off), TERMINAL_EFFECT_OFF);
    assert_eq!(keypress_effect(KeypressMotion::Off), TERMINAL_EFFECT_OFF);
    assert_eq!(erase_effect(EraseMotion::Off), TERMINAL_EFFECT_OFF);
    assert_eq!(block_effect(BlockFoldMotion::Off), TERMINAL_EFFECT_OFF);
}

#[test]
fn visible_erase_variants_have_stable_distinct_shader_identifiers() {
    let variants = [
        (EraseMotion::Shatter, 512),
        (EraseMotion::Flame, 515),
        (EraseMotion::Sandstorm, 516),
        (EraseMotion::Droplet, 517),
        (EraseMotion::SpaceRipple, 518),
        (EraseMotion::Ash, 519),
        (EraseMotion::Crystal, 520),
        (EraseMotion::InkBloom, 521),
        (EraseMotion::Vapor, 522),
        (EraseMotion::PixelDust, 523),
    ];
    for (motion, expected) in variants {
        assert_eq!(erase_effect(motion), expected);
    }
}

#[test]
fn space_ripple_uses_three_delayed_bounded_ring_instances() {
    let mut ripple = config();
    ripple.profile.erase = erase_effect(EraseMotion::SpaceRipple);
    let mut runtime = TerminalVisualRuntime::new(ripple);
    begin(&mut runtime, ripple);

    runtime.emit_input_erase_burst(vec2f(30., 40.), vec2f(30., 40.), 17, ColorU::white());
    let frame = runtime.finish_frame();

    assert_eq!(frame.particles.len(), 3);
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.effect_kind == 518
                && particle.velocity.x() == 0.
                && particle.velocity.y() == 0.)
    );
    assert!(frame.particles[0].start_seconds < frame.particles[2].start_seconds);
    assert!(frame.is_active());
}

#[test]
fn visible_blocks_and_row_reflow_are_captured_without_changing_grid_data() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.observe_cell(vec2f(20., 20.), 8, false);
    runtime.observe_block(7, RectF::new(vec2f(0., 10.), vec2f(800., 40.)));
    let first = runtime.finish_frame();
    assert_eq!(first.blocks.len(), 1);

    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 40.), 7, false);
    runtime.observe_cell(vec2f(20., 40.), 8, false);
    let second = runtime.finish_frame();
    assert_eq!(second.rows.len(), 1);
    assert_eq!(second.rows[0].previous_row, 1);
    assert_eq!(second.rows[0].row, 2);
}

#[test]
fn duration_changes_rescale_active_deadlines() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.start_input_cell_motion(vec2f(10., 20.), 7);
    runtime.finish_frame();

    let mut shorter = config();
    shorter.duration_seconds = 0.001;
    thread::sleep(Duration::from_millis(3));
    begin(&mut runtime, shorter);
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    assert!(!runtime.finish_frame().is_active());
}

#[test]
fn alternate_screen_rejects_command_block_material_metadata() {
    let mut runtime = TerminalVisualRuntime::new(config());
    runtime.begin_frame(
        config(),
        RectF::new(Vector2F::zero(), vec2f(800., 600.)),
        vec2f(10., 20.),
        true,
    );
    runtime.observe_block(7, RectF::new(vec2f(0., 0.), vec2f(800., 40.)));
    assert!(runtime.finish_frame().blocks.is_empty());
}

#[test]
fn command_output_uses_new_line_motion_after_commit_transition_settles() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.finish_frame();
    runtime.command_started();
    thread::sleep(Duration::from_millis(90));
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(20., 20.), 8, false);
    assert_eq!(runtime.finish_frame().cells[0].effect_kind, 101);
}

#[test]
fn command_block_height_changes_create_fold_motion() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_block(7, RectF::new(vec2f(0., 10.), vec2f(800., 40.)));
    runtime.finish_frame();

    begin(&mut runtime, config());
    runtime.observe_block(7, RectF::new(vec2f(0., 10.), vec2f(800., 80.)));
    let frame = runtime.finish_frame();
    assert_eq!(frame.blocks[0].effect_kind, 601);
    assert_eq!(frame.blocks[0].phase, 1.);
}

#[test]
fn latest_semantic_event_replaces_an_overlapping_pulse() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(10., 20.), 7, false);
    runtime.finish_frame();
    runtime.command_started();
    runtime.command_completed();

    begin(&mut runtime, config());
    runtime.observe_cell(vec2f(20., 20.), 8, false);
    assert_eq!(runtime.finish_frame().cells[0].effect_kind, 101);
}

#[test]
fn uncommitted_composition_without_a_grid_cell_emits_no_motion() {
    let mut runtime = TerminalVisualRuntime::new(config());
    begin(&mut runtime, config());

    let frame = runtime.finish_frame();
    assert!(frame.cells.is_empty());
    assert!(!frame.is_active());
}
