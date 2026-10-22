use std::path::PathBuf;

use warpui::elements::Empty;
use warpui::keymap::ContextPredicate;
use warpui::{Action, AppContext, Element, Entity, TypedActionView, View, ViewContext, ViewHandle};

use super::settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle};
use super::{SettingsAction, SettingsSection};

pub struct CodeIndexingPageView;

impl CodeIndexingPageView {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let _ = ctx;
        Self
    }
}

impl Entity for CodeIndexingPageView {
    type Event = CodeIndexingPageEvent;
}

impl View for CodeIndexingPageView {
    fn ui_name() -> &'static str {
        "CodeIndexingPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let _ = app;
        Empty::new().finish()
    }
}

#[derive(Debug, Clone)]
pub enum CodeIndexingPageEvent {
    SignupAnonymousUser,
    OpenLspLogs { log_path: PathBuf },
    OpenProjectRules { rule_paths: Vec<PathBuf> },
}

#[derive(Debug, Clone)]
pub enum CodeIndexingPageAction {}

impl TypedActionView for CodeIndexingPageView {
    type Action = CodeIndexingPageAction;
}

impl SettingsPageMeta for CodeIndexingPageView {
    fn section() -> SettingsSection {
        SettingsSection::CodeIndexing
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        let _ = (query, ctx);
        false.into()
    }

    fn should_render(&self, app: &AppContext) -> bool {
        let _ = app;
        false
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        let _ = widget_id;
    }

    fn clear_highlighted_widget(&mut self) {}
}

impl From<ViewHandle<CodeIndexingPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<CodeIndexingPageView>) -> Self {
        SettingsPageViewHandle::CodeIndexing(view_handle)
    }
}

pub fn init_actions_from_parent_view<T: Action + Clone>(
    app: &mut AppContext,
    context: &ContextPredicate,
    builder: fn(SettingsAction) -> T,
) {
    let _ = (app, context, builder);
}
