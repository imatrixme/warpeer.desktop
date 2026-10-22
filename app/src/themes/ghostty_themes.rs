use serde::Deserialize;
use warp_core::ui::theme::{AnsiColor, AnsiColors, Details, Fill, TerminalColors, WarpTheme};
use warpui::color::ColorU;

const CATALOG: &str = include_str!("../../assets/warpeer/ghostty-themes.json");

#[derive(Deserialize)]
struct GhosttyThemeCatalog {
    schema_version: u32,
    themes: Vec<GhosttyThemeDefinition>,
}

#[derive(Deserialize)]
struct GhosttyThemeDefinition {
    id: String,
    name: String,
    background: u32,
    foreground: u32,
    cursor: u32,
    selection_background: u32,
    palette: [u32; 16],
}

impl GhosttyThemeDefinition {
    fn into_theme(self) -> (String, WarpTheme) {
        let foreground = color(self.foreground);
        let luminance = u32::from(foreground.r) * 299
            + u32::from(foreground.g) * 587
            + u32::from(foreground.b) * 114;
        let details = if luminance > 127_000 {
            Details::Darker
        } else {
            Details::Lighter
        };
        let theme = WarpTheme::new(
            Fill::Solid(color(self.background)),
            foreground,
            Fill::Solid(color(self.selection_background)),
            Some(Fill::Solid(color(self.cursor))),
            Some(details),
            TerminalColors::new(
                ansi_colors(&self.palette[..8]),
                ansi_colors(&self.palette[8..]),
            ),
            None,
            Some(self.name),
        );
        (self.id, theme)
    }
}

fn color(rgb: u32) -> ColorU {
    ColorU::from_u32((rgb << 8) | 0xff)
}

fn ansi_color(rgb: u32) -> AnsiColor {
    AnsiColor::from_u32((rgb << 8) | 0xff)
}

fn ansi_colors(colors: &[u32]) -> AnsiColors {
    AnsiColors::new(
        ansi_color(colors[0]),
        ansi_color(colors[1]),
        ansi_color(colors[2]),
        ansi_color(colors[3]),
        ansi_color(colors[4]),
        ansi_color(colors[5]),
        ansi_color(colors[6]),
        ansi_color(colors[7]),
    )
}

pub(super) fn load() -> Result<Vec<(String, WarpTheme)>, serde_json::Error> {
    let catalog: GhosttyThemeCatalog = serde_json::from_str(CATALOG)?;
    debug_assert_eq!(catalog.schema_version, 1);
    Ok(catalog
        .themes
        .into_iter()
        .map(GhosttyThemeDefinition::into_theme)
        .collect())
}

#[cfg(test)]
#[path = "ghostty_themes_tests.rs"]
mod tests;
