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
#[schemars(
    description = "The default ordering for agents inside each worktree group.",
    rename_all = "snake_case"
)]
pub enum WorktreeAgentSortKey {
    #[default]
    Auto,
    Status,
    LastActive,
    TokenUsage,
    ContextRemaining,
    SessionName,
}

impl WorktreeAgentSortKey {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Status => "Status",
            Self::LastActive => "Last active",
            Self::TokenUsage => "Token usage",
            Self::ContextRemaining => "Context remaining",
            Self::SessionName => "Session name",
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Auto => Self::Status,
            Self::Status => Self::LastActive,
            Self::LastActive => Self::TokenUsage,
            Self::TokenUsage => Self::ContextRemaining,
            Self::ContextRemaining => Self::SessionName,
            Self::SessionName => Self::Auto,
        }
    }
}

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
#[schemars(
    description = "How a pane attached to the hovered agent is identified.",
    rename_all = "snake_case"
)]
pub enum WorktreeAgentPaneLinkEffect {
    Off,
    Highlight,
    #[default]
    Pulse,
}

impl WorktreeAgentPaneLinkEffect {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Highlight => "Title bar highlight",
            Self::Pulse => "Animated connector",
        }
    }
}

define_settings_group!(WorktreeAgentSettings, settings: [
    show_in_pane_header: WorktreeAgentShowInPaneHeader {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "worktrees.agents.show_in_pane_header",
        description: "Whether active worktree agents are shown in terminal pane headers.",
    },
    reuse_current_pane: WorktreeAgentReuseCurrentPane {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "worktrees.agents.reuse_current_pane",
        description: "Whether agent activation reuses the current pane when no exact pane route exists.",
    },
    pane_link_effect: WorktreeAgentPaneLinkEffectSetting {
        type: WorktreeAgentPaneLinkEffect,
        default: WorktreeAgentPaneLinkEffect::Pulse,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "worktrees.agents.pane_link_effect",
        description: "How the pane attached to the hovered agent is identified.",
    },
    default_sort: WorktreeAgentDefaultSort {
        type: WorktreeAgentSortKey,
        default: WorktreeAgentSortKey::Auto,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "worktrees.agents.default_sort",
        description: "The default ordering for agent sessions inside each worktree group.",
    },
    default_sort_descending: WorktreeAgentDefaultSortDescending {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        sync_to_cloud: SyncToCloud::Never,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "worktrees.agents.default_sort_descending",
        description: "Whether non-automatic worktree agent sorting uses descending order.",
    },
]);
