use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Write};

use semver::{Version, VersionReq};
use sha2::{Digest, Sha256};
use warpeer_plugin_protocol::*;
use zip::write::SimpleFileOptions;

use crate::*;

fn manifest(bundle_hash: &str, assets_hash: &str) -> String {
    format!(
        r#"id = "dev.warpeer.image-preview"
name = "Image Preview"
description = "Preview terminal image resources."
version = "1.0.0"
surfaces = ["terminal-hover"]

[runtime]
protocol = 1
abi = "^1"
engine = "pocket-surface"

[entrypoints]
bundle = "dist/main.pocketjs"
assets = "dist/assets.pak"

[hashes]
"dist/main.pocketjs" = "{bundle_hash}"
"dist/assets.pak" = "{assets_hash}"

[[capabilities]]
capability = "terminal-context"
reason = "Recognize the resource under the terminal pointer."

[[capabilities]]
capability = "resource-read"
reason = "Read the exact local image selected by the host."

[[capabilities]]
capability = "pane-surface"
reason = "Render the image preview surface."

[[settings]]
key = "hover_delay_ms"
title = "Hover delay"
description = "Stable hover delay."
type = "integer"
default = 450
min = 100
max = 2000
unit = "ms"
"#
    )
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn package(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        for (name, bytes) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    output.into_inner()
}

fn valid_package() -> Vec<u8> {
    let bundle = b"globalThis.frame = () => {};";
    let assets = b"pak";
    let manifest = manifest(&sha(bundle), &sha(assets));
    package(&[
        (MANIFEST_PATH, manifest.as_bytes()),
        ("dist/main.pocketjs", bundle),
        ("dist/assets.pak", assets),
    ])
}

fn package_with_manifest(manifest: String) -> Vec<u8> {
    let bundle = b"globalThis.frame = () => {};";
    let assets = b"pak";
    package(&[
        (MANIFEST_PATH, manifest.as_bytes()),
        ("dist/main.pocketjs", bundle),
        ("dist/assets.pak", assets),
    ])
}

fn symlink_package() -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut output);
        writer
            .add_symlink("dist/link", "../escape", SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
    }
    output.into_inner()
}

#[test]
fn package_validation_accepts_declared_hashed_content() {
    let validated = validate_archive(Cursor::new(valid_package())).unwrap();
    assert_eq!(validated.manifest.name, "Image Preview");
    assert_eq!(validated.files.len(), 3);
}

#[test]
fn package_validation_rejects_traversal_and_tampering() {
    let traversal = package(&[("../escape", b"bad")]);
    assert!(matches!(
        validate_archive(Cursor::new(traversal)),
        Err(PackageError::UnsafePath(_))
    ));

    let bundle = b"tampered";
    let assets = b"pak";
    let manifest = manifest(&sha(b"expected"), &sha(assets));
    let tampered = package(&[
        (MANIFEST_PATH, manifest.as_bytes()),
        ("dist/main.pocketjs", bundle),
        ("dist/assets.pak", assets),
    ]);
    assert!(matches!(
        validate_archive(Cursor::new(tampered)),
        Err(PackageError::HashMismatch { .. })
    ));
}

#[test]
fn package_validation_rejects_truncated_incompatible_symlink_and_duplicate_archives() {
    let mut truncated = valid_package();
    truncated.truncate(truncated.len() / 2);
    assert!(matches!(
        validate_archive(Cursor::new(truncated)),
        Err(PackageError::Archive(_))
    ));

    let bundle = b"globalThis.frame = () => {};";
    let assets = b"pak";
    let incompatible_manifest =
        manifest(&sha(bundle), &sha(assets)).replace("protocol = 1", "protocol = 99");
    assert!(matches!(
        validate_archive(Cursor::new(package_with_manifest(incompatible_manifest))),
        Err(PackageError::IncompatibleRuntime { .. })
    ));

    assert!(matches!(
        validate_archive(Cursor::new(symlink_package())),
        Err(PackageError::Symlink { .. })
    ));

    let duplicate = package(&[(MANIFEST_PATH, b"first"), ("./plugin.toml", b"second")]);
    let duplicate_result = validate_archive(Cursor::new(duplicate));
    assert!(
        matches!(
            duplicate_result,
            Err(PackageError::DuplicateEntry { .. } | PackageError::Archive(_))
        ),
        "unexpected duplicate archive result: {duplicate_result:?}"
    );
}

