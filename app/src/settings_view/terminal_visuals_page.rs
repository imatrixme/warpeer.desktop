use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::SettingsSection;
use super::settings_page::{
    Category, MatchData, PageTitle, PageType, SettingsPageMeta, SettingsPageViewHandle,
};
use super::terminal_visuals_controls::{self, TerminalVisualAction};
use crate::product_profile;

const PAGE_TITLE: &str = "Terminal effects";

#[derive(Clone, Debug, PartialEq)]
pub enum TerminalVisualsPageAction {
    Apply(TerminalVisualAction),
}

pub struct TerminalVisualsSettingsPageView {
    page: PageType<Self>,
}

impl TerminalVisualsSettingsPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        Self {
            page: PageType::new_categorized(
                vec![
                    Category::new(
                        "Terminal motion",
                        terminal_visuals_controls::build_motion_widgets(ctx),
                    ),
                    Category::new(
                        "Cursor effects",
                        terminal_visuals_controls::build_cursor_widgets(ctx),
                    ),
                    Category::new(
                        "Terminal material",
                        terminal_visuals_controls::build_material_widgets(ctx),
                    ),
                ],
                Some(PageTitle::new(PAGE_TITLE)),
            ),
        }
    }
}

impl Entity for TerminalVisualsSettingsPageView {
    type Event = ();
}

impl View for TerminalVisualsSettingsPageView {
    fn ui_name() -> &'static str {
        "TerminalVisualsSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        self.page.render(self, app)
    }
}

impl TypedActionView for TerminalVisualsSettingsPageView {
    type Action = TerminalVisualsPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            TerminalVisualsPageAction::Apply(action) => {
                terminal_visuals_controls::handle_action(action, ctx)
            }
        }
    }
}

impl SettingsPageMeta for TerminalVisualsSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::TerminalEffects
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        product_profile::is_terminal_only()
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        self.page.update_filter(query, ctx)
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.page.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.page.clear_highlighted_widget();
    }
}

impl From<ViewHandle<TerminalVisualsSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<TerminalVisualsSettingsPageView>) -> Self {
        SettingsPageViewHandle::TerminalVisuals(view_handle)
    }
}
