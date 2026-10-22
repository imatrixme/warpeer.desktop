use serde::{Deserialize, Serialize};
use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

macro_rules! visual_enum {
    ($name:ident, $default:ident, [$($variant:ident),+ $(,)?]) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            Eq,
            PartialEq,
            Serialize,
            Deserialize,
            schemars::JsonSchema,
            settings_value::SettingsValue,
        )]
        #[serde(rename_all = "snake_case")]
        #[schemars(rename_all = "snake_case")]
        pub enum $name {
            $($variant,)+
            #[default]
            $default,
        }
    };
}

visual_enum!(
    NewLineMotion,
    Slide,
    [Off, Stagger, Snap, Glow, Focus, Recede]
);
visual_enum!(
    CommitMotion,
    Carry,
    [Off, Arc, Lean, Ripple, Streak, Quantize]
);
visual_enum!(
    KeypressMotion,
    Rise,
    [Off, Fade, Pop, Extrude, Heat, Echo, Drop, Ink, Squeeze]
);
visual_enum!(
    CursorMotion,
    Ease,
    [Snap, Spring, Smear, Squash, Phosphor, Arc, Warp, Beam]
);
visual_enum!(
    EraseMotion,
    Shatter,
    [
        Iris,
        Undertow,
        Echo,
        Bleed,
        Unravel,
        Recede,
        Sublime,
        Reverse,
        Sweep,
        Drain,
        Dissolve,
        Collapse,
        Gravity,
        Auto,
        Flame,
        Sandstorm,
        Droplet,
        SpaceRipple,
        Ash,
        Crystal,
        InkBloom,
        Vapor,
        PixelDust,
        Off
    ]
);
visual_enum!(
    BlockFoldMotion,
    Height,
    [Off, Stagger, Wipe, Converge, Seam, Drain]
);
visual_enum!(
    TerminalMaterial,
    SmokedGlass,
    [Flat, Obsidian, Frosted, Carbon]
);
visual_enum!(ReducedMotionPolicy, System, [Always, Never]);

pub const MIN_TERMINAL_VISUAL_DURATION_MS: f32 = 60.;
pub const MAX_TERMINAL_VISUAL_DURATION_MS: f32 = 1200.;
pub const DEFAULT_TERMINAL_VISUAL_DURATION_MS: f32 = 240.;
pub const MIN_TERMINAL_PARTICLE_SPREAD: f32 = 0.5;
pub const MAX_TERMINAL_PARTICLE_SPREAD: f32 = 2.0;
pub const DEFAULT_TERMINAL_PARTICLE_SPREAD: f32 = 1.0;

pub fn normalized_terminal_visual_duration_ms(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(
            MIN_TERMINAL_VISUAL_DURATION_MS,
            MAX_TERMINAL_VISUAL_DURATION_MS,
        )
    } else {
        DEFAULT_TERMINAL_VISUAL_DURATION_MS
    }
}

pub fn normalized_terminal_particle_spread(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_TERMINAL_PARTICLE_SPREAD, MAX_TERMINAL_PARTICLE_SPREAD)
    } else {
        DEFAULT_TERMINAL_PARTICLE_SPREAD
    }
}

define_settings_group!(TerminalVisualSettings, settings: [
    enabled: TerminalVisualsEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.enabled",
        description: "Whether semantic terminal motion and material depth are enabled.",
    },
    duration_ms: TerminalVisualDurationMs {
        type: f32,
        default: DEFAULT_TERMINAL_VISUAL_DURATION_MS,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.duration_ms",
        description: "The shared duration in milliseconds for terminal visual motion.",
    },
    particle_spread: TerminalParticleSpread {
        type: f32,
        default: DEFAULT_TERMINAL_PARTICLE_SPREAD,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.particle_spread",
        description: "The spatial spread multiplier for terminal erase particles.",
    },
    reduced_motion: TerminalVisualReducedMotion {
        type: ReducedMotionPolicy,
        default: ReducedMotionPolicy::System,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.reduced_motion",
        description: "How terminal visuals respond to reduced-motion preferences.",
    },
    new_line_motion: TerminalNewLineMotion {
        type: NewLineMotion,
        default: NewLineMotion::Slide,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.new_line",
        description: "Motion applied when terminal output creates a new line.",
    },
    commit_motion: TerminalCommitMotion {
        type: CommitMotion,
        default: CommitMotion::Carry,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.commit",
        description: "Motion applied when a command is committed.",
    },
    keypress_motion: TerminalKeypressMotion {
        type: KeypressMotion,
        default: KeypressMotion::Rise,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.keypress",
        description: "Motion applied to newly committed grapheme clusters.",
    },
    cursor_motion: TerminalCursorMotion {
        type: CursorMotion,
        default: CursorMotion::Ease,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.cursor",
        description: "Motion applied when the terminal cursor moves.",
    },
    erase_motion: TerminalEraseMotion {
        type: EraseMotion,
        default: EraseMotion::Shatter,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.erase",
        description: "Motion applied to erased terminal cells.",
    },
    block_fold_motion: TerminalBlockFoldMotion {
        type: BlockFoldMotion,
        default: BlockFoldMotion::Height,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.block_fold",
        description: "Motion applied when a command block folds or expands.",
    },
    scroll_glide: TerminalScrollGlide {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.scroll_glide",
        description: "Whether discrete wheel scrolling uses bounded glide.",
    },
    material: TerminalVisualMaterial {
        type: TerminalMaterial,
        default: TerminalMaterial::SmokedGlass,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.material",
        description: "The material treatment used behind terminal content.",
    },
    substrate_enabled: TerminalVisualSubstrateEnabled {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.substrate_enabled",
        description: "Whether the terminal material substrate is rendered.",
    },
    command_gutter: TerminalCommandGutter {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.command_gutter",
        description: "Whether command blocks receive a restrained gutter marker.",
    },
    block_depth: TerminalBlockDepth {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "appearance.terminal_visuals.block_depth",
        description: "Whether command blocks receive subtle depth separation.",
    },
]);

#[cfg(test)]
#[path = "terminal_visuals_tests.rs"]
mod tests;