#[test]
fn manager_installs_atomically_and_preserves_state() {
    let root = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginPaths::new(root.path())).unwrap();
    let validated = validate_archive(Cursor::new(valid_package())).unwrap();
    let review = manager.inspect_bundled(validated).unwrap();
    let installed = manager
        .install(
            review,
            InstallApproval {
                capability_expansion: true,
                downgrade: false,
            },
        )
        .unwrap();
    assert!(installed.enabled);
    assert!(installed.package_root(manager.paths()).is_dir());
    assert_eq!(manager.inventory().unwrap().len(), 1);
    assert!(installed.grants.contains_key(&Capability::TerminalContext));
    assert!(installed.grants.contains_key(&Capability::ResourceRead));

    let invalid = manager.update_settings(
        &installed.manifest.id,
        BTreeMap::from([("hover_delay_ms".into(), serde_json::json!(99))]),
    );
    assert!(matches!(invalid, Err(ManagerError::InvalidSetting { .. })));
    let configured = manager
        .update_settings(
            &installed.manifest.id,
            BTreeMap::from([("hover_delay_ms".into(), serde_json::json!(800))]),
        )
        .unwrap();
    assert_eq!(configured.settings_revision, 1);
    assert_eq!(
        configured.settings["hover_delay_ms"],
        serde_json::json!(800)
    );
    let reset = manager.reset_settings(&installed.manifest.id).unwrap();
    assert_eq!(reset.settings["hover_delay_ms"], serde_json::json!(450));

    let disabled = manager.set_enabled(&installed.manifest.id, false).unwrap();
    assert!(!disabled.enabled);
    manager.uninstall(&installed.manifest.id, false).unwrap();
    assert!(manager.inventory().unwrap().is_empty());
}

#[test]
fn manager_requires_capability_expansion_approval_and_retains_data_on_request() {
    let root = tempfile::tempdir().unwrap();
    let manager = PluginManager::new(PluginPaths::new(root.path())).unwrap();
    let first = validate_archive(Cursor::new(valid_package())).unwrap();
    let first = manager.inspect_bundled(first).unwrap();
    let installed = manager.install(first, InstallApproval::default()).unwrap();

    let bundle = b"globalThis.frame = () => {};";
    let assets = b"pak";
    let next_manifest = manifest(&sha(bundle), &sha(assets))
        .replace("version = \"1.0.0\"", "version = \"1.1.0\"")
        + "\n[[capabilities]]\ncapability = \"network-fetch\"\nreason = \"Fetch exact HTTPS images.\"\noptional = true\nscopes = [\"https-image\"]\n";
    let next = validate_archive(Cursor::new(package_with_manifest(next_manifest))).unwrap();
    let review = manager.inspect_bundled(next).unwrap();
    assert_eq!(
        review.expanded_capabilities,
        BTreeSet::from([Capability::NetworkFetch])
    );
    assert!(matches!(
        manager.install(review, InstallApproval::default()),
        Err(ManagerError::CapabilityReviewRequired(_))
    ));

    let data_root = manager.paths().data_root(&installed.manifest.id);
    std::fs::create_dir_all(&data_root).unwrap();
    std::fs::write(data_root.join("retained"), b"state").unwrap();
    manager.uninstall(&installed.manifest.id, true).unwrap();
    assert!(data_root.join("retained").is_file());
}

#[test]
fn expanded_capabilities_require_review() {
    let base = PluginManifest {
        id: PluginId::parse("dev.warpeer.preview").unwrap(),
        name: "Preview".into(),
        description: "Preview resources.".into(),
        version: Version::new(1, 0, 0),
        runtime: RuntimeRequirement {
            protocol: 1,
            abi: VersionReq::parse("^1").unwrap(),
            engine: PluginEngine::PocketSurface,
            target: None,
        },
        entrypoints: Entrypoints {
            bundle: "dist/main.pocketjs".into(),
            assets: "dist/assets.pak".into(),
            icon: None,
        },
        hashes: BTreeMap::new(),
        publisher: None,
        signature: None,
        surfaces: BTreeSet::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        capabilities: vec![CapabilityDeclaration {
            capability: Capability::TerminalContext,
            reason: "Context".into(),
            optional: false,
            scopes: Vec::new(),
        }],
        settings: Vec::new(),
        budgets: ResourceBudgets::default(),
    };
    let mut next = base.clone();
    next.capabilities.push(CapabilityDeclaration {
        capability: Capability::NetworkFetch,
        reason: "Remote images".into(),
        optional: true,
        scopes: vec!["https".into()],
    });
    assert_eq!(
        expanded_capabilities(&base, &next),
        BTreeSet::from([Capability::NetworkFetch])
    );
}

#[test]
fn capability_handles_are_exact_and_revocable() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("image.png");
    std::fs::write(&path, b"image").unwrap();
    let plugin_id = PluginId::parse("dev.warpeer.preview").unwrap();
    let registry = CapabilityRegistry::default();
    registry.register(
        plugin_id.clone(),
        [Capability::ResourceRead],
        BTreeMap::from([(Capability::ResourceRead, CapabilityGrant::default())]),
    );
    let context_id = ContextId::new();
    let revision = Revision(3);
    let handle = registry
        .issue_exact_read_handle(&plugin_id, &path, context_id, revision)
        .unwrap();
    assert_eq!(
        registry
            .read_exact(&plugin_id, handle, context_id, revision, 32)
            .unwrap(),
        b"image"
    );
    registry.revoke(&plugin_id, &Capability::ResourceRead);
    assert!(
        registry
            .read_exact(&plugin_id, handle, context_id, revision, 32)
            .is_err()
    );
}
