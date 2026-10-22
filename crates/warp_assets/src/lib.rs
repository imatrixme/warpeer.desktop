use std::borrow::Cow;

use anyhow::{Result, anyhow};
use rust_embed::RustEmbed;
use warpui_core::AssetProvider;

// RustEmbed does not invalidate an existing Cargo artifact when a file is first added under a
// directory glob, so track this late-added privacy icon explicitly for incremental builds.
const _: &[u8] = include_bytes!("../../../app/assets/bundled/svg/fingerprint-pattern.svg");

#[derive(Clone, Copy, RustEmbed)]
#[folder = "../../app/assets"]
#[include = "bundled/**"] // Should be kept in sync with BUNDLED_ASSETS_DIR.
#[include = "async/**"] // Should be kept in sync with ASYNC_ASSETS_DIR.
#[cfg_attr(target_family = "wasm", exclude = "async/**")]
// Excludes take precedence.
// Standalone CLI builds (the `oz` tarball) are headless and never render the
// onboarding/theme imagery in `async/`, so we exclude those bytes from the
// embedded asset set to keep the CLI binary small — mirroring the carve-out
// already applied for the WASM target above.
#[cfg_attr(feature = "standalone", exclude = "async/**")]
// Warpeer uses its committed palette catalog and does not expose any surface
// backed by asynchronously fetched product imagery.
#[cfg_attr(feature = "warpeer", exclude = "async/**")]
pub struct Assets;

impl AssetProvider for Assets {
    fn get(&self, path: &str) -> Result<Cow<'_, [u8]>> {
        <Assets as RustEmbed>::get(path)
            .map(|f| f.data)
            .ok_or_else(|| anyhow!("no asset exists at path {}", path))
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
