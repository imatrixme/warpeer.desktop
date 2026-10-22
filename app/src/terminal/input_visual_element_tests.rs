use pathfinder_color::ColorU;
use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::vec2f;
use warpui::terminal_visuals::{
    TerminalCursorMotion, TerminalMaterialProfile, TerminalMotionProfile, TerminalVisualLayer,
};

use super::{
    InputCursorSnapshot, InputEditDirection, InputGlyphSnapshot, InputVisualFrame,
    TerminalInputVisualRuntime, input_edit_direction, input_glyph_before_cursor,
    input_text_snapshot, input_text_snapshot_with_previous, reconcile_glyph_change,
    should_replace_native_cursor,
};
use crate::editor::PlainTextEditorViewAction;
use crate::terminal::visual_runtime::{INPUT_SHATTER_PARTICLE_COUNT, TerminalVisualConfig};

fn glyph(hash: u64, index: usize) -> InputGlyphSnapshot {
    InputGlyphSnapshot {
        hash,
        position: vec2f(index as f32 * 10., 20.),
        scene_index: index,
    }
}

fn visual_config() -> TerminalVisualConfig {
    TerminalVisualConfig {
        enabled: true,
        duration_seconds: 0.24,
        particle_spread: 1.0,
        reduce_motion: false,
        profile: TerminalMotionProfile {
            keypress: 302,
            cursor: 401,
            erase: 512,
            ..Default::default()
        },
        material: TerminalMaterialProfile::default(),
    }
}

#[test]
fn append_animates_only_the_last_inserted_glyph() {
    let previous = vec![glyph(1, 0), glyph(2, 1)];
    let current = vec![glyph(1, 0), glyph(2, 1), glyph(3, 2), glyph(4, 3)];

    let change = reconcile_glyph_change(&previous, &current, None, InputEditDirection::Forward);

    assert_eq!(change.inserted_scene_index, Some(3));
    assert!(change.removed.is_empty());
}

#[test]
fn backspace_reports_the_removed_glyph_without_reanimating_the_line() {
    let previous = vec![glyph(1, 0), glyph(2, 1), glyph(3, 2)];
    let current = vec![glyph(1, 0), glyph(2, 1)];

    let change = reconcile_glyph_change(&previous, &current, None, InputEditDirection::Backward);

    assert_eq!(change.inserted_scene_index, None);
    assert_eq!(change.removed.len(), 1);
    assert_eq!(change.removed[0].hash, 3);
}

#[test]
fn insertion_in_the_middle_keeps_the_unchanged_suffix_static() {
    let previous = vec![glyph(1, 0), glyph(3, 1)];
    let current = vec![glyph(1, 0), glyph(2, 1), glyph(3, 2)];

    let change = reconcile_glyph_change(&previous, &current, None, InputEditDirection::Forward);

    assert_eq!(change.inserted_scene_index, Some(1));
    assert!(change.removed.is_empty());
}

#[test]
fn forward_shaping_change_never_reports_an_erased_glyph() {
    let previous = vec![glyph(1, 0), glyph(2, 1)];
    let current = vec![glyph(1, 0), glyph(9, 1)];

    let change = reconcile_glyph_change(&previous, &current, None, InputEditDirection::Forward);

    assert_eq!(change.inserted_scene_index, Some(1));
    assert!(change.removed.is_empty());
}

#[test]
fn cursor_anchor_selects_the_typed_glyph_instead_of_autosuggestion_tail() {
    let previous = vec![glyph(1, 0)];
    let current = vec![glyph(1, 0), glyph(2, 1), glyph(9, 2), glyph(10, 3)];
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(20., 20.), vec2f(2., 16.)),
        color: ColorU::white(),
    };

    let change = reconcile_glyph_change(
        &previous,
        &current,
        Some(cursor),
        InputEditDirection::Forward,
    );

    assert_eq!(change.inserted_scene_index, Some(1));
}

#[test]
fn cursor_anchor_selects_only_the_character_removed_by_backspace() {
    let previous = vec![
        glyph(1, 0),
        glyph(2, 1),
        glyph(3, 2),
        glyph(9, 3),
        glyph(10, 4),
    ];
    let current = vec![glyph(1, 0), glyph(2, 1), glyph(9, 2), glyph(11, 3)];
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(20., 20.), vec2f(2., 16.)),
        color: ColorU::white(),
    };

    let change = reconcile_glyph_change(
        &previous,
        &current,
        Some(cursor),
        InputEditDirection::Backward,
    );

    assert_eq!(change.inserted_scene_index, None);
    assert_eq!(change.removed.len(), 1);
    assert_eq!(change.removed[0].hash, 3);
}

