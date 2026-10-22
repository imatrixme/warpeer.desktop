use warpui_core::AppContext;
use warpui_core::assets::asset_cache::AssetSource;
use warpui_core::units::IntoPixels;

use crate::render::layout::TextLayout;
use crate::render::model::{BlockSpacing, ImageBlockConfig};

pub fn mermaid_asset_source(_source: &str) -> AssetSource {
    AssetSource::Bundled {
        path: "__disabled__/mermaid.svg",
    }
}

pub fn mermaid_diagram_layout(
    source: &str,
    layout: &TextLayout,
    spacing: BlockSpacing,
    _app: &AppContext,
) -> (AssetSource, ImageBlockConfig) {
    let asset_source = mermaid_asset_source(source);
    let width = layout.max_width() - spacing.x_axis_offset();
    let height = layout.rich_text_styles().base_line_height() * 2.0.into_pixels();
    (
        asset_source,
        ImageBlockConfig {
            width,
            height,
            spacing,
        },
    )
}
