use settings::macros::define_settings_group;
use settings::{SupportedPlatforms, SyncToCloud};

pub(crate) const fn is_service_discovery_available() -> bool {
    false
}

define_settings_group!(ServiceDiscoverySettings, settings: [
    enabled: ServiceDiscoveryEnabled {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.enabled",
        description: "Whether the project service discovery control panel is available.",
    },
    open_on_hover: ServiceDiscoveryOpenOnHover {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.open_on_hover",
        description: "Whether hovering the pane header button opens project service discovery.",
    },
    show_cache_usage: ServiceDiscoveryShowCacheUsage {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.show_cache_usage",
        description: "Whether common dependency and build cache sizes are collected and shown.",
    },
    show_runtime_details: ServiceDiscoveryShowRuntimeDetails {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.show_runtime_details",
        description: "Whether related processes and listening ports are collected and shown.",
    },
    enable_server_controls: ServiceDiscoveryEnableServerControls {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.enable_server_controls",
        description: "Whether static and package script service controls are available.",
    },
    enable_docker_controls: ServiceDiscoveryEnableDockerControls {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "project_service_discovery.enable_docker_controls",
        description: "Whether Docker and Compose discovery and controls are available.",
    },
]);

#[cfg(test)]
#[path = "service_discovery_settings_tests.rs"]
mod tests;
