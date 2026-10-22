use std::time::Duration;

use serde::{Deserialize, Serialize};
use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
    settings_value::SettingsValue,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename_all = "snake_case")]
pub enum TabRelockDelay {
    Immediately,
    Seconds30,
    #[default]
    Minute1,
    Minutes5,
    Minutes15,
    Never,
}

impl TabRelockDelay {
    pub fn duration(self) -> Option<Duration> {
        match self {
            Self::Immediately => Some(Duration::ZERO),
            Self::Seconds30 => Some(Duration::from_secs(30)),
            Self::Minute1 => Some(Duration::from_secs(60)),
            Self::Minutes5 => Some(Duration::from_secs(5 * 60)),
            Self::Minutes15 => Some(Duration::from_secs(15 * 60)),
            Self::Never => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Immediately => "Immediately",
            Self::Seconds30 => "30 seconds",
            Self::Minute1 => "1 minute",
            Self::Minutes5 => "5 minutes",
            Self::Minutes15 => "15 minutes",
            Self::Never => "Never",
        }
    }
}

define_settings_group!(TabPrivacySettings, settings: [
    relock_delay: TabPrivacyRelockDelay {
        type: TabRelockDelay,
        default: TabRelockDelay::Minute1,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "security.tab_privacy.relock_after_leaving",
        description: "How long an unlocked protected tab remains visible after the user leaves it.",
    },
    biometric_quick_unlock: TabPrivacyBiometricQuickUnlock {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::MAC,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "security.tab_privacy.biometric_quick_unlock",
        description: "Whether platform biometrics are available as a shortcut alongside the privacy password.",
    },
    lock_on_system_inactivity: TabPrivacyLockOnSystemInactivity {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "security.tab_privacy.lock_on_system_inactivity",
        description: "Whether protected tabs lock immediately when the system session locks or sleeps.",
    },
    toolbar_item_migrated: TabPrivacyToolbarItemMigrated {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: true,
        storage_key: "WarpeerTabPrivacyToolbarItemMigrated",
        description: "Whether the privacy toolbar item migration has been applied.",
    },
    toolbar_item_rightmost_migrated: TabPrivacyToolbarItemRightmostMigrated {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: true,
        storage_key: "WarpeerTabPrivacyToolbarItemRightmostMigrated",
        description: "Whether the privacy toolbar item has been migrated to the right edge.",
    },
]);
