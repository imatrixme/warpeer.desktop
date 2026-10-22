use std::time::Duration;

use super::{CursorBlinkActivity, cursor_blink_restart_delay, resolve_cursor_presentation};
use crate::settings::{CursorBlink, CursorDisplayType};
use crate::terminal::model::ansi::{CursorShape, CursorStyle};

#[test]
fn terminal_only_cursor_uses_user_shape_instead_of_tmux_shape() {
    let presentation = resolve_cursor_presentation(
        CursorStyle {
            shape: CursorShape::Block,
            blinking: false,
        },
        true,
        CursorDisplayType::Underline,
        CursorBlink::Disabled,
        true,
        Duration::ZERO,
    );

    assert_eq!(presentation.style.shape, CursorShape::Underline);
    assert!(!presentation.style.blinking);
    assert!(presentation.visible);
    assert_eq!(presentation.repaint_after, None);
}

#[test]
fn terminal_only_cursor_waits_for_idle_before_blinking() {
    let presentation = resolve_cursor_presentation(
        CursorStyle::default(),
        true,
        CursorDisplayType::Bar,
        CursorBlink::Enabled,
        true,
        Duration::from_millis(600),
    );

    assert!(presentation.visible);
    assert_eq!(presentation.repaint_after, Some(Duration::from_millis(400)));
}

#[test]
fn terminal_only_cursor_blinks_after_idle_delay() {
    let hidden = resolve_cursor_presentation(
        CursorStyle::default(),
        true,
        CursorDisplayType::Bar,
        CursorBlink::Enabled,
        true,
        Duration::from_millis(1100),
    );
    let visible = resolve_cursor_presentation(
        CursorStyle::default(),
        true,
        CursorDisplayType::Bar,
        CursorBlink::Enabled,
        true,
        Duration::from_millis(1600),
    );

    assert!(!hidden.visible);
    assert_eq!(hidden.repaint_after, Some(Duration::from_millis(400)));
    assert!(visible.visible);
    assert_eq!(visible.repaint_after, Some(Duration::from_millis(400)));
}

#[test]
fn terminal_only_cursor_restart_includes_idle_and_visible_delays() {
    assert_eq!(
        cursor_blink_restart_delay(true),
        Duration::from_millis(1000)
    );
    assert_eq!(
        cursor_blink_restart_delay(false),
        Duration::from_millis(500)
    );
}

#[test]
fn cursor_activity_restarts_when_the_position_changes() {
    let mut activity = CursorBlinkActivity::default();

    assert_eq!(
        activity.idle_elapsed((4, 2), true, Duration::from_millis(2000)),
        Duration::ZERO
    );
    assert_eq!(
        activity.idle_elapsed((4, 2), true, Duration::from_millis(2600)),
        Duration::from_millis(600)
    );
    assert_eq!(
        activity.idle_elapsed((5, 2), true, Duration::from_millis(2700)),
        Duration::ZERO
    );
}

#[test]
fn input_activity_restarts_the_idle_period_without_cursor_movement() {
    let mut activity = CursorBlinkActivity::default();
    activity.idle_elapsed((4, 2), true, Duration::from_millis(2000));

    activity.mark_active(Duration::from_millis(2500));

    assert_eq!(
        activity.idle_elapsed((4, 2), true, Duration::from_millis(2700)),
        Duration::from_millis(200)
    );
}

#[test]
fn unfocused_terminal_cursor_stays_visible() {
    let presentation = resolve_cursor_presentation(
        CursorStyle::default(),
        true,
        CursorDisplayType::Block,
        CursorBlink::Enabled,
        false,
        Duration::from_millis(600),
    );

    assert!(presentation.visible);
    assert_eq!(presentation.repaint_after, None);
}

#[test]
fn full_warp_preserves_terminal_owned_cursor_style() {
    let terminal_style = CursorStyle {
        shape: CursorShape::Beam,
        blinking: true,
    };
    let presentation = resolve_cursor_presentation(
        terminal_style,
        false,
        CursorDisplayType::Underline,
        CursorBlink::Disabled,
        true,
        Duration::from_millis(600),
    );

    assert_eq!(presentation.style, terminal_style);
    assert!(presentation.visible);
    assert_eq!(presentation.repaint_after, None);
}
