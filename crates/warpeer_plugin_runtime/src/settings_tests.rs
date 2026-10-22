use std::collections::{BTreeMap, BTreeSet};

use semver::{Version, VersionReq};
use warpeer_plugin_protocol::{
    Entrypoints, PluginEngine, PluginId, PluginManifest, ResourceBudgets, RuntimeRequirement,
    SettingDeclaration, SettingValueDeclaration,
};

use crate::manager::ManagerError;
use crate::settings::{normalize_settings, reconcile_settings};

fn manifest() -> PluginManifest {
    PluginManifest {
        id: PluginId::parse("dev.warpeer.settings-test").unwrap(),
        name: "Settings Test".into(),
        description: "Settings contract fixture.".into(),
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
        capabilities: Vec::new(),
        settings: vec![
            SettingDeclaration {
                key: "enabled".into(),
                title: "Enabled".into(),
                description: "Toggle the fixture.".into(),
                value: SettingValueDeclaration::Boolean { default: true },
            },
            SettingDeclaration {
                key: "delay".into(),
                title: "Delay".into(),
                description: "Delay in milliseconds.".into(),
                value: SettingValueDeclaration::Integer {
                    default: 200,
                    min: 100,
                    max: 500,
                    step: 50,
                    unit: Some("ms".into()),
                },
            },
        ],
        budgets: ResourceBudgets::default(),
    }
}

#[test]
fn whole_object_settings_fill_defaults_and_reject_unknown_or_off_step_values() {
    let manifest = manifest();
    let normalized = normalize_settings(
        &manifest,
        BTreeMap::from([("delay".into(), serde_json::json!(350))]),
    )
    .unwrap();
    assert_eq!(normalized["enabled"], serde_json::json!(true));
    assert_eq!(normalized["delay"], serde_json::json!(350));

    for invalid in [
        BTreeMap::from([("unknown".into(), serde_json::json!(true))]),
        BTreeMap::from([("delay".into(), serde_json::json!(325))]),
    ] {
        assert!(matches!(
            normalize_settings(&manifest, invalid),
            Err(ManagerError::InvalidSetting { .. })
        ));
    }
}

#[test]
fn stored_settings_reconcile_without_migration_rules() {
    let manifest = manifest();
    let reconciled = reconcile_settings(
        &manifest,
        &BTreeMap::from([
            ("delay".into(), serde_json::json!(400)),
            ("removed".into(), serde_json::json!("stale")),
        ]),
    );
    assert_eq!(reconciled.len(), 2);
    assert_eq!(reconciled["enabled"], serde_json::json!(true));
    assert_eq!(reconciled["delay"], serde_json::json!(400));
}