#[test]
fn stationary_repaint_does_not_animate_changed_input_glyphs() {
    let previous = vec![glyph(1, 0), glyph(2, 1)];
    let current = vec![glyph(1, 0), glyph(9, 1)];

    let change = reconcile_glyph_change(&previous, &current, None, InputEditDirection::Stationary);

    assert_eq!(change.inserted_scene_index, None);
    assert!(change.removed.is_empty());
}

#[test]
fn cursor_navigation_without_a_buffer_edit_has_no_direction() {
    let previous = input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar));
    let current = input_text_snapshot("abc", 2, None);

    assert_eq!(
        input_edit_direction(Some(previous), current, false, false),
        InputEditDirection::Stationary
    );
}

#[test]
fn authoritative_buffer_length_distinguishes_insert_from_delete() {
    let initial = input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::InsertChar));
    let inserted = input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar));
    let deleted = input_text_snapshot("a", 1, Some(PlainTextEditorViewAction::Backspace));

    assert_eq!(
        input_edit_direction(Some(initial), inserted, false, false),
        InputEditDirection::Forward
    );
    assert_eq!(
        input_edit_direction(Some(initial), deleted, false, false),
        InputEditDirection::Backward
    );
}

#[test]
fn shrinking_buffer_remains_a_delete_when_the_undo_action_was_replaced() {
    let initial = input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar));
    let deleted = input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::CursorChanged));

    assert_eq!(
        input_edit_direction(Some(initial), deleted, false, true),
        InputEditDirection::Backward
    );
}

#[test]
fn cursor_changed_does_not_turn_a_whole_line_clear_into_backspace() {
    let initial = input_text_snapshot(
        "echo hello",
        10,
        Some(PlainTextEditorViewAction::InsertChar),
    );
    let cleared = input_text_snapshot("", 0, Some(PlainTextEditorViewAction::CursorChanged));

    assert_eq!(
        input_edit_direction(Some(initial), cleared, false, false),
        InputEditDirection::Stationary
    );
}

#[test]
fn ime_commit_can_animate_the_glyph_before_the_cursor_without_a_glyph_diff() {
    let current = vec![glyph(1, 0), glyph(2, 1), glyph(3, 2)];
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 20.), vec2f(2., 16.)),
        color: ColorU::white(),
    };

    assert_eq!(input_glyph_before_cursor(&current, cursor), Some(2));
}

#[test]
fn ime_commit_uses_the_precomposition_buffer_as_its_forward_signal() {
    let initial = input_text_snapshot("a", 1, Some(PlainTextEditorViewAction::InsertChar));
    let committed = input_text_snapshot(
        "a中文",
        7,
        Some(PlainTextEditorViewAction::UpdateMarkedText),
    );

    assert_eq!(
        input_edit_direction(Some(initial), committed, true, false),
        InputEditDirection::Forward
    );
    assert_eq!(
        input_edit_direction(Some(initial), committed, false, false),
        InputEditDirection::Stationary
    );
}

#[test]
fn non_edit_replacements_never_masquerade_as_typing_or_deletion() {
    let initial = input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar));
    let history = input_text_snapshot(
        "long history command",
        20,
        Some(PlainTextEditorViewAction::ReplaceBuffer),
    );
    let command_clear = input_text_snapshot("", 0, Some(PlainTextEditorViewAction::ClearBuffer));

    assert_eq!(
        input_edit_direction(Some(initial), history, false, false),
        InputEditDirection::Stationary
    );
    assert_eq!(
        input_edit_direction(Some(initial), command_clear, false, false),
        InputEditDirection::Stationary
    );
}

#[test]
fn native_cursor_is_replaced_only_while_visual_cursor_motion_is_active() {
    let mut frame = TerminalVisualLayer::default();
    assert!(!should_replace_native_cursor(&frame));

    frame.duration_seconds = 0.2;
    frame.elapsed_seconds = 0.05;
    frame.cursor = Some(TerminalCursorMotion {
        start_seconds: 0.,
        duration_scale: 1.,
        ..Default::default()
    });
    assert!(should_replace_native_cursor(&frame));

    frame.elapsed_seconds = 0.21;
    assert!(!should_replace_native_cursor(&frame));
}

#[test]
fn backspace_during_a_native_cursor_blink_off_frame_still_emits_particles() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar)),
        &[glyph(1, 0), glyph(2, 1), glyph(3, 2)],
        Some(cursor),
    );
    runtime.input_backspace();

    let (frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::Backspace)),
        &[glyph(1, 0), glyph(2, 1)],
        None,
    );

    assert_eq!(frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);
    assert!(frame.cursor.is_none());
}

