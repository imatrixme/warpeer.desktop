use instant::Duration;
use settings::{PrivatePreferences, PublicPreferences, Setting, SettingsManager};
use settings_value::SettingsValue;
use warp_core::features::FeatureFlag;
use warp_core::settings::macros::define_settings_group;
use warp_core::settings::{SupportedPlatforms, SyncToCloud};
use warp_core::user_preferences::GetUserPreferences as _;
use warpui::SingletonEntity;
use warpui_extras::user_preferences;

#[cfg(feature = "terminal_visuals")]
use super::apply_terminal_only_defaults;
use super::{
    SETTINGS_FILE_MIGRATION_COMPLETE_KEY, migrate_native_settings_to_settings_file,
    needs_settings_file_migration_for_path,
};
#[cfg(feature = "terminal_visuals")]
use crate::settings::CodeSettings;
#[cfg(feature = "terminal_visuals")]
use crate::settings::{
    AppEditorSettings, CursorDisplayType, CursorMotion, EraseMotion, FontSettings, KeypressMotion,
    PaneSettings, ReducedMotionPolicy, TerminalMaterial, TerminalVisualSettings, ThemeSettings,
};
use crate::terminal::session_settings::{NotificationsMode, NotificationsSettings};
#[cfg(feature = "terminal_visuals")]
use crate::terminal::settings::{SpacingMode, TerminalSettings};
#[cfg(feature = "terminal_visuals")]
use crate::themes::theme::ThemeKind;
#[cfg(feature = "terminal_visuals")]
use crate::window_settings::WindowSettings;
#[cfg(feature = "terminal_visuals")]
use crate::workspace::tab_settings::{
    TabSettings, VerticalTabsCompactSubtitle, VerticalTabsDisplayGranularity,
    VerticalTabsPrimaryInfo, VerticalTabsViewMode,
};

