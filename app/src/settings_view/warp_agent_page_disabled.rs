use warpui::elements::Empty;
use warpui::keymap::ContextPredicate;
use warpui::{Action, AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle};
use super::{SettingsAction, SettingsSection};

pub struct WarpAgentPageView;

impl WarpAgentPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let _ = ctx;
        Self
    }

    pub fn get_modal_content(&self, app: &AppContext) -> Option<Box<dyn Element>> {
        let _ = app;
        None
    }
}

impl View for WarpAgentPageView {
    fn ui_name() -> &'static str {
        "WarpAgentPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

pub enum WarpAgentPageEvent {
    FocusModal,
    SignupAnonymousUser,
    ShowModal,
    HideModal,
}

impl Entity for WarpAgentPageView {
    type Event = WarpAgentPageEvent;
}

#[derive(Debug, Clone, PartialEq)]
pub enum WarpAgentPageAction {}

impl TypedActionView for WarpAgentPageView {
    type Action = WarpAgentPageAction;
}

impl SettingsPageMeta for WarpAgentPageView {
    fn section() -> SettingsSection {
        SettingsSection::WarpAgent
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

impl From<ViewHandle<WarpAgentPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<WarpAgentPageView>) -> Self {
        SettingsPageViewHandle::WarpAgent(view_handle)
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let _ = (app, context, builder);
}

struct CustomModelRoutersWidget;

pub(crate) fn custom_model_routers_widget_id() -> &'static str {
    std::any::type_name::<CustomModelRoutersWidget>()
}
