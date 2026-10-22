use std::collections::BTreeMap;

use warpeer_plugin_protocol::{PluginManifest, SettingValueDeclaration};

use crate::manager::ManagerError;

pub(crate) fn default_settings(manifest: &PluginManifest) -> BTreeMap<String, serde_json::Value> {
    manifest
        .settings
        .iter()
        .map(|setting| (setting.key.clone(), default_value(&setting.value)))
        .collect()
}

pub(crate) fn normalize_settings(
    manifest: &PluginManifest,
    submitted: BTreeMap<String, serde_json::Value>,
) -> Result<BTreeMap<String, serde_json::Value>, ManagerError> {
    let mut normalized = default_settings(manifest);
    for (key, value) in submitted {
        validate_setting(manifest, &key, &value)?;
        normalized.insert(key, value);
    }
    Ok(normalized)
}

pub(crate) fn reconcile_settings(
    manifest: &PluginManifest,
    stored: &BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    let mut reconciled = default_settings(manifest);
    for (key, value) in stored {
        if validate_setting(manifest, key, value).is_ok() {
            reconciled.insert(key.clone(), value.clone());
        }
    }
    reconciled
}

fn default_value(declaration: &SettingValueDeclaration) -> serde_json::Value {
    match declaration {
        SettingValueDeclaration::Boolean { default } => serde_json::Value::Bool(*default),
        SettingValueDeclaration::Integer { default, .. } => serde_json::Value::from(*default),
        SettingValueDeclaration::String { default, .. } => {
            serde_json::Value::String(default.clone())
        }
    }
}

fn validate_setting(
    manifest: &PluginManifest,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), ManagerError> {
    let declaration = manifest
        .settings
        .iter()
        .find(|setting| setting.key == key)
        .ok_or_else(|| invalid_setting(key, "setting is not declared by the plugin"))?;
    let valid = match &declaration.value {
        SettingValueDeclaration::Boolean { .. } => value.is_boolean(),
        SettingValueDeclaration::Integer { min, max, step, .. } => {
            value.as_i64().is_some_and(|candidate| {
                candidate >= *min && candidate <= *max && (candidate - *min) % *step == 0
            })
        }
        SettingValueDeclaration::String { choices, .. } => value.as_str().is_some_and(|value| {
            choices.is_empty() || choices.iter().any(|choice| choice == value)
        }),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_setting(
            key,
            "value does not match the declared type, bounds, step, or choices",
        ))
    }
}

fn invalid_setting(key: &str, detail: &str) -> ManagerError {
    ManagerError::InvalidSetting {
        key: key.to_string(),
        detail: detail.into(),
    }
}
