#[cfg(feature = "warpeer")]
use super::HeaderToolbarItemKind;
#[cfg(feature = "warpeer")]
use crate::ui_components::icons::Icon;

#[cfg(feature = "warpeer")]
#[test]
fn warpeer_defaults_privacy_at_the_far_right() {
    let items = HeaderToolbarItemKind::default_right();
    assert_eq!(items.last(), Some(&HeaderToolbarItemKind::TabPrivacy));
}

#[cfg(feature = "warpeer")]
#[test]
fn toolbar_editor_lists_warpeer_privacy_item() {
    assert!(HeaderToolbarItemKind::all_items().contains(&HeaderToolbarItemKind::TabPrivacy));
}

#[cfg(feature = "warpeer")]
#[test]
fn warpeer_defaults_include_removable_pane_layouts_on_the_right() {
    let items = HeaderToolbarItemKind::default_right();
    assert!(items.contains(&HeaderToolbarItemKind::PaneLayouts));
    assert_eq!(items.last(), Some(&HeaderToolbarItemKind::TabPrivacy));
}

#[cfg(feature = "warpeer")]
#[test]
fn pane_layouts_uses_the_grid_icon() {
    assert_eq!(HeaderToolbarItemKind::PaneLayouts.icon(), Icon::Grid);
}
