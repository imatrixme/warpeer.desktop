use std::cell::Cell;

use warp_core::features::FeatureFlag;

use super::ProductProfile;

#[test]
fn terminal_only_profile_keeps_terminal_features() {
    let profile = ProductProfile::TerminalOnly;

    assert!(profile.allows_feature(FeatureFlag::ImeMarkedText));
    assert!(profile.allows_feature(FeatureFlag::KittyKeyboardProtocol));
    assert!(profile.allows_feature(FeatureFlag::VerticalTabs));
    assert!(profile.allows_feature(FeatureFlag::GlobalSearch));
    assert!(profile.allows_feature(FeatureFlag::CodeReviewFind));
}

#[test]
fn terminal_only_profile_enables_cell_filling_box_drawing() {
    let features = ProductProfile::TerminalOnly.default_enabled_features();

    assert!(features.contains(&FeatureFlag::BoxDrawingGlyphs));
    assert!(
        ProductProfile::FullWarp
            .default_enabled_features()
            .is_empty()
    );
}

#[test]
fn terminal_only_profile_rejects_service_features() {
    let profile = ProductProfile::TerminalOnly;

    assert!(!profile.allows_feature(FeatureFlag::AgentMode));
    assert!(!profile.allows_feature(FeatureFlag::AgentManagementView));
    assert!(!profile.allows_feature(FeatureFlag::HOANotifications));
    assert!(!profile.allows_feature(FeatureFlag::McpServer));
    assert!(!profile.allows_feature(FeatureFlag::Autoupdate));
    assert!(!profile.allows_hosted_services());
    assert!(!profile.allows_product_education());
    assert!(!profile.allows_language_server_downloads());
    assert!(!profile.allows_theme_background_images());
    assert!(!profile.allows_focus_panes_on_hover());
    assert!(!profile.allows_ssh_wrapper());
    assert!(ProductProfile::FullWarp.allows_ssh_wrapper());
    assert!(profile.allows_worktree_agent_control_center());
    assert!(ProductProfile::FullWarp.allows_theme_background_images());
    assert!(ProductProfile::FullWarp.allows_focus_panes_on_hover());
    assert!(!ProductProfile::FullWarp.allows_worktree_agent_control_center());
    assert_eq!(profile.display_name(), "Warpeer");
    assert_eq!(ProductProfile::FullWarp.display_name(), "Warp");
}

#[test]
fn terminal_only_profile_filters_full_warp_items() {
    let profile = ProductProfile::TerminalOnly;
    let constructed = Cell::new(false);

    assert!(!profile.is_full_warp());
    assert!(ProductProfile::FullWarp.is_full_warp());

    let items = profile.full_warp_items(|| {
        constructed.set(true);
        ["share"]
    });
    assert!(items.is_empty());
    assert!(!constructed.get());

    let items = ProductProfile::FullWarp.full_warp_items(|| ["share"]);
    assert_eq!(items, ["share"]);
}
