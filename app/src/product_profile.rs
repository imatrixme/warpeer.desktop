use std::sync::OnceLock;

use warp_core::features::FeatureFlag;

use crate::service_composition::ServiceComposition;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ProductProfile {
    #[default]
    FullWarp,
    TerminalOnly,
}

impl ProductProfile {
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::FullWarp => "Warp",
            Self::TerminalOnly => "Warpeer",
        }
    }

    pub(crate) fn is_full_warp(self) -> bool {
        self == Self::FullWarp
    }

    #[cfg(test)]
    pub(crate) fn allows_hosted_services(self) -> bool {
        self.is_full_warp()
    }

    #[cfg(test)]
    pub(crate) fn allows_product_education(self) -> bool {
        self.is_full_warp()
    }

    pub(crate) fn allows_language_server_downloads(self) -> bool {
        self.is_full_warp()
    }

    pub(crate) fn allows_theme_background_images(self) -> bool {
        self.is_full_warp()
    }

    pub(crate) fn allows_focus_panes_on_hover(self) -> bool {
        self.is_full_warp()
    }

    #[cfg(any(feature = "warpeer", test))]
    pub(crate) fn allows_worktree_agent_control_center(self) -> bool {
        self == Self::TerminalOnly
    }

    pub(crate) fn allows_feature(self, flag: FeatureFlag) -> bool {
        if self == Self::FullWarp {
            return true;
        }

        matches!(
            flag,
            FeatureFlag::ThinStrokes
                | FeatureFlag::Ligatures
                | FeatureFlag::CommandCorrectionKey
                | FeatureFlag::CommandCorrectionsHistoryRule
                | FeatureFlag::ClassicCompletions
                | FeatureFlag::ForceClassicCompletions
                | FeatureFlag::RemoveAutosuggestionDuringTabCompletions
                | FeatureFlag::ResizeFix
                | FeatureFlag::DefaultWaterfallMode
                | FeatureFlag::SelectablePrompt
                | FeatureFlag::RectSelection
                | FeatureFlag::AlacrittySettingsImport
                | FeatureFlag::ShellSelector
                | FeatureFlag::LazySceneBuilding
                | FeatureFlag::FullScreenZenMode
                | FeatureFlag::MinimalistUI
                | FeatureFlag::NativeShellCompletions
                | FeatureFlag::DragTabsToWindows
                | FeatureFlag::OscHyperlinks
                | FeatureFlag::ImeMarkedText
                | FeatureFlag::ITermImages
                | FeatureFlag::ValidateAutosuggestions
                | FeatureFlag::ClearAutosuggestionOnEscape
                | FeatureFlag::KittyImages
                | FeatureFlag::UndoClosedPanes
                | FeatureFlag::UIZoom
                | FeatureFlag::AsyncFind
                | FeatureFlag::ConfigurableToolbar
                | FeatureFlag::NewTabStyling
                | FeatureFlag::KittyKeyboardProtocol
                | FeatureFlag::DirectoryTabColors
                | FeatureFlag::TabConfigs
                | FeatureFlag::VerticalTabs
                | FeatureFlag::VerticalTabsSummaryMode
                | FeatureFlag::GroupedTabs
                | FeatureFlag::PinnedTabs
                | FeatureFlag::TerminalLifecycleRecovery
                | FeatureFlag::BoxDrawingGlyphs
                | FeatureFlag::SettingsFile
                | FeatureFlag::GlobalSearch
                | FeatureFlag::CodeReviewSaveChanges
                | FeatureFlag::CodeReviewFind
                | FeatureFlag::CodeReviewScrollPreservation
                | FeatureFlag::GitOperationsInCodeReview
        )
    }

    pub(crate) fn default_enabled_features(self) -> &'static [FeatureFlag] {
        match self {
            Self::FullWarp => &[],
            Self::TerminalOnly => &[FeatureFlag::BoxDrawingGlyphs],
        }
    }

    pub(crate) fn full_warp_items<T, I>(self, items: impl FnOnce() -> I) -> Vec<T>
    where
        I: IntoIterator<Item = T>,
    {
        if self.is_full_warp() {
            items().into_iter().collect()
        } else {
            Vec::new()
        }
    }
}

static ACTIVE_PROFILE: OnceLock<ProductProfile> = OnceLock::new();

pub(crate) fn activate_terminal_only() -> Result<(), ProductProfile> {
    ACTIVE_PROFILE.set(ProductProfile::TerminalOnly)
}

pub(crate) fn active() -> ProductProfile {
    ACTIVE_PROFILE.get().copied().unwrap_or_default()
}

pub(crate) fn is_terminal_only() -> bool {
    active() == ProductProfile::TerminalOnly
}

pub(crate) fn is_full_warp() -> bool {
    active().is_full_warp()
}

pub(crate) fn display_name() -> &'static str {
    active().display_name()
}

pub(crate) fn allows_hosted_services() -> bool {
    ServiceComposition::active().allows_hosted_services()
}

pub(crate) fn allows_product_education() -> bool {
    ServiceComposition::active().allows_product_education()
}

pub(crate) fn allows_language_server_downloads() -> bool {
    active().allows_language_server_downloads()
}

pub(crate) fn allows_theme_background_images() -> bool {
    active().allows_theme_background_images()
}

pub(crate) fn allows_focus_panes_on_hover() -> bool {
    active().allows_focus_panes_on_hover()
}

#[cfg(feature = "warpeer")]
pub(crate) fn allows_worktree_agent_control_center() -> bool {
    active().allows_worktree_agent_control_center()
}

pub(crate) fn full_warp_items<T, I>(items: impl FnOnce() -> I) -> Vec<T>
where
    I: IntoIterator<Item = T>,
{
    active().full_warp_items(items)
}

pub(crate) fn allows_feature(flag: FeatureFlag) -> bool {
    active().allows_feature(flag)
}

pub(crate) fn default_enabled_features() -> &'static [FeatureFlag] {
    active().default_enabled_features()
}

#[cfg(test)]
#[path = "product_profile_tests.rs"]
mod tests;
