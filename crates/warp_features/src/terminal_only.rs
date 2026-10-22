use super::FeatureFlag;

impl FeatureFlag {
    /// Whether this flag may be enabled in the terminal-only (Warpeer) product profile.
    #[inline(always)]
    pub const fn allowed_in_terminal_only(self) -> bool {
        matches!(
            self,
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
}
