use pathfinder_color::ColorU;
use warpui::terminal_visuals::{
    TERMINAL_EFFECT_BLOCK_BASE, TERMINAL_EFFECT_COMMIT_BASE, TERMINAL_EFFECT_CURSOR_BASE,
    TERMINAL_EFFECT_ERASE_BASE, TERMINAL_EFFECT_KEYPRESS_BASE, TERMINAL_EFFECT_NEW_LINE_BASE,
    TERMINAL_EFFECT_OFF, TerminalMaterialProfile, TerminalMotionProfile,
};
use warpui::{AppContext, SingletonEntity as _};

use crate::settings::{
    BlockFoldMotion, CommitMotion, CursorMotion, EraseMotion, KeypressMotion, NewLineMotion,
    ReducedMotionPolicy, TerminalMaterial, TerminalVisualSettings,
    normalized_terminal_visual_duration_ms,
};

#[derive(Clone, Copy, Debug)]
pub struct TerminalVisualConfig {
    pub enabled: bool,
    pub duration_seconds: f32,
    pub particle_spread: f32,
    pub reduce_motion: bool,
    pub profile: TerminalMotionProfile,
    pub material: TerminalMaterialProfile,
}

impl TerminalVisualConfig {
    pub fn from_app(app: &AppContext, tint: ColorU, accent: ColorU, foreground: ColorU) -> Self {
        let settings = TerminalVisualSettings::as_ref(app);
        let enabled = *settings.enabled;
        let reduced_motion = match *settings.reduced_motion {
            ReducedMotionPolicy::Always => true,
            ReducedMotionPolicy::Never => false,
            ReducedMotionPolicy::System => system_reduce_motion_enabled(),
        };
        Self {
            enabled,
            duration_seconds: normalized_terminal_visual_duration_ms(*settings.duration_ms) / 1000.,
            particle_spread: crate::settings::normalized_terminal_particle_spread(
                *settings.particle_spread,
            ),
            reduce_motion: reduced_motion,
            profile: TerminalMotionProfile {
                new_line: new_line_effect(*settings.new_line_motion),
                commit: commit_effect(*settings.commit_motion),
                keypress: keypress_effect(*settings.keypress_motion),
                cursor: cursor_effect(*settings.cursor_motion),
                erase: erase_effect(*settings.erase_motion),
                block_fold: block_effect(*settings.block_fold_motion),
            },
            material: TerminalMaterialProfile {
                style: material_id(*settings.material),
                substrate_enabled: enabled
                    && *settings.substrate_enabled
                    && !matches!(*settings.material, TerminalMaterial::Flat),
                command_gutter: *settings.command_gutter,
                block_depth: *settings.block_depth,
                tint: color_array(tint),
                accent: color_array(accent),
                foreground: color_array(foreground),
            },
        }
    }
}

pub(super) fn color_array(color: ColorU) -> [f32; 4] {
    let color = color.to_f32();
    [color.r(), color.g(), color.b(), color.a()]
}

pub(super) fn new_line_effect(value: NewLineMotion) -> u32 {
    match value {
        NewLineMotion::Off => TERMINAL_EFFECT_OFF,
        NewLineMotion::Slide => TERMINAL_EFFECT_NEW_LINE_BASE + 1,
        NewLineMotion::Stagger => TERMINAL_EFFECT_NEW_LINE_BASE + 2,
        NewLineMotion::Snap => TERMINAL_EFFECT_NEW_LINE_BASE + 3,
        NewLineMotion::Glow => TERMINAL_EFFECT_NEW_LINE_BASE + 4,
        NewLineMotion::Focus => TERMINAL_EFFECT_NEW_LINE_BASE + 5,
        NewLineMotion::Recede => TERMINAL_EFFECT_NEW_LINE_BASE + 6,
    }
}

pub(super) fn commit_effect(value: CommitMotion) -> u32 {
    match value {
        CommitMotion::Off => TERMINAL_EFFECT_OFF,
        CommitMotion::Carry => TERMINAL_EFFECT_COMMIT_BASE + 1,
        CommitMotion::Arc => TERMINAL_EFFECT_COMMIT_BASE + 2,
        CommitMotion::Lean => TERMINAL_EFFECT_COMMIT_BASE + 3,
        CommitMotion::Ripple => TERMINAL_EFFECT_COMMIT_BASE + 4,
        CommitMotion::Streak => TERMINAL_EFFECT_COMMIT_BASE + 5,
        CommitMotion::Quantize => TERMINAL_EFFECT_COMMIT_BASE + 6,
    }
}