// A minimal settings group with one public and one private setting, used to
// verify that migration only copies public settings.
define_settings_group!(MigrationTestSettings, settings: [
    public_setting: PublicSetting {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "migration_test.public_setting",
    },
    public_string_setting: PublicStringSetting {
        type: String,
        default: String::new(),
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "migration_test.public_string_setting",
    },
    private_setting: PrivateSetting {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
]);

/// Registers separate InMemoryPreferences singletons for public and private
/// stores, then adds a SettingsManager and the test settings group.
fn init_test_app(ctx: &mut warpui::AppContext) {
    ctx.add_singleton_model(move |_| {
        PublicPreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    ctx.add_singleton_model(move |_| -> PrivatePreferences {
        PrivatePreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    ctx.add_singleton_model(|_| SettingsManager::default());
    MigrationTestSettings::register(ctx);
}

#[cfg(feature = "terminal_visuals")]
fn init_product_default_test_app(ctx: &mut warpui::AppContext) {
    ctx.add_singleton_model(move |_| {
        PublicPreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    ctx.add_singleton_model(move |_| -> PrivatePreferences {
        PrivatePreferences::new(Box::<user_preferences::in_memory::InMemoryPreferences>::default())
    });
    ctx.add_singleton_model(|_| SettingsManager::default());
    AppEditorSettings::register(ctx);
    CodeSettings::register(ctx);
    FontSettings::register(ctx);
    PaneSettings::register(ctx);
    TabSettings::register(ctx);
    TerminalSettings::register(ctx);
    TerminalVisualSettings::register(ctx);
    ThemeSettings::register(ctx);
    WindowSettings::register(ctx);
}

#[cfg(feature = "terminal_visuals")]
#[test]
fn terminal_only_defaults_match_the_warpeer_profile() {
    warpui::App::test((), |mut app| async move {
        app.update(init_product_default_test_app);
        app.update(apply_terminal_only_defaults);

        app.read(|ctx| {
            let terminal = TerminalSettings::as_ref(ctx);
            assert_eq!(*terminal.spacing_mode, SpacingMode::Normal);

            let tabs = TabSettings::as_ref(ctx);
            assert!(*tabs.use_vertical_tabs);
            assert!(*tabs.hide_title_bar_search_bar_in_vertical_tabs);
            assert!(!*tabs.show_vertical_tab_panel_in_restored_windows);
            assert_eq!(
                tabs.vertical_tabs_display_granularity,
                VerticalTabsDisplayGranularity::Tabs
            );
            assert_eq!(tabs.vertical_tabs_view_mode, VerticalTabsViewMode::Compact);
            assert_eq!(
                tabs.vertical_tabs_primary_info,
                VerticalTabsPrimaryInfo::Command
            );
            assert_eq!(
                tabs.vertical_tabs_compact_subtitle,
                VerticalTabsCompactSubtitle::Branch
            );
            assert!(!*tabs.preserve_active_tab_color);

            let font = FontSettings::as_ref(ctx);
            assert_eq!(*font.notebook_font_size, 14.);
            assert_eq!(*font.monospace_font_size, 11.);
            assert_eq!(font.monospace_font_name.as_str(), "Maple Mono NF");
            assert_eq!(font.ai_font_name.as_str(), "Maple Mono NF");
            assert!(!*font.match_ai_font_to_terminal_font);

            assert_eq!(
                ThemeSettings::as_ref(ctx).theme_kind.value(),
                &ThemeKind::Ghostty("Shades Of Purple".to_owned())
            );

            let visuals = TerminalVisualSettings::as_ref(ctx);
            assert!(*visuals.enabled);
            assert_eq!(*visuals.duration_ms, 220.);
            assert_eq!(*visuals.particle_spread, 1.);
            assert_eq!(*visuals.keypress_motion, KeypressMotion::Rise);
            assert_eq!(*visuals.cursor_motion, CursorMotion::Ease);
            assert_eq!(*visuals.material, TerminalMaterial::SmokedGlass);
            assert_eq!(*visuals.reduced_motion, ReducedMotionPolicy::Never);
            assert_eq!(*visuals.erase_motion, EraseMotion::Shatter);

            assert_eq!(
                *AppEditorSettings::as_ref(ctx).cursor_display_type,
                CursorDisplayType::Block
            );
            let panes = PaneSettings::as_ref(ctx);
            assert!(*panes.show_active_pane_indicator);
            assert!(*panes.should_dim_inactive_panes);
            assert!(!*panes.focus_panes_on_hover);
            let window = WindowSettings::as_ref(ctx);
            assert_eq!(*window.background_opacity, 85);
            assert_eq!(*window.background_blur_radius, 24);
            assert!(*CodeSettings::as_ref(ctx).show_global_search);
        });
    });
}

#[cfg(feature = "terminal_visuals")]
#[test]
fn terminal_only_defaults_preserve_explicit_user_values() {
    warpui::App::test((), |mut app| async move {
        app.update(init_product_default_test_app);
        app.update(|ctx| {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings.use_vertical_tabs.set_value(false, ctx).unwrap();
            });
            FontSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings.monospace_font_size.set_value(16., ctx).unwrap();
            });
            ThemeSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings.theme_kind.set_value(ThemeKind::Dark, ctx).unwrap();
            });
            TerminalVisualSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .erase_motion
                    .set_value(EraseMotion::Flame, ctx)
                    .unwrap();
            });
            PaneSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings
                    .show_active_pane_indicator
                    .set_value(false, ctx)
                    .unwrap();
            });
            WindowSettings::handle(ctx).update(ctx, |settings, ctx| {
                settings.background_opacity.set_value(100, ctx).unwrap();
            });
        });

        app.update(apply_terminal_only_defaults);

        app.read(|ctx| {
            assert!(!*TabSettings::as_ref(ctx).use_vertical_tabs);
            assert_eq!(*FontSettings::as_ref(ctx).monospace_font_size, 16.);
            assert_eq!(
                ThemeSettings::as_ref(ctx).theme_kind.value(),
                &ThemeKind::Dark
            );
            assert_eq!(
                *TerminalVisualSettings::as_ref(ctx).erase_motion,
                EraseMotion::Flame
            );
            assert!(!*PaneSettings::as_ref(ctx).show_active_pane_indicator);
            assert_eq!(*WindowSettings::as_ref(ctx).background_opacity, 100);
        });
    });
}

