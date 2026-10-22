use warpeer_plugin_protocol::WorkerState;
use warpeer_plugin_runtime::{InstalledPlugin, PluginSource};
use warpui::elements::{Container, Element, Flex, MouseStateHandle};
use warpui::platform::Cursor;
use warpui::{AppContext, SingletonEntity as _, ViewContext, ViewHandle};

use super::plugins_controls::plugin_health;
use super::plugins_page::{PluginsPageAction, PluginsSettingsPageView};
use super::settings_page::{LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item};
use crate::appearance::Appearance;
use crate::menu::{Menu, MenuItem, MenuItemFields};
use crate::ui_components::icons::Icon;
use crate::ui_components::menu_button::{MenuDirection, icon_button_with_context_menu};
use crate::warpeer_plugin::WarpeerPluginHost;

pub fn detail_widget(
    plugin: InstalledPlugin,
    actions_menu: ViewHandle<Menu<PluginsPageAction>>,
    actions_menu_button: MouseStateHandle,
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Box<dyn SettingsWidget<View = PluginsSettingsPageView>> {
    Box::new(PluginDetailWidget::new(
        plugin,
        actions_menu,
        actions_menu_button,
        ctx,
    ))
}

pub fn diagnostics_widget(
    plugin: InstalledPlugin,
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Box<dyn SettingsWidget<View = PluginsSettingsPageView>> {
    Box::new(PluginDiagnosticsWidget::new(plugin, ctx))
}

struct PluginDetailWidget {
    plugin: InstalledPlugin,
    health: String,
    actions_menu: ViewHandle<Menu<PluginsPageAction>>,
    actions_menu_button: MouseStateHandle,
}

impl PluginDetailWidget {
    fn new(
        plugin: InstalledPlugin,
        actions_menu: ViewHandle<Menu<PluginsPageAction>>,
        actions_menu_button: MouseStateHandle,
        app: &AppContext,
    ) -> Self {
        let host = WarpeerPluginHost::as_ref(app);
        Self {
            health: plugin_health(
                &plugin,
                host.worker_state(&plugin.manifest.id),
                host.diagnostics(&plugin.manifest.id).last(),
            ),
            plugin,
            actions_menu,
            actions_menu_button,
        }
    }
}

pub fn action_menu_items(
    plugin: &InstalledPlugin,
    app: &AppContext,
) -> Vec<MenuItem<PluginsPageAction>> {
    let id = plugin.manifest.id.clone();
    let mut items = vec![
        MenuItemFields::new(if plugin.enabled {
            "Disable plugin"
        } else {
            "Enable plugin"
        })
        .with_on_select_action(PluginsPageAction::ToggleEnabled(id.clone()))
        .into_item(),
        MenuItemFields::new("Update from package…")
            .with_on_select_action(PluginsPageAction::ChoosePackage)
            .into_item(),
        MenuItemFields::new("Reveal in Finder")
            .with_on_select_action(PluginsPageAction::Reveal(id.clone()))
            .into_item(),
    ];

    if !plugin.manifest.settings.is_empty() {
        items.push(
            MenuItemFields::new("Reset settings")
                .with_on_select_action(PluginsPageAction::ResetSettings(id.clone()))
                .into_item(),
        );
    }

    if plugin.enabled {
        items.push(MenuItem::Separator);
        items.push(
            MenuItemFields::new("Reload plugin")
                .with_on_select_action(PluginsPageAction::Reload(id.clone()))
                .into_item(),
        );
        items.push(
            MenuItemFields::new("Force restart")
                .with_on_select_action(PluginsPageAction::ForceRestart(id.clone()))
                .into_item(),
        );
    }

    if matches!(
        WarpeerPluginHost::as_ref(app).worker_state(&id),
        Some(WorkerState::Failed | WorkerState::WatchdogTerminated)
    ) {
        items.push(
            MenuItemFields::new("Retry worker")
                .with_on_select_action(PluginsPageAction::Retry(id.clone()))
                .into_item(),
        );
    }

    if plugin.source == PluginSource::Local {
        items.push(MenuItem::Separator);
        items.push(
            MenuItemFields::new("Remove plugin…")
                .with_override_text_color(Appearance::as_ref(app).theme().ansi_fg_red())
                .with_on_select_action(PluginsPageAction::RequestRemove(id))
                .into_item(),
        );
    }

    items
}

impl SettingsWidget for PluginDetailWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin detail source runtime abi resource health update retry reveal remove"
    }

    fn render(
        &self,
        view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let id = self.plugin.manifest.id.clone();
        let menu_id = id.clone();
        let controls = Container::new(
            icon_button_with_context_menu(
                Icon::DotsVertical,
                move |ctx, _, _| {
                    ctx.dispatch_typed_action(PluginsPageAction::ToggleActionsMenu(
                        menu_id.clone(),
                    ));
                },
                self.actions_menu_button.clone(),
                &self.actions_menu,
                view.actions_menu_is_open(&id),
                MenuDirection::Left,
                Some(Cursor::PointingHand),
                None,
                appearance,
            )
            .finish(),
        )
        .with_margin_left(8.)
        .finish();
        let source = match self.plugin.source {
            PluginSource::Bundled => "Bundled by Warpeer",
            PluginSource::Local => "Local package",
        };
        let budgets = self.plugin.manifest.budgets;
        render_body_item::<PluginsPageAction>(
            self.plugin.manifest.name.clone(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            controls,
            Some(format!(
                "{} · {} · Runtime protocol {} · ABI {} · Surfaces {}/{} px · Worker memory {} MB. {}",
                self.plugin.manifest.description,
                source,
                self.plugin.manifest.runtime.protocol,
                self.plugin.manifest.runtime.abi,
                budgets.active_surfaces,
                budgets.max_surface_pixels,
                budgets.memory_bytes / (1024 * 1024),
                self.health,
            )),
        )
    }
}

struct PluginDiagnosticsWidget {
    summary: String,
}

impl PluginDiagnosticsWidget {
    fn new(plugin: InstalledPlugin, app: &AppContext) -> Self {
        let diagnostics = WarpeerPluginHost::as_ref(app).diagnostics(&plugin.manifest.id);
        let summary = if diagnostics.is_empty() {
            "No diagnostic events. The worker starts only when a declared trigger is used.".into()
        } else {
            diagnostics
                .iter()
                .rev()
                .take(5)
                .map(|entry| format!("{:?}: {}", entry.level, entry.message))
                .collect::<Vec<_>>()
                .join(" · ")
        };
        Self { summary }
    }
}

impl SettingsWidget for PluginDiagnosticsWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin diagnostics log crash failure watchdog resource limit"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<PluginsPageAction>(
            "Recent runtime events".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            Flex::row().finish(),
            Some(self.summary.clone()),
        )
    }
}

#[cfg(feature = "integration_tests")]
pub fn plugin_detail_widget_id() -> &'static str {
    PluginDetailWidget::static_widget_id()
}

#[cfg(feature = "integration_tests")]
pub fn plugin_diagnostics_widget_id() -> &'static str {
    PluginDiagnosticsWidget::static_widget_id()
}
