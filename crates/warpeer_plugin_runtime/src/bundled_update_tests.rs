use std::io::{Cursor, Write};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::{PluginManager, PluginPaths, validate_archive};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn package(version: &str) -> Vec<u8> {
    package_with_hover_default(version, 450)
}

fn package_with_hover_default(version: &str, hover_default: u64) -> Vec<u8> {
    let bundle = format!("globalThis.version = '{version}';");
    let assets = b"pak";
    let manifest = format!(
        r#"id = "dev.warpeer.image-preview"
name = "Image Preview"
description = "Preview terminal image resources."
version = "{version}"
surfaces = ["terminal-hover"]

[runtime]
protocol = 1
abi = "^1"
engine = "pocket-surface"

[entrypoints]
bundle = "dist/main.pocketjs"
assets = "dist/assets.pak"

[hashes]
"dist/main.pocketjs" = "{}"
"dist/assets.pak" = "{}"

[[capabilities]]
capability = "terminal-context"
reason = "Recognize the resource under the pointer."

[[capabilities]]
capability = "pane-surface"
reason = "Present a bounded terminal hover surface."

[[capabilities]]
capability = "settings-store"
reason = "Persist plugin configuration."

[[settings]]
key = "hover_delay_ms"
title = "Hover delay"
description = "Stable hover delay."
type = "integer"
default = {hover_default}
min = 100
max = 2000
step = 50
unit = "ms"
"#,
        sha(bundle.as_bytes()),
        sha(assets)
    );
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        for (name, bytes) in [
            ("plugin.toml", manifest.as_bytes()),
            ("dist/main.pocketjs", bundle.as_bytes()),
            ("dist/assets.pak", assets.as_slice()),
        ] {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    output.into_inner()
}

#[test]
fn newer_bundled_package_replaces_runtime_and_preserves_user_state() {
    let root = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginPaths::new(root.path())).unwrap();
    let first = validate_archive(Cursor::new(package("1.0.0"))).unwrap();
    let first = manager.install_bundled_if_newer(first).unwrap().unwrap();
    manager
        .update_settings(
            &first.manifest.id,
            [("hover_delay_ms".into(), serde_json::json!(800))].into(),
        )
        .unwrap();
    manager.set_enabled(&first.manifest.id, false).unwrap();

    let update_bytes = package("1.0.1");
    let update = validate_archive(Cursor::new(update_bytes.clone())).unwrap();
    let installed = manager.install_bundled_if_newer(update).unwrap().unwrap();
    assert_eq!(installed.manifest.version.to_string(), "1.0.1");
    assert!(!installed.enabled);
    assert_eq!(installed.settings["hover_delay_ms"], serde_json::json!(800));

    let same = validate_archive(Cursor::new(update_bytes)).unwrap();
    assert!(manager.install_bundled_if_newer(same).unwrap().is_none());
}

#[test]
fn newer_bundled_package_adopts_new_defaults_until_settings_are_customized() {
    let root = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginPaths::new(root.path())).unwrap();
    let first = validate_archive(Cursor::new(package("1.0.0"))).unwrap();
    let first = manager.install_bundled_if_newer(first).unwrap().unwrap();
    assert_eq!(first.settings_revision, 0);
    assert_eq!(first.settings["hover_delay_ms"], serde_json::json!(450));

    let update = validate_archive(Cursor::new(package_with_hover_default("1.0.1", 1500))).unwrap();
    let installed = manager.install_bundled_if_newer(update).unwrap().unwrap();
    assert_eq!(installed.settings_revision, 0);
    assert_eq!(
        installed.settings["hover_delay_ms"],
        serde_json::json!(1500)
    );
}
