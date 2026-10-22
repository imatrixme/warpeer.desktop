use warpeer_plugin_protocol::{Capability, CapabilityDeclaration, PluginId, WorkerState};
use warpeer_plugin_runtime::{DiagnosticEntry, InstalledPlugin, PluginSource};
use warpui::elements::{
    ConstrainedBox, Container, CrossAxisAlignment, Element, Flex, MainAxisSize, MouseStateHandle,
    ParentElement, Shrinkable, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::ui_components::button::ButtonVariant;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{AppContext, SingletonEntity as _, ViewContext};

use super::plugins_page::{PluginsPageAction, PluginsSettingsPageView};
use super::settings_page::{LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item};
use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;
use crate::warpeer_plugin::WarpeerPluginHost;

pub fn build_inventory_widgets(
    inventory: &[InstalledPlugin],
    selected_plugin: Option<&PluginId>,
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>> {
    if inventory.is_empty() {
        vec![Box::new(EmptyPluginWidget::default())
            as Box<dyn SettingsWidget<View = PluginsSettingsPageView>>]
    } else {
        inventory
            .iter()
            .cloned()
            .map(|plugin| {
                let selected = selected_plugin == Some(&plugin.manifest.id);
                Box::new(PluginWidget::new(plugin, selected, ctx))
                    as Box<dyn SettingsWidget<View = PluginsSettingsPageView>>
            })
            .collect()
    }
}

pub fn build_permission_widgets(
    plugin: &InstalledPlugin,
) -> Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>> {
    plugin
        .manifest
        .capabilities
        .iter()
        .filter(|declaration| declaration.optional)
        .cloned()
        .map(|declaration| {
            let granted = plugin.grants.contains_key(&declaration.capability);
            Box::new(PermissionWidget::new(
                plugin.manifest.id.clone(),
                plugin.manifest.name.clone(),
                declaration,
                granted,
            )) as Box<dyn SettingsWidget<View = PluginsSettingsPageView>>
        })
        .collect()
}

#[derive(Default)]
struct EmptyPluginWidget {
    install: MouseStateHandle,
}

impl SettingsWidget for EmptyPluginWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin empty install local package offline"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        render_body_item::<PluginsPageAction>(
            "No plugins installed".into(),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            button(
                appearance,
                "Choose package…",
                self.install.clone(),
                PluginsPageAction::ChoosePackage,
            ),
            Some(
                "Install a local .warpeerplugin package. No account or marketplace is required."
                    .into(),
            ),
        )
    }
}

struct PermissionWidget {
    plugin_id: PluginId,
    plugin_name: String,
    declaration: CapabilityDeclaration,
    granted: bool,
    toggle: SwitchStateHandle,
}

impl PermissionWidget {
    fn new(
        plugin_id: PluginId,
        plugin_name: String,
        declaration: CapabilityDeclaration,
        granted: bool,
    ) -> Self {
        Self {
            plugin_id,
            plugin_name,
            declaration,
            granted,
            toggle: Default::default(),
        }
    }
}

impl SettingsWidget for PermissionWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin permission capability scope access revoke"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let id = self.plugin_id.clone();
        let capability = self.declaration.capability.clone();
        let enabled = !self.granted;
        let control = Container::new(
            appearance
                .ui_builder()
                .switch(self.toggle.clone())
                .check(self.granted)
                .build()
                .on_click(move |ctx, _, _| {
                    ctx.dispatch_typed_action(PluginsPageAction::SetPermission {
                        plugin_id: id.clone(),
                        capability: capability.clone(),
                        enabled,
                    });
                })
                .finish(),
        )
        .with_margin_left(8.)
        .finish();
        let scopes = if self.declaration.scopes.is_empty() {
            String::new()
        } else {
            format!(" Scopes: {}.", self.declaration.scopes.join(", "))
        };
        render_body_item::<PluginsPageAction>(
            format!(
                "{} · {}",
                self.plugin_name,
                capability_label(&self.declaration.capability)
            ),
            None,
            LocalOnlyIconState::Hidden,
            ToggleState::Enabled,
            appearance,
            control,
            Some(format!("{}{}", self.declaration.reason, scopes)),
        )
    }
}

fn capability_label(capability: &Capability) -> &'static str {
    match capability {
        Capability::TerminalContext => "Terminal context",
        Capability::ResourceRead => "Read hovered resource",
        Capability::NetworkFetch => "Fetch exact HTTPS resource",
        Capability::PaneSurface => "Render plugin surface",
        Capability::PaneMetadata => "Pane metadata",
        Capability::SettingsStore => "Plugin settings",
    }
}

