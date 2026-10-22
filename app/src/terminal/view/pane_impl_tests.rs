use super::should_render_terminal_pane_header;

#[test]
fn active_pane_indicator_keeps_split_pane_header_independent_of_ai_features() {
    assert!(should_render_terminal_pane_header(
        false, false, true, false, true, false, false,
    ));
    assert!(!should_render_terminal_pane_header(
        false, false, true, false, false, false, false,
    ));
    assert!(should_render_terminal_pane_header(
        false, false, true, true, false, false, false,
    ));
    assert!(!should_render_terminal_pane_header(
        false, false, false, false, true, false, false,
    ));
    assert!(should_render_terminal_pane_header(
        false, false, false, false, false, true, false,
    ));
}

#[test]
fn project_service_discovery_keeps_single_pane_header_visible() {
    assert!(should_render_terminal_pane_header(
        false, false, false, false, false, false, true,
    ));
}

#[cfg(feature = "warpeer")]
#[test]
fn pane_size_fraction_items_render_without_unbounded_flex() {
    use warp_core::ui::appearance::Appearance;
    use warpui::App;
    use warpui::platform::WindowStyle;

    use super::{PaneSizeFraction, TerminalAction, pane_size_fraction_menu_item};
    use crate::menu::{Menu, MenuItem};

    App::test((), |mut app| async move {
        app.add_singleton_model(|_| Appearance::mock());

        let items = PaneSizeFraction::ALL
            .into_iter()
            .map(pane_size_fraction_menu_item)
            .collect();
        let (_, _menu) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
            let mut menu = Menu::<TerminalAction>::new();
            menu.set_items(vec![MenuItem::ItemsRow { items }], ctx);
            menu
        });

        app.update(|_| {});
    });
}