#[test]
fn test_migration_copies_public_settings_from_native_store() {
    warpui::App::test((), |mut app| async move {
        // Enable the settings file so `preferences_for_setting` routes
        // public setting writes to the Model singleton (not the private store).
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);

        app.update(init_test_app);

        // Seed the native (private) store with values for both settings.
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            native
                .write_value("PublicSetting", "true".to_owned())
                .unwrap();
            native
                .write_value("PrivateSetting", "true".to_owned())
                .unwrap();
        });

        // Before migration, in-memory values should still be defaults (the
        // public store is empty and registration read from there).
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(!*settings.public_setting.value());
            assert!(!*settings.private_setting.value());
        });

        // Run the migration.
        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // The public setting should now reflect the native store value.
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(
                *settings.public_setting.value(),
                "public setting should have been migrated from native store"
            );
        });

        // The public store should now contain the migrated value.
        app.read(|ctx| {
            let public = PublicSetting::preferences_for_setting(ctx);
            let stored = public
                .read_value_with_hierarchy(PublicSetting::storage_key(), PublicSetting::hierarchy())
                .unwrap();
            assert_eq!(stored, Some("true".to_owned()));
        });
        // The private setting should NOT have been touched by migration
        // (it's private, so migration skips it). The in-memory value stays
        // at default because register() read from the public store (empty).
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(
                !*settings.private_setting.value(),
                "private setting should not be affected by migration"
            );
        });
    });
}

#[test]
fn test_migration_writes_marker_to_native_store() {
    warpui::App::test((), |mut app| async move {
        app.update(init_test_app);

        // No marker before migration.
        app.read(|ctx| {
            let marker = ctx
                .private_user_preferences()
                .read_value(SETTINGS_FILE_MIGRATION_COMPLETE_KEY)
                .unwrap();
            assert!(marker.is_none());
        });

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // Marker should now be present.
        app.read(|ctx| {
            let marker = ctx
                .private_user_preferences()
                .read_value(SETTINGS_FILE_MIGRATION_COMPLETE_KEY)
                .unwrap();
            assert!(marker.is_some(), "migration marker should be written");
        });
    });
}

#[test]
fn test_migration_skips_settings_absent_from_native_store() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);
        app.update(init_test_app);

        // Don't seed anything in the native store — all settings are absent.

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // Settings should remain at defaults.
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(!*settings.public_setting.value());
            assert_eq!(settings.public_string_setting.value().as_str(), "");
        });

        // The public store should have nothing written.
        app.read(|ctx| {
            let public = PublicSetting::preferences_for_setting(ctx);
            assert!(
                public
                    .read_value_with_hierarchy(
                        PublicSetting::storage_key(),
                        PublicSetting::hierarchy(),
                    )
                    .unwrap()
                    .is_none()
            );
            assert!(
                public
                    .read_value_with_hierarchy(
                        PublicStringSetting::storage_key(),
                        PublicStringSetting::hierarchy(),
                    )
                    .unwrap()
                    .is_none()
            );
        });
    });
}

#[test]
fn test_migration_handles_string_setting() {
    warpui::App::test((), |mut app| async move {
        app.update(init_test_app);

        // Seed a JSON-encoded string value in the native store.
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            native
                .write_value("PublicStringSetting", "\"Fira Code\"".to_owned())
                .unwrap();
        });

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert_eq!(
                settings.public_string_setting.value().as_str(),
                "Fira Code",
                "string setting should have been migrated"
            );
        });
    });
}

#[test]
fn test_migration_does_not_rerun_when_marker_present() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file_path = temp_dir.path().join("settings.toml");

        app.update(init_test_app);

        // Seed the native store with a public setting.
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            native
                .write_value("PublicSetting", "true".to_owned())
                .unwrap();
        });

        // Before migration, the guard should allow migration.
        app.read(|ctx| {
            assert!(
                needs_settings_file_migration_for_path(ctx, &settings_file_path),
                "migration should be needed before first run"
            );
        });

        // Run migration.
        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // After migration, the marker should prevent re-migration.
        app.read(|ctx| {
            assert!(
                !needs_settings_file_migration_for_path(ctx, &settings_file_path),
                "migration should not be needed after marker is written"
            );
        });
    });
}