#[test]
fn cursor_reappears_without_animating_from_its_pre_blink_position() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let first_cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let next_cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(50., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };

    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::CursorChanged)),
        &[glyph(1, 0), glyph(2, 1), glyph(3, 2)],
        Some(first_cursor),
    );
    let (hidden_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::CursorChanged)),
        &[glyph(1, 0), glyph(2, 1), glyph(3, 2)],
        None,
    );
    assert!(hidden_frame.cursor.is_none());

    let (visible_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::CursorChanged)),
        &[glyph(1, 0), glyph(2, 1), glyph(3, 2)],
        Some(next_cursor),
    );
    let cursor = visible_frame.cursor.unwrap();
    assert_eq!(cursor.from, next_cursor.bounds.origin());
    assert_eq!(cursor.to, next_cursor.bounds.origin());
}

#[test]
fn visible_state_without_a_rendered_cursor_does_not_restore_stale_motion() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(80., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let text = input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::CursorChanged));
    let glyphs = [glyph(1, 0), glyph(2, 1), glyph(3, 2)];

    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        text,
        &glyphs,
        Some(cursor),
    );
    runtime.render_frame(InputVisualFrame {
        config,
        bounds,
        cell_size: vec2f(10., 20.),
        composition_active: false,
        text,
        glyphs: &glyphs,
        cursor: None,
        cursor_visible: false,
    });

    let (frame, _) = runtime.render_frame(InputVisualFrame {
        config,
        bounds,
        cell_size: vec2f(10., 20.),
        composition_active: false,
        text,
        glyphs: &glyphs,
        cursor: None,
        cursor_visible: true,
    });

    assert!(frame.cursor.is_none());
}

#[test]
fn backspace_emits_once_even_when_the_glyph_scene_catches_up_later() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let previous_glyphs = [glyph(1, 0), glyph(2, 1), glyph(3, 2)];
    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar)),
        &previous_glyphs,
        Some(cursor),
    );
    runtime.input_backspace();

    let (model_first_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::Backspace)),
        &previous_glyphs,
        None,
    );
    assert_eq!(
        model_first_frame.particles.len(),
        INPUT_SHATTER_PARTICLE_COUNT
    );

    let (scene_catch_up_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::CursorChanged)),
        &[glyph(1, 0), glyph(2, 1)],
        None,
    );

    assert_eq!(
        scene_catch_up_frame.particles.len(),
        INPUT_SHATTER_PARTICLE_COUNT
    );
}

#[test]
fn authoritative_backspace_survives_a_preupdated_frame_snapshot() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(20., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let stale_glyphs = [glyph(1, 0), glyph(2, 1), glyph(3, 2)];

    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::CursorChanged)),
        &stale_glyphs,
        Some(cursor),
    );
    runtime.input_backspace();

    let (model_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot_with_previous(
            "ab",
            2,
            Some(PlainTextEditorViewAction::Backspace),
            "abc",
        ),
        &stale_glyphs,
        Some(cursor),
    );
    assert_eq!(model_frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);

    let (scene_frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot_with_previous(
            "ab",
            2,
            Some(PlainTextEditorViewAction::Backspace),
            "abc",
        ),
        &[glyph(1, 0), glyph(2, 1)],
        Some(cursor),
    );

    assert_eq!(scene_frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);
}

#[test]
fn empty_authoritative_snapshot_falls_back_to_the_last_rendered_buffer() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };

    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar)),
        &[glyph(1, 0), glyph(2, 1), glyph(3, 2)],
        Some(cursor),
    );
    runtime.input_backspace();

    let (frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot_with_previous("", 0, Some(PlainTextEditorViewAction::Backspace), ""),
        &[],
        Some(cursor),
    );

    assert_eq!(frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);
}

#[test]
fn backspace_uses_the_new_cursor_position_without_waiting_for_a_glyph_diff() {
    let config = visual_config();
    let mut runtime = TerminalInputVisualRuntime::new(config);
    let bounds = RectF::new(vec2f(0., 0.), vec2f(200., 30.));
    let old_cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(30., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let new_cursor = InputCursorSnapshot {
        bounds: RectF::new(vec2f(20., 4.), vec2f(2., 16.)),
        color: ColorU::white(),
    };
    let stale_glyphs = [glyph(1, 0), glyph(2, 1), glyph(3, 2)];
    runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("abc", 3, Some(PlainTextEditorViewAction::InsertChar)),
        &stale_glyphs,
        Some(old_cursor),
    );
    runtime.input_backspace();

    let (frame, _) = runtime.render_test_frame(
        config,
        bounds,
        vec2f(10., 20.),
        false,
        input_text_snapshot("ab", 2, Some(PlainTextEditorViewAction::Backspace)),
        &stale_glyphs,
        Some(new_cursor),
    );

    assert_eq!(frame.particles.len(), INPUT_SHATTER_PARTICLE_COUNT);
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.origin.x() >= 20.)
    );
    assert!(
        frame
            .particles
            .iter()
            .all(|particle| particle.origin.y() >= new_cursor.bounds.origin_y())
    );
}
