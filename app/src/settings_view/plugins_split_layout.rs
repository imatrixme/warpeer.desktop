use warp_core::ui::theme::color::internal_colors;
use warpeer_plugin_protocol::PluginId;
use warpeer_plugin_runtime::{InstallReview, InstalledPlugin};
use warpui::elements::{
    Align, Border, ClippedScrollStateHandle, ClippedScrollable, ConstrainedBox, Container,
    CrossAxisAlignment, Element, Expanded, Fill, Flex, MainAxisSize, MouseStateHandle,
    ParentElement, SavePosition, ScrollTarget, ScrollToPositionMode, ScrollbarWidth, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::{AppContext, SingletonEntity as _, UpdateView, ViewContext, ViewHandle};

use super::plugins_page::{PluginsPageAction, PluginsSettingsPageView};
use super::settings_page::{
    MatchData, SettingsWidget, render_separator, render_sub_header, search_terms_match,
};
use super::{
    plugins_controls, plugins_custom_surface, plugins_detail_controls, plugins_lifecycle_controls,
    plugins_setting_controls,
};
use crate::appearance::Appearance;
use crate::menu::{Event as MenuEvent, Menu};
use crate::warpeer_plugin::WarpeerPluginHost;

const PLUGIN_LIST_WIDTH: f32 = 248.;
const PLUGIN_LIST_POSITION_ID: &str = "warpeer-plugins-list";
const PLUGIN_DETAIL_POSITION_ID: &str = "warpeer-plugins-detail";

struct PluginSection {
    title: &'static str,
    widgets: Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>>,
}

impl PluginSection {
    fn new(
        title: &'static str,
        widgets: Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>>,
    ) -> Self {
        Self { title, widgets }
    }
}

pub struct PluginsSplitLayout {
    inventory: Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>>,
    sections: Vec<PluginSection>,
    selected_search_terms: String,
    filter_query: String,
    highlighted_widget_id: Option<&'static str>,
    list_scroll: ClippedScrollStateHandle,
    detail_scroll: ClippedScrollStateHandle,
    install: MouseStateHandle,
    actions_menu: ViewHandle<Menu<PluginsPageAction>>,
    actions_menu_open_for: Option<PluginId>,
    actions_menu_button: MouseStateHandle,
}

impl PluginsSplitLayout {
    pub fn new(
        ctx: &mut ViewContext<PluginsSettingsPageView>,
        selected_plugin: Option<&PluginId>,
        pending_install: Option<&InstallReview>,
        pending_remove: Option<&PluginId>,
    ) -> Self {
        let actions_menu = ctx.add_typed_action_view(|_ctx| Menu::<PluginsPageAction>::new());
        ctx.subscribe_to_view(&actions_menu, |view, _, event, ctx| {
            if let MenuEvent::Close { .. } = event {
                view.close_actions_menu();
                ctx.focus_self();
                ctx.notify();
            }
        });
        let mut layout = Self {
            inventory: Vec::new(),
            sections: Vec::new(),
            selected_search_terms: String::new(),
            filter_query: String::new(),
            highlighted_widget_id: None,
            list_scroll: Default::default(),
            detail_scroll: Default::default(),
            install: Default::default(),
            actions_menu,
            actions_menu_open_for: None,
            actions_menu_button: Default::default(),
        };
        layout.rebuild(ctx, selected_plugin, pending_install, pending_remove);
        layout
    }

    pub fn rebuild(
        &mut self,
        ctx: &mut ViewContext<PluginsSettingsPageView>,
        selected_plugin: Option<&PluginId>,
        pending_install: Option<&InstallReview>,
        pending_remove: Option<&PluginId>,
    ) {
        let inventory = WarpeerPluginHost::as_ref(ctx)
            .inventory()
            .unwrap_or_default();
        self.inventory =
            plugins_controls::build_inventory_widgets(&inventory, selected_plugin, ctx);
        self.sections.clear();
        if self.actions_menu_open_for.as_ref() != selected_plugin {
            self.actions_menu_open_for = None;
        }

        if let Some(review) = pending_install {
            self.sections.push(PluginSection::new(
                "Installation review",
                vec![plugins_lifecycle_controls::install_review_widget(review)],
            ));
        }
        if let Some(id) = pending_remove {
            let name = inventory
                .iter()
                .find(|plugin| &plugin.manifest.id == id)
                .map_or_else(|| id.to_string(), |plugin| plugin.manifest.name.clone());
            self.sections.push(PluginSection::new(
                "Remove plugin",
                vec![plugins_lifecycle_controls::remove_review_widget(name)],
            ));
        }

        let Some(plugin) = selected_plugin
            .and_then(|id| inventory.iter().find(|plugin| &plugin.manifest.id == id))
            .cloned()
        else {
            self.selected_search_terms.clear();
            return;
        };
        self.selected_search_terms = format!(
            "{} {} {} {}",
            plugin.manifest.id,
            plugin.manifest.name,
            plugin.manifest.version,
            plugin.manifest.description
        );
        self.build_selected_sections(plugin, ctx);
    }

    fn build_selected_sections(
        &mut self,
        plugin: InstalledPlugin,
        ctx: &mut ViewContext<PluginsSettingsPageView>,
    ) {
        self.sections.push(PluginSection::new(
            "Overview",
            vec![plugins_detail_controls::detail_widget(
                plugin.clone(),
                self.actions_menu.clone(),
                self.actions_menu_button.clone(),
                ctx,
            )],
        ));
        let selected = std::slice::from_ref(&plugin);
        let settings = plugins_setting_controls::build_settings(selected, ctx);
        if !settings.is_empty() {
            self.sections
                .push(PluginSection::new("Configuration", settings));
        }
        let permissions = plugins_controls::build_permission_widgets(&plugin);
        if !permissions.is_empty() {
            self.sections
                .push(PluginSection::new("Permissions", permissions));
        }
        let custom_surfaces = plugins_custom_surface::build_custom_surfaces(selected, ctx);
        if !custom_surfaces.is_empty() {
            self.sections
                .push(PluginSection::new("Plugin interface", custom_surfaces));
        }
        self.sections.push(PluginSection::new(
            "Diagnostics",
            vec![plugins_detail_controls::diagnostics_widget(plugin, ctx)],
        ));
    }

    pub fn toggle_actions_menu(
        &mut self,
        id: &PluginId,
        ctx: &mut ViewContext<PluginsSettingsPageView>,
    ) {
        if self.actions_menu_open_for.as_ref() == Some(id) {
            self.actions_menu_open_for = None;
            ctx.focus_self();
            ctx.notify();
            return;
        }

        let Some(plugin) = WarpeerPluginHost::as_ref(ctx)
            .inventory()
            .unwrap_or_default()
            .into_iter()
            .find(|plugin| &plugin.manifest.id == id)
        else {
            return;
        };
        let items = plugins_detail_controls::action_menu_items(&plugin, ctx);
        ctx.update_view(&self.actions_menu, |menu, ctx| menu.set_items(items, ctx));
        self.actions_menu_open_for = Some(id.clone());
        ctx.focus(&self.actions_menu);
        ctx.notify();
    }

    pub fn actions_menu_is_open(&self, id: &PluginId) -> bool {
        self.actions_menu_open_for.as_ref() == Some(id)
    }

    pub fn close_actions_menu(&mut self) {
        self.actions_menu_open_for = None;
    }

    pub fn update_filter(&mut self, query: &str, app: &AppContext) -> MatchData {
        self.filter_query = query.trim().to_string();
        if self.filter_query.is_empty() {
            return MatchData::Uncounted(true);
        }
        let selected_matches = search_terms_match(&self.selected_search_terms, &self.filter_query);
        let inventory_matches = self
            .inventory
            .iter()
            .filter(|widget| self.widget_matches(widget.as_ref(), app))
            .count();
        let detail_matches = self
            .sections
            .iter()
            .flat_map(|section| &section.widgets)
            .filter(|widget| selected_matches || self.widget_matches(widget.as_ref(), app))
            .count();
        MatchData::Countable(inventory_matches + detail_matches)
    }

    pub fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.highlighted_widget_id = Some(widget_id);
        self.detail_scroll.scroll_to_position(ScrollTarget {
            position_id: widget_id.to_string(),
            mode: ScrollToPositionMode::FullyIntoView,
        });
    }

    pub fn clear_highlighted_widget(&mut self) {
        self.highlighted_widget_id = None;
    }

    pub fn render(&self, view: &PluginsSettingsPageView, app: &AppContext) -> Box<dyn Element> {
        let list = ConstrainedBox::new(self.render_list(view, app))
            .with_width(PLUGIN_LIST_WIDTH)
            .finish();
        Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
            .with_child(list)
            .with_child(Expanded::new(1., self.render_detail(view, app)).finish())
            .finish()
    }

    fn render_list(&self, view: &PluginsSettingsPageView, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let mut rows = Flex::column().with_main_axis_size(MainAxisSize::Min);
        for widget in &self.inventory {
            if self.widget_matches(widget.as_ref(), app) {
                rows.add_child(widget.render_widget(view, false, appearance, app));
            }
        }
        let list = ClippedScrollable::vertical(
            self.list_scroll.clone(),
            Container::new(rows.finish())
                .with_uniform_padding(10.)
                .finish(),
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into(),
            theme.active_ui_detail().into(),
            Fill::None,
        )
        .finish();
        let install = appearance
            .ui_builder()
            .button(ButtonVariant::Secondary, self.install.clone())
            .with_text_label("Install plugin…".into())
            .with_style(UiComponentStyles {
                width: Some(220.),
                padding: Some(Coords::uniform(7.)),
                ..Default::default()
            })
            .build()
            .on_click(|ctx, _, _| {
                ctx.dispatch_typed_action(PluginsPageAction::ChoosePackage);
            })
            .finish();
        let content = Container::new(
            Flex::column()
                .with_child(
                    Container::new(
                        Text::new_inline(
                            "Installed plugins",
                            appearance.ui_font_family(),
                            appearance.ui_font_size(),
                        )
                        .with_style(Properties::default().weight(Weight::Semibold))
                        .with_color(theme.active_ui_text_color().into_solid())
                        .finish(),
                    )
                    .with_uniform_padding(14.)
                    .finish(),
                )
                .with_child(render_separator(appearance))
                .with_child(Expanded::new(1., list).finish())
                .with_child(render_separator(appearance))
                .with_child(Container::new(install).with_uniform_padding(10.).finish())
                .finish(),
        )
        .with_background(internal_colors::fg_overlay_1(theme))
        .with_border(Border::right(1.).with_border_fill(theme.outline()))
        .finish();
        SavePosition::new(content, PLUGIN_LIST_POSITION_ID)
            .for_single_frame()
            .finish()
    }

    fn render_detail(&self, view: &PluginsSettingsPageView, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);
        let theme = appearance.theme();
        let selected_matches = search_terms_match(&self.selected_search_terms, &self.filter_query);
        let mut content = Flex::column().with_main_axis_size(MainAxisSize::Min);
        let mut visible_sections = 0;
        for section in &self.sections {
            let widgets = section
                .widgets
                .iter()
                .filter(|widget| selected_matches || self.widget_matches(widget.as_ref(), app))
                .collect::<Vec<_>>();
            if widgets.is_empty() {
                continue;
            }
            if visible_sections > 0 {
                content.add_child(render_separator(appearance));
            }
            content.add_child(
                Container::new(render_sub_header(appearance, section.title, None))
                    .with_margin_bottom(6.)
                    .finish(),
            );
            for widget in widgets {
                let highlighted = self
                    .highlighted_widget_id
                    .is_some_and(|id| id == widget.widget_id());
                content.add_child(widget.render_widget(view, highlighted, appearance, app));
            }
            visible_sections += 1;
        }
        let body = if visible_sections == 0 {
            self.render_empty_detail(appearance)
        } else {
            Container::new(content.finish())
                .with_uniform_padding(24.)
                .finish()
        };
        let scrollable = ClippedScrollable::vertical(
            self.detail_scroll.clone(),
            body,
            ScrollbarWidth::Auto,
            theme.nonactive_ui_detail().into(),
            theme.active_ui_detail().into(),
            Fill::None,
        )
        .finish();
        SavePosition::new(scrollable, PLUGIN_DETAIL_POSITION_ID)
            .for_single_frame()
            .finish()
    }

    fn render_empty_detail(&self, appearance: &Appearance) -> Box<dyn Element> {
        let theme = appearance.theme();
        Align::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(6.)
                .with_child(
                    Text::new_inline(
                        "Select a plugin",
                        appearance.ui_font_family(),
                        appearance.ui_font_size() + 2.,
                    )
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .with_color(theme.active_ui_text_color().into_solid())
                    .finish(),
                )
                .with_child(
                    Text::new_inline(
                        "Its settings and permissions will appear here.",
                        appearance.ui_font_family(),
                        appearance.ui_font_size(),
                    )
                    .with_color(theme.nonactive_ui_text_color().into_solid())
                    .finish(),
                )
                .finish(),
        )
        .finish()
    }

    fn widget_matches(
        &self,
        widget: &dyn SettingsWidget<View = PluginsSettingsPageView>,
        app: &AppContext,
    ) -> bool {
        widget.should_render(app)
            && search_terms_match(widget.search_terms(), self.filter_query.as_str())
    }
}

#[cfg(feature = "integration_tests")]
pub fn plugin_list_position_id() -> &'static str {
    PLUGIN_LIST_POSITION_ID
}

#[cfg(feature = "integration_tests")]
pub fn plugin_detail_position_id() -> &'static str {
    PLUGIN_DETAIL_POSITION_ID
}
