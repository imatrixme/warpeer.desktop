use std::collections::BTreeSet;

use warpeer_plugin_protocol::{PluginId, SettingDeclaration, SettingValueDeclaration};
use warpeer_plugin_runtime::InstalledPlugin;
use warpui::elements::Element;
use warpui::ui_components::components::{Coords, UiComponent, UiComponentStyles};
use warpui::ui_components::slider::SliderStateHandle;
use warpui::ui_components::switch::SwitchStateHandle;
use warpui::{AppContext, ViewContext, ViewHandle};

use super::plugins_page::{PluginsPageAction, PluginsSettingsPageView};
use super::settings_page::{
    LocalOnlyIconState, SettingsWidget, ToggleState, render_body_item, render_dropdown_item,
};
use crate::appearance::Appearance;
use crate::view_components::{Dropdown, DropdownItem};

pub fn build_settings(
    inventory: &[InstalledPlugin],
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Vec<Box<dyn SettingsWidget<View = PluginsSettingsPageView>>> {
    inventory
        .iter()
        .flat_map(|plugin| {
            plugin
                .manifest
                .settings
                .iter()
                .map(|declaration| setting_widget(plugin, declaration, ctx))
                .collect::<Vec<_>>()
        })
        .collect()
}

enum SettingControl {
    Toggle(SwitchStateHandle),
    Integer {
        state: SliderStateHandle,
        min: i64,
        max: i64,
        step: i64,
        unit: Option<String>,
    },
    Dropdown(ViewHandle<Dropdown<PluginsPageAction>>),
}

struct PluginSettingWidget {
    plugin_name: String,
    plugin_id: PluginId,
    declaration: SettingDeclaration,
    current: serde_json::Value,
    control: SettingControl,
}

impl SettingsWidget for PluginSettingWidget {
    type View = PluginsSettingsPageView;

    fn search_terms(&self) -> &str {
        "plugin configuration setting image preview trigger hover click menu width height remote cache"
    }

    fn render(
        &self,
        _view: &Self::View,
        appearance: &Appearance,
        _app: &AppContext,
    ) -> Box<dyn Element> {
        match &self.control {
            SettingControl::Toggle(state) => {
                let plugin_id = self.plugin_id.clone();
                let key = self.declaration.key.clone();
                let value = !self.current.as_bool().unwrap_or(false);
                render_body_item::<PluginsPageAction>(
                    format!("{} - {}", self.plugin_name, self.declaration.title),
                    None,
                    LocalOnlyIconState::Hidden,
                    ToggleState::Enabled,
                    appearance,
                    appearance
                        .ui_builder()
                        .switch(state.clone())
                        .check(!value)
                        .build()
                        .on_click(move |ctx, _, _| {
                            ctx.dispatch_typed_action(PluginsPageAction::SetSetting {
                                plugin_id: plugin_id.clone(),
                                key: key.clone(),
                                value: serde_json::Value::Bool(value),
                            });
                        })
                        .finish(),
                    Some(self.declaration.description.clone()),
                )
            }
            SettingControl::Dropdown(dropdown) => render_dropdown_item(
                appearance,
                &format!("{} - {}", self.plugin_name, self.declaration.title),
                Some(&self.declaration.description),
                None,
                LocalOnlyIconState::Hidden,
                None,
                dropdown,
            ),
            SettingControl::Integer {
                state,
                min,
                max,
                step,
                unit,
            } => {
                let plugin_id = self.plugin_id.clone();
                let key = self.declaration.key.clone();
                let current = self.current.as_i64().unwrap_or(*min);
                let label = unit
                    .as_ref()
                    .map_or_else(|| current.to_string(), |unit| format!("{current} {unit}"));
                let min = *min;
                let max = *max;
                let step = *step;
                render_body_item::<PluginsPageAction>(
                    format!("{} - {}: {label}", self.plugin_name, self.declaration.title),
                    None,
                    LocalOnlyIconState::Hidden,
                    ToggleState::Enabled,
                    appearance,
                    appearance
                        .ui_builder()
                        .slider(state.clone())
                        .with_range(min as f32..max as f32)
                        .with_default_value(current as f32)
                        .with_style(UiComponentStyles {
                            width: Some(210.),
                            margin: Some(Coords::default().top(3.).bottom(3.)),
                            ..Default::default()
                        })
                        .on_change(move |ctx, _, value| {
                            let stepped = min
                                + (((value.round() as i64 - min) as f64 / step as f64).round()
                                    as i64)
                                    * step;
                            ctx.dispatch_typed_action(PluginsPageAction::SetSetting {
                                plugin_id: plugin_id.clone(),
                                key: key.clone(),
                                value: serde_json::Value::from(stepped.clamp(min, max)),
                            });
                        })
                        .build()
                        .finish(),
                    Some(self.declaration.description.clone()),
                )
            }
        }
    }
}

fn setting_widget(
    plugin: &InstalledPlugin,
    declaration: &SettingDeclaration,
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> Box<dyn SettingsWidget<View = PluginsSettingsPageView>> {
    let current = plugin
        .settings
        .get(&declaration.key)
        .cloned()
        .unwrap_or_else(|| default_value(&declaration.value));
    let control = match &declaration.value {
        SettingValueDeclaration::Boolean { .. } => SettingControl::Toggle(Default::default()),
        SettingValueDeclaration::Integer {
            min,
            max,
            step,
            unit,
            ..
        } => SettingControl::Integer {
            state: Default::default(),
            min: *min,
            max: *max,
            step: *step,
            unit: unit.clone(),
        },
        value @ SettingValueDeclaration::String { .. } => SettingControl::Dropdown(
            setting_dropdown(&plugin.manifest.id, &declaration.key, value, &current, ctx),
        ),
    };
    Box::new(PluginSettingWidget {
        plugin_name: plugin.manifest.name.clone(),
        plugin_id: plugin.manifest.id.clone(),
        declaration: declaration.clone(),
        current,
        control,
    })
}

fn setting_dropdown(
    plugin_id: &PluginId,
    key: &str,
    declaration: &SettingValueDeclaration,
    current: &serde_json::Value,
    ctx: &mut ViewContext<PluginsSettingsPageView>,
) -> ViewHandle<Dropdown<PluginsPageAction>> {
    let choices = setting_choices(declaration, current);
    let selected = PluginsPageAction::SetSetting {
        plugin_id: plugin_id.clone(),
        key: key.to_string(),
        value: current.clone(),
    };
    let plugin_id = plugin_id.clone();
    let key = key.to_string();
    ctx.add_typed_action_view(move |ctx| {
        let mut dropdown = Dropdown::new(ctx);
        dropdown.set_top_bar_max_width(210.);
        dropdown.set_menu_width(210., ctx);
        dropdown.add_items(
            choices
                .into_iter()
                .map(|(label, value)| {
                    DropdownItem::new(
                        label,
                        PluginsPageAction::SetSetting {
                            plugin_id: plugin_id.clone(),
                            key: key.clone(),
                            value,
                        },
                    )
                })
                .collect(),
            ctx,
        );
        dropdown.set_selected_by_action(selected, ctx);
        dropdown
    })
}

fn setting_choices(
    declaration: &SettingValueDeclaration,
    current: &serde_json::Value,
) -> Vec<(String, serde_json::Value)> {
    match declaration {
        SettingValueDeclaration::String {
            default, choices, ..
        } => {
            let mut values = BTreeSet::from([default.clone()]);
            values.extend(choices.iter().cloned());
            if let Some(value) = current.as_str() {
                values.insert(value.to_string());
            }
            values
                .into_iter()
                .map(|value| (value.clone(), serde_json::Value::String(value)))
                .collect()
        }
        SettingValueDeclaration::Boolean { default } => {
            vec![(default.to_string(), serde_json::Value::Bool(*default))]
        }
        SettingValueDeclaration::Integer { .. } => Vec::new(),
    }
}

fn default_value(declaration: &SettingValueDeclaration) -> serde_json::Value {
    match declaration {
        SettingValueDeclaration::Boolean { default } => serde_json::Value::Bool(*default),
        SettingValueDeclaration::Integer { default, .. } => serde_json::Value::from(*default),
        SettingValueDeclaration::String { default, .. } => {
            serde_json::Value::String(default.clone())
        }
    }
}
