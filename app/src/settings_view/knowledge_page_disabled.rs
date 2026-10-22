use warpui::elements::Empty;
use warpui::keymap::ContextPredicate;
use warpui::{Action, AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle};
use super::{SettingsAction, SettingsSection};

pub struct KnowledgePageView;

impl KnowledgePageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let _ = ctx;
        Self
    }
}

impl Entity for KnowledgePageView {
    type Event = KnowledgePageEvent;
}

impl View for KnowledgePageView {
    fn ui_name() -> &'static str {
        "KnowledgePage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

pub enum KnowledgePageEvent {
    OpenAIFactCollection,
}

#[derive(Debug, Clone)]
pub enum KnowledgePageAction {}

impl TypedActionView for KnowledgePageView {
    type Action = KnowledgePageAction;
}

impl SettingsPageMeta for KnowledgePageView {
    fn section() -> SettingsSection {
        SettingsSection::Knowledge
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

impl From<ViewHandle<KnowledgePageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<KnowledgePageView>) -> Self {
        SettingsPageViewHandle::Knowledge(view_handle)
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let _ = (app, context, builder);
}
