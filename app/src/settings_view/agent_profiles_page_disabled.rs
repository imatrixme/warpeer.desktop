use warpui::elements::Empty;
use warpui::keymap::ContextPredicate;
use warpui::{Action, AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle};
use super::{SettingsAction, SettingsSection};
use crate::ai::execution_profiles::ExecutionProfileId;

pub struct AgentProfilesPageView;

impl AgentProfilesPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let _ = ctx;
        Self
    }
}

impl View for AgentProfilesPageView {
    fn ui_name() -> &'static str {
        "AgentProfilesPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

pub enum AgentProfilesPageEvent {
    FocusModal,
    OpenMCPServerCollection,
    OpenExecutionProfileEditor(ExecutionProfileId),
}

impl Entity for AgentProfilesPageView {
    type Event = AgentProfilesPageEvent;
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentProfilesPageAction {}

impl TypedActionView for AgentProfilesPageView {
    type Action = AgentProfilesPageAction;
}

impl SettingsPageMeta for AgentProfilesPageView {
    fn section() -> SettingsSection {
        SettingsSection::AgentProfiles
    }

    fn should_render(&self, app: &AppContext) -> bool {
        let _ = app;
        false
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        let _ = (query, ctx);
        false.into()
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        let _ = widget_id;
    }

    fn clear_highlighted_widget(&mut self) {}
}

impl From<ViewHandle<AgentProfilesPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<AgentProfilesPageView>) -> Self {
        SettingsPageViewHandle::AgentProfiles(view_handle)
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let _ = (app, context, builder);
}
