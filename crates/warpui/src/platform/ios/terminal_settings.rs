#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalFontFamily {
    JetBrainsMono,
    MapleMonoNF,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalFontWeight {
    Thin,
    ExtraLight,
    Light,
    Normal,
    Medium,
    Semibold,
    Bold,
    ExtraBold,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCursorStyle {
    Block,
    Bar,
    Underline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalCursorMotion {
    Snap,
    Ease,
    Spring,
    Smear,
    Squash,
    Phosphor,
    Arc,
    Warp,
    Beam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalInputEffect {
    Off,
    Fade,
    Rise,
    Pop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalDeletionEffect {
    Off,
    Recede,
    Dissolve,
    Shatter,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalRenderSettings {
    pub foreground: [u8; 4],
    pub background: [u8; 4],
    pub cursor: [u8; 4],
    pub font_family: TerminalFontFamily,
    pub font_weight: TerminalFontWeight,
    pub font_size: f32,
    pub line_height_ratio: f32,
    pub cursor_style: TerminalCursorStyle,
    pub cursor_blink: bool,
    pub effects_enabled: bool,
    pub cursor_motion: TerminalCursorMotion,
    pub input_effect: TerminalInputEffect,
    pub deletion_effect: TerminalDeletionEffect,
    pub effect_duration_ms: f32,
    pub reduced_motion: bool,
}

impl Default for TerminalRenderSettings {
    fn default() -> Self {
        Self {
            foreground: [222, 224, 230, 255],
            background: [12, 14, 20, 255],
            cursor: [126, 220, 190, 255],
            font_family: TerminalFontFamily::JetBrainsMono,
            font_weight: TerminalFontWeight::Normal,
            font_size: 8.0,
            line_height_ratio: 1.375,
            cursor_style: TerminalCursorStyle::Block,
            cursor_blink: true,
            effects_enabled: false,
            cursor_motion: TerminalCursorMotion::Ease,
            input_effect: TerminalInputEffect::Fade,
            deletion_effect: TerminalDeletionEffect::Shatter,
            effect_duration_ms: 180.0,
            reduced_motion: false,
        }
    }
}