pub(super) fn button(
    appearance: &Appearance,
    label: &str,
    state: MouseStateHandle,
    action: PluginsPageAction,
) -> Box<dyn Element> {
    Container::new(
        appearance
            .ui_builder()
            .button(ButtonVariant::Secondary, state)
            .with_text_label(label.to_string())
            .with_style(UiComponentStyles {
                padding: Some(Coords::default().top(5.).bottom(5.).left(10.).right(10.)),
                ..Default::default()
            })
            .build()
            .on_click(move |ctx, _, _| ctx.dispatch_typed_action(action.clone()))
            .finish(),
    )
    .with_margin_left(8.)
    .finish()
}

struct PluginWidget {
    plugin: InstalledPlugin,
    status: &'static str,
    search_terms: String,
    selected: bool,
    details: MouseStateHandle,
}

impl PluginWidget {
    fn new(plugin: InstalledPlugin, selected: bool, app: &AppContext) -> Self {
        let host = WarpeerPluginHost::as_ref(app);
        let status = if plugin.enabled {
            host.worker_state(&plugin.manifest.id)
                .map_or("Idle", worker_state_label)
        } else {
            "Disabled"
        };
        Self {
            search_terms: format!(
                "plugin installed {} {} {}",
                plugin.manifest.name, plugin.manifest.version, status
            ),
            status,
            plugin,
            selected,
            details: Default::default(),
        }
    }
}

impl SettingsWidget for PluginWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        &self.search_terms
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        let theme = appearance.theme();
        let id = self.plugin.manifest.id.clone();
        let source = match &self.plugin.source {
            PluginSource::Bundled => "Bundled",
            PluginSource::Local => "Local",
        };
        let icon = ConstrainedBox::new(
            plugin_icon(&self.plugin)
                .to_warpui_icon(theme.active_ui_text_color())
                .finish(),
        )
        .with_width(16.)
        .with_height(16.)
        .finish();
        let labels = Flex::column()
            .with_child(
                Text::new_inline(
                    self.plugin.manifest.name.clone(),
                    appearance.ui_font_family(),
                    appearance.ui_font_size(),
                )
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.active_ui_text_color().into_solid())
                .finish(),
            )
            .with_child(
                Text::new_inline(
                    format!(
                        "{} · {} · {}",
                        self.plugin.manifest.version, source, self.status
                    ),
                    appearance.ui_font_family(),
                    appearance.ui_font_size() - 1.,
                )
                .with_color(theme.nonactive_ui_text_color().into_solid())
                .finish(),
            )
            .finish();
        let label = Flex::row()
            .with_main_axis_size(MainAxisSize::Max)
            .with_cross_axis_alignment(CrossAxisAlignment::Center)
            .with_spacing(10.)
            .with_child(icon)
            .with_child(Shrinkable::new(1., labels).finish())
            .finish();
        appearance
            .ui_builder()
            .button(
                if self.selected {
                    ButtonVariant::Accent
                } else {
                    ButtonVariant::Text
                },
                self.details.clone(),
            )
            .with_custom_label(label)
            .with_style(UiComponentStyles {
                width: Some(220.),
                padding: Some(Coords::uniform(9.)),
                border_width: Some(0.),
                ..Default::default()
            })
            .build()
            .on_click(move |ctx, _, _| {
                ctx.dispatch_typed_action(PluginsPageAction::SelectPlugin(id.clone()));
            })
            .finish()
    }
}

pub(super) fn plugin_health(
    plugin: &InstalledPlugin,
    state: Option<WorkerState>,
    diagnostic: Option<&DiagnosticEntry>,
) -> String {
    let state = if plugin.enabled {
        state.map_or("Idle", worker_state_label)
    } else {
        "Disabled"
    };
    diagnostic.map_or_else(
        || format!("{state} · worker starts on demand"),
        |entry| format!("{state} · {}", entry.message),
    )
}

fn plugin_icon(plugin: &InstalledPlugin) -> Icon {
    if plugin.manifest.id.as_str() == "dev.warpeer.image-preview" {
        Icon::Image
    } else {
        Icon::PackageCheck
    }
}

pub(super) fn worker_state_label(state: WorkerState) -> &'static str {
    match state {
        WorkerState::Starting => "Starting",
        WorkerState::Ready => "Ready",
        WorkerState::Idle => "Idle",
        WorkerState::Running => "Running",
        WorkerState::Suspended => "Suspended",
        WorkerState::Stopping => "Stopping",
        WorkerState::Stopped => "Stopped",
        WorkerState::Failed => "Failed",
        WorkerState::WatchdogTerminated => "Watchdog terminated",
        WorkerState::Uninstalled => "Uninstalled",
    }
}

#[cfg(feature = "integration_tests")]
pub fn plugin_inventory_widget_id() -> &'static str {
    PluginWidget::static_widget_id()
}

#[cfg(feature = "integration_tests")]
pub fn empty_plugin_widget_id() -> &'static str {
    EmptyPluginWidget::static_widget_id()
}
