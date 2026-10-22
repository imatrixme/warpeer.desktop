use std::path::PathBuf;

use warpeer_plugin_runtime::validate_package;

#[test]
fn bundled_image_preview_is_a_valid_public_plugin_package() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/image-preview/dist/image-preview.warpeerplugin");
    let validated = validate_package(&package).unwrap();
    assert_eq!(validated.manifest.id.as_str(), "dev.warpeer.image-preview");
    assert!(
        validated
            .manifest
            .surfaces
            .contains(&warpeer_plugin_protocol::SurfaceSlot::TerminalHover)
    );
}