pub(super) fn keypress_effect(value: KeypressMotion) -> u32 {
    match value {
        KeypressMotion::Off => TERMINAL_EFFECT_OFF,
        KeypressMotion::Fade => TERMINAL_EFFECT_KEYPRESS_BASE + 1,
        KeypressMotion::Rise => TERMINAL_EFFECT_KEYPRESS_BASE + 2,
        KeypressMotion::Pop => TERMINAL_EFFECT_KEYPRESS_BASE + 3,
        KeypressMotion::Extrude => TERMINAL_EFFECT_KEYPRESS_BASE + 4,
        KeypressMotion::Heat => TERMINAL_EFFECT_KEYPRESS_BASE + 5,
        KeypressMotion::Echo => TERMINAL_EFFECT_KEYPRESS_BASE + 6,
        KeypressMotion::Drop => TERMINAL_EFFECT_KEYPRESS_BASE + 7,
        KeypressMotion::Ink => TERMINAL_EFFECT_KEYPRESS_BASE + 8,
        KeypressMotion::Squeeze => TERMINAL_EFFECT_KEYPRESS_BASE + 9,
    }
}

fn cursor_effect(value: CursorMotion) -> u32 {
    let variant = match value {
        CursorMotion::Snap => 0,
        CursorMotion::Ease => 1,
        CursorMotion::Spring => 2,
        CursorMotion::Smear => 3,
        CursorMotion::Squash => 4,
        CursorMotion::Phosphor => 5,
        CursorMotion::Arc => 6,
        CursorMotion::Warp => 7,
        CursorMotion::Beam => 8,
    };
    TERMINAL_EFFECT_CURSOR_BASE + variant
}

pub(super) fn erase_effect(value: EraseMotion) -> u32 {
    let variant = match value {
        EraseMotion::Off => return TERMINAL_EFFECT_OFF,
        EraseMotion::Auto => 0,
        EraseMotion::Iris => 1,
        EraseMotion::Undertow => 2,
        EraseMotion::Echo => 3,
        EraseMotion::Bleed => 4,
        EraseMotion::Unravel => 5,
        EraseMotion::Recede => 6,
        EraseMotion::Sublime => 7,
        EraseMotion::Reverse => 8,
        EraseMotion::Sweep => 9,
        EraseMotion::Drain => 10,
        EraseMotion::Dissolve => 11,
        EraseMotion::Shatter => 12,
        EraseMotion::Collapse => 13,
        EraseMotion::Gravity => 14,
        EraseMotion::Flame => 15,
        EraseMotion::Sandstorm => 16,
        EraseMotion::Droplet => 17,
        EraseMotion::SpaceRipple => 18,
        EraseMotion::Ash => 19,
        EraseMotion::Crystal => 20,
        EraseMotion::InkBloom => 21,
        EraseMotion::Vapor => 22,
        EraseMotion::PixelDust => 23,
    };
    TERMINAL_EFFECT_ERASE_BASE + variant
}

pub(super) fn block_effect(value: BlockFoldMotion) -> u32 {
    match value {
        BlockFoldMotion::Off => TERMINAL_EFFECT_OFF,
        BlockFoldMotion::Height => TERMINAL_EFFECT_BLOCK_BASE + 1,
        BlockFoldMotion::Stagger => TERMINAL_EFFECT_BLOCK_BASE + 2,
        BlockFoldMotion::Wipe => TERMINAL_EFFECT_BLOCK_BASE + 3,
        BlockFoldMotion::Converge => TERMINAL_EFFECT_BLOCK_BASE + 4,
        BlockFoldMotion::Seam => TERMINAL_EFFECT_BLOCK_BASE + 5,
        BlockFoldMotion::Drain => TERMINAL_EFFECT_BLOCK_BASE + 6,
    }
}

fn material_id(value: TerminalMaterial) -> u32 {
    match value {
        TerminalMaterial::Flat => 0,
        TerminalMaterial::SmokedGlass => 1,
        TerminalMaterial::Obsidian => 2,
        TerminalMaterial::Frosted => 3,
        TerminalMaterial::Carbon => 4,
    }
}

#[cfg(target_os = "macos")]
fn system_reduce_motion_enabled() -> bool {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let enabled: bool = msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        enabled
    }
}

#[cfg(not(target_os = "macos"))]
fn system_reduce_motion_enabled() -> bool {
    false
}
