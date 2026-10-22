use super::{PaneIndicatorState, inactive_pane_overlay_opacity, pane_indicator_state};

#[test]
fn pane_indicator_is_hidden_when_the_setting_is_disabled() {
    assert_eq!(pane_indicator_state(true, false, true, true), None);
}

#[test]
fn pane_indicator_is_hidden_for_unsupported_pane_types() {
    assert_eq!(pane_indicator_state(false, true, true, true), None);
}

#[test]
fn active_pane_uses_the_fixed_indicator_state() {
    assert_eq!(
        pane_indicator_state(true, true, true, false),
        Some(PaneIndicatorState::Active)
    );
}

#[test]
fn hovered_pane_uses_the_hover_indicator_state() {
    assert_eq!(
        pane_indicator_state(true, true, false, true),
        Some(PaneIndicatorState::Hovered)
    );
}

#[test]
fn active_indicator_stays_full_strength_while_hovered() {
    assert_eq!(
        pane_indicator_state(true, true, true, true),
        Some(PaneIndicatorState::Active)
    );
}

#[test]
fn inactive_pane_hover_uses_half_strength_overlay() {
    assert_eq!(inactive_pane_overlay_opacity(true, false), Some(100));
    assert_eq!(inactive_pane_overlay_opacity(true, true), Some(50));
    assert_eq!(inactive_pane_overlay_opacity(false, true), None);
}