#[test]
fn test_migration_not_needed_when_settings_file_exists() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_file_path = temp_dir.path().join("settings.toml");
        std::fs::write(&settings_file_path, "").unwrap();

        app.update(init_test_app);

        app.read(|ctx| {
            assert!(
                !needs_settings_file_migration_for_path(ctx, &settings_file_path),
                "migration should not be needed when settings.toml exists"
            );
        });
    });
}

#[test]
fn test_migration_with_multiple_setting_types() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);

        app.update(init_test_app);

        // Seed the native store with values for all three settings.
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            native
                .write_value("PublicSetting", "true".to_owned())
                .unwrap();
            native
                .write_value("PublicStringSetting", "\"Custom Value\"".to_owned())
                .unwrap();
            native
                .write_value("PrivateSetting", "true".to_owned())
                .unwrap();
        });

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // Both public settings should have been migrated.
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(
                *settings.public_setting.value(),
                "public bool should have been migrated"
            );
            assert_eq!(
                settings.public_string_setting.value().as_str(),
                "Custom Value",
                "public string should have been migrated"
            );
        });

        // Public store should contain the migrated bool.
        app.read(|ctx| {
            let public = PublicSetting::preferences_for_setting(ctx);
            assert_eq!(
                public
                    .read_value_with_hierarchy(
                        PublicSetting::storage_key(),
                        PublicSetting::hierarchy(),
                    )
                    .unwrap(),
                Some("true".to_owned())
            );
        });

        // Private setting should NOT have been migrated — in-memory
        // value stays at default because register() read from the
        // public store (which was empty for this key).
        app.read(|ctx| {
            let settings = MigrationTestSettings::as_ref(ctx);
            assert!(
                !*settings.private_setting.value(),
                "private setting should not be affected by migration"
            );
        });

        // The private setting should NOT be in the public store.
        app.read(|ctx| {
            let public = PublicSetting::preferences_for_setting(ctx);
            assert!(
                public
                    .read_value_with_hierarchy(
                        PrivateSetting::storage_key(),
                        PrivateSetting::hierarchy(),
                    )
                    .unwrap()
                    .is_none(),
                "private setting should not appear in public store"
            );
        });
    });
}

// ---------------------------------------------------------------------------
// Tests for serde ↔ file-format mismatch during migration
// ---------------------------------------------------------------------------
//
// NotificationsSettings has #[serde(default)] and contains fields whose serde
// and SettingsValue file formats differ:
//   - NotificationsMode: serde uses PascalCase ("Enabled"), file uses snake_case ("enabled")
//   - Duration: serde uses {"secs":N,"nanos":N}, file uses a plain integer
//
// The migration reads serde-format values from the native store and feeds them
// through update_setting_with_storage_key, which tries from_file_value first.
// If from_file_value silently defaults fields (due to #[serde(default)]), the
// serde fallback is never reached and values are lost.

mod notifications_migration {
    use settings::{PrivatePreferences, PublicPreferences, SettingsManager};
    use warp_core::settings::macros::define_settings_group;
    use warp_core::settings::{SupportedPlatforms, SyncToCloud};
    use warpui_extras::user_preferences;

    use crate::terminal::session_settings::NotificationsSettings;

    define_settings_group!(NotificationsMigrationTestSettings, settings: [
        notifications: MigrationTestNotifications {
            type: NotificationsSettings,
            default: NotificationsSettings::default(),
            supported_platforms: SupportedPlatforms::ALL,
            sync_to_cloud: SyncToCloud::Never,
            surface: settings::SettingSurfaces::GUI,
            private: false,
            toml_path: "migration_test.notifications",
            max_table_depth: 1,
        },
    ]);

