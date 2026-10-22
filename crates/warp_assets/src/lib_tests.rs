use rust_embed::RustEmbed;

use super::Assets;

#[test]
#[cfg(feature = "warpeer")]
fn warpeer_excludes_async_assets() {
    assert!(<Assets as RustEmbed>::get("async/jpg/dark_city_bg.jpg").is_none());
    assert!(<Assets as RustEmbed>::get("async/png/onboarding/Welcome.png").is_none());
    assert!(<Assets as RustEmbed>::get("bundled/bootstrap/zsh.sh").is_some());
    assert!(<Assets as RustEmbed>::get("bundled/svg/fingerprint-pattern.svg").is_some());
}
