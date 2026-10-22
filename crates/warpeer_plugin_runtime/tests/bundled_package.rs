use std::fs::File;
use std::path::PathBuf;

use warpeer_plugin_protocol::{Capability, CapabilityDeclaration, SurfaceSlot};
use warpeer_plugin_runtime::validate_archive;

#[test]
fn bundled_image_preview_uses_the_public_package_contract() {
    let package = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/image-preview/dist/image-preview.warpeerplugin");
    let validated = validate_archive(File::open(package).unwrap()).unwrap();
    assert_eq!(validated.manifest.id.as_str(), "dev.warpeer.image-preview");
    assert!(
        validated
            .manifest
            .surfaces
            .contains(&SurfaceSlot::TerminalHover)
    );
    assert_eq!(validated.manifest.capabilities.len(), 5);
    for capability in [
        Capability::TerminalContext,
        Capability::ResourceRead,
        Capability::NetworkFetch,
        Capability::PaneSurface,
        Capability::SettingsStore,
    ] {
        assert!(
            validated
                .manifest
                .capabilities
                .iter()
                .any(|declaration| declaration.capability == capability)
        );
    }
    assert_capability(
        &validated.manifest.capabilities,
        Capability::ResourceRead,
        false,
        &["exact-image-file"],
    );
    assert_capability(
        &validated.manifest.capabilities,
        Capability::NetworkFetch,
        true,
        &["https-image"],
    );
}

fn assert_capability(
    declarations: &[CapabilityDeclaration],
    capability: Capability,
    optional: bool,
    scopes: &[&str],
) {
    let declaration = declarations
        .iter()
        .find(|declaration| declaration.capability == capability)
        .unwrap();
    assert_eq!(declaration.optional, optional);
    assert_eq!(
        declaration.scopes,
        scopes
            .iter()
            .map(|scope| (*scope).to_owned())
            .collect::<Vec<_>>()
    );
}