    pub fn init_notifications_migration_test_app(ctx: &mut warpui::AppContext) {
        ctx.add_singleton_model(move |_| {
            PublicPreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        ctx.add_singleton_model(move |_| -> PrivatePreferences {
            PrivatePreferences::new(
                Box::<user_preferences::in_memory::InMemoryPreferences>::default(),
            )
        });
        ctx.add_singleton_model(|_| SettingsManager::default());
        NotificationsMigrationTestSettings::register(ctx);
    }
}
use notifications_migration::{
    NotificationsMigrationTestSettings, init_notifications_migration_test_app,
};

// -- from_file_value unit tests: these demonstrate the derive-level bug ------

#[test]
fn test_notifications_from_file_value_rejects_serde_format_enum() {
    // serde serializes NotificationsMode::Enabled as "Enabled" (PascalCase),
    // but from_file_value expects "enabled" (snake_case). When the field is
    // present but unparsable, from_file_value should return None — not
    // silently fall back to the #[serde(default)] value (Unset).
    let serde_json_value = serde_json::to_value(NotificationsSettings {
        mode: NotificationsMode::Enabled,
        ..NotificationsSettings::default()
    })
    .unwrap();

    let result = NotificationsSettings::from_file_value(&serde_json_value);
    assert!(
        result.is_none(),
        "from_file_value should reject serde-format enum values, but got: {result:?}"
    );
}

#[test]
fn test_notifications_from_file_value_rejects_serde_format_duration() {
    // serde serializes Duration as {"secs": N, "nanos": N}, but
    // Duration::from_file_value expects a plain integer. Use file-format
    // for mode ("unset") so that the failure is isolated to the Duration field.
    let json = serde_json::json!({
        "mode": "unset",
        "is_long_running_enabled": true,
        "long_running_threshold": {"secs": 60, "nanos": 0},
        "is_password_prompt_enabled": true,
        "is_agent_task_completed_enabled": true,
        "is_needs_attention_enabled": true,
        "play_notification_sound": true,
    });

    let result = NotificationsSettings::from_file_value(&json);
    assert!(
        result.is_none(),
        "from_file_value should reject serde-format Duration, but got: {result:?}"
    );
}

// -- Migration integration tests: these demonstrate end-to-end data loss -----

#[test]
fn test_migration_preserves_notifications_mode() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);

        app.update(init_notifications_migration_test_app);

        // Seed the native store with serde-serialized NotificationsSettings
        // where mode is Enabled. In serde format: {"mode":"Enabled",...}.
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            let serde_value = serde_json::to_string(&NotificationsSettings {
                mode: NotificationsMode::Enabled,
                ..NotificationsSettings::default()
            })
            .unwrap();
            native
                .write_value("MigrationTestNotifications", serde_value)
                .unwrap();
        });

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        // Mode should be preserved as Enabled, not silently defaulted to Unset.
        app.read(|ctx| {
            let settings = NotificationsMigrationTestSettings::as_ref(ctx);
            assert_eq!(
                settings.notifications.value().mode,
                NotificationsMode::Enabled,
                "NotificationsMode should be preserved during migration"
            );
        });
    });
}

#[test]
fn test_migration_preserves_custom_long_running_threshold() {
    warpui::App::test((), |mut app| async move {
        let _guard = FeatureFlag::SettingsFile.override_enabled(true);

        app.update(init_notifications_migration_test_app);

        // Seed with a non-default threshold (60s instead of default 30s).
        // serde serializes Duration as {"secs":60,"nanos":0}, which differs
        // from the file format (plain integer 60).
        let custom_threshold = Duration::from_secs(60);
        app.update(|ctx| {
            let native = ctx.private_user_preferences();
            let serde_value = serde_json::to_string(&NotificationsSettings {
                long_running_threshold: custom_threshold,
                ..NotificationsSettings::default()
            })
            .unwrap();
            native
                .write_value("MigrationTestNotifications", serde_value)
                .unwrap();
        });

        app.update(|ctx| {
            migrate_native_settings_to_settings_file(ctx);
        });

        app.read(|ctx| {
            let settings = NotificationsMigrationTestSettings::as_ref(ctx);
            assert_eq!(
                settings.notifications.value().long_running_threshold,
                custom_threshold,
                "custom long_running_threshold should be preserved during migration"
            );
        });
    });
}
