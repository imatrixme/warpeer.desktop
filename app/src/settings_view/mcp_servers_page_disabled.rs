use warpui::elements::Empty;
use warpui::{AppContext, Element, Entity, TypedActionView, View, ViewContext};

use super::SettingsSection;
use super::mcp_servers::ServerCardItemId;
use super::settings_page::{MatchData, SettingsPageMeta};

#[derive(Debug, Default, Copy, Clone)]
pub enum MCPServersSettingsPage {
    #[default]
    List,
    Edit {
        item_id: Option<ServerCardItemId>,
    },
}

#[derive(Debug, Clone)]
pub enum MCPServersSettingsPageEvent {
    ShowModal,
    HideModal,
}

pub struct MCPServersSettingsPageView;

impl MCPServersSettingsPageView {
    pub fn new(_ctx: &mut ViewContext<Self>) -> Self {
        Self
    }

    pub fn update_page(&mut self, _page: MCPServersSettingsPage, _ctx: &mut ViewContext<Self>) {}

    pub fn autoinstall_from_gallery(&mut self, _title: &str, _ctx: &mut ViewContext<Self>) {}

    pub fn get_modal_content(&self, _app: &AppContext) -> Option<Box<dyn Element>> {
        None
    }
}

impl Entity for MCPServersSettingsPageView {
    type Event = MCPServersSettingsPageEvent;
}

impl View for MCPServersSettingsPageView {
    fn ui_name() -> &'static str {
        "MCPServersSettingsPageView"
    }

    fn render(&self, _app: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

impl TypedActionView for MCPServersSettingsPageView {
    type Action = ();
}

impl SettingsPageMeta for MCPServersSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::AgentMCPServers
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        false
    }

    fn update_filter(&mut self, _query: &str, _ctx: &mut ViewContext<Self>) -> MatchData {
        false.into()
    }

    fn scroll_to_widget(&mut self, _widget_id: &'static str) {}

    fn clear_highlighted_widget(&mut self) {}
}
