use settings::Setting as _;
use settings_value::SettingsValue as _;

use super::*;

#[test]
fn visual_variants_use_stable_snake_case_values() {
    assert_eq!(
        CursorMotion::Phosphor.to_file_value(),
        serde_json::json!("phosphor")
    );
    let shatter = serde_json::json!("shatter");
    assert_eq!(
        EraseMotion::from_file_value(&shatter).unwrap(),
        EraseMotion::Shatter
    );
    for (motion, value) in [
        (EraseMotion::Flame, "flame"),
        (EraseMotion::Sandstorm, "sandstorm"),
        (EraseMotion::Droplet, "droplet"),
        (EraseMotion::SpaceRipple, "space_ripple"),
        (EraseMotion::Ash, "ash"),
        (EraseMotion::Crystal, "crystal"),
        (EraseMotion::InkBloom, "ink_bloom"),
        (EraseMotion::Vapor, "vapor"),
        (EraseMotion::PixelDust, "pixel_dust"),
    ] {
        assert_eq!(motion.to_file_value(), serde_json::json!(value));
    }
    assert!(EraseMotion::from_file_value(&serde_json::json!("unknown")).is_none());
}

#[test]
fn duration_is_bounded_and_recovers_from_invalid_values() {
    assert_eq!(normalized_terminal_visual_duration_ms(10.), 60.);
    assert_eq!(normalized_terminal_visual_duration_ms(5000.), 1200.);
    assert_eq!(
        normalized_terminal_visual_duration_ms(f32::NAN),
        DEFAULT_TERMINAL_VISUAL_DURATION_MS
    );
}

#[test]
fn particle_spread_is_bounded_and_recovers_from_invalid_values() {
    assert_eq!(normalized_terminal_particle_spread(0.2), 0.5);
    assert_eq!(normalized_terminal_particle_spread(3.0), 2.0);
    assert_eq!(
        normalized_terminal_particle_spread(f32::NAN),
        DEFAULT_TERMINAL_PARTICLE_SPREAD
    );
}

#[test]
fn generic_defaults_keep_the_optional_renderer_disabled() {
    assert!(!TerminalVisualsEnabled::default_value());
    assert_eq!(NewLineMotion::default(), NewLineMotion::Slide);
    assert_eq!(CommitMotion::default(), CommitMotion::Carry);
    assert_eq!(KeypressMotion::default(), KeypressMotion::Rise);
    assert_eq!(EraseMotion::default(), EraseMotion::Shatter);
    assert_eq!(CursorMotion::default(), CursorMotion::Ease);
    assert_eq!(BlockFoldMotion::default(), BlockFoldMotion::Height);
    assert_eq!(TerminalVisualDurationMs::default_value(), 240.);
    assert_eq!(TerminalParticleSpread::default_value(), 1.);
    assert!(!TerminalScrollGlide::default_value());
    assert_eq!(TerminalMaterial::default(), TerminalMaterial::SmokedGlass);
}
