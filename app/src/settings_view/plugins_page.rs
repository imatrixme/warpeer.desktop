use std::collections::BTreeMap;
use std::path::PathBuf;

use warpeer_plugin_protocol::{Capability, PluginId};
use warpeer_plugin_runtime::{InstallApproval, InstallReview};
use warpui::platform::FilePickerConfiguration;
use warpui::{
    AppContext, Entity, SingletonEntity as _, TypedActionView, View, ViewContext, ViewHandle,
};

use super::SettingsSection;
use super::plugins_split_layout::PluginsSplitLayout;
use super::settings_page::{MatchData, SettingsPageMeta, SettingsPageViewHandle};
use crate::view_components::DismissibleToast;
use crate::warpeer_plugin::{PluginRestartMode, WarpeerPluginHost};
use crate::workspace::ToastStack;

#[cfg(feature = "integration_tests")]
const PAGE_TITLE: &str = "Plugins";

#[derive(Clone, Debug, PartialEq)]
pub enum PluginsPageAction {
    ChoosePackage,
    PackagePicked(Result<PathBuf, String>),
    ConfirmInstall,
    CancelInstall,
    SelectPlugin(PluginId),
    ToggleActionsMenu(PluginId),
    ToggleEnabled(PluginId),
    Reload(PluginId),
    ForceRestart(PluginId),
    Retry(PluginId),
    SetPermission {
        plugin_id: PluginId,
        capability: Capability,
        enabled: bool,
    },
    SetSetting {
        plugin_id: PluginId,
        key: String,
        value: serde_json::Value,
    },
    Reveal(PluginId),
    ResetSettings(PluginId),
    RequestRemove(PluginId),
    ConfirmRemove {
        retain_data: bool,
    },
    CancelRemove,
}

pub struct PluginsSettingsPageView {
    layout: PluginsSplitLayout,
    selected_plugin: Option<PluginId>,
    pending_install: Option<InstallReview>,
    pending_remove: Option<PluginId>,
}

impl PluginsSettingsPageView {
    #[cfg(feature = "integration_tests")]
    pub(crate) fn integration_page_title(&self) -> &'static str {
        PAGE_TITLE
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn integration_selected_plugin(&self) -> Option<&PluginId> {
        self.selected_plugin.as_ref()
    }

    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        ctx.observe(&WarpeerPluginHost::handle(ctx), |_, _, ctx| ctx.notify());
        let selected_plugin = WarpeerPluginHost::as_ref(ctx)
            .inventory()
            .ok()
            .and_then(|plugins| plugins.into_iter().next())
            .map(|plugin| plugin.manifest.id);
        let layout = PluginsSplitLayout::new(ctx, selected_plugin.as_ref(), None, None);
        Self {
            layout,
            selected_plugin,
            pending_install: None,
            pending_remove: None,
        }
    }

    fn rebuild(&mut self, ctx: &mut ViewContext<Self>) {
        let inventory = WarpeerPluginHost::as_ref(ctx)
            .inventory()
            .unwrap_or_default();
        if self.selected_plugin.as_ref().is_none_or(|selected| {
            !inventory
                .iter()
                .any(|plugin| &plugin.manifest.id == selected)
        }) {
            self.selected_plugin = inventory.first().map(|plugin| plugin.manifest.id.clone());
        }
        self.layout.rebuild(
            ctx,
            self.selected_plugin.as_ref(),
            self.pending_install.as_ref(),
            self.pending_remove.as_ref(),
        );
        ctx.notify();
    }

    fn show_toast(&self, message: String, success: bool, ctx: &mut ViewContext<Self>) {
        let toast = if success {
            DismissibleToast::success(message)
        } else {
            DismissibleToast::error(message)
        };
        let window_id = ctx.window_id();
        ToastStack::handle(ctx).update(ctx, |stack, ctx| {
            stack.add_ephemeral_toast(toast, window_id, ctx);
        });
    }

    fn choose_package(&mut self, ctx: &mut ViewContext<Self>) {
        let window_id = ctx.window_id();
        let view_id = ctx.view_id();
        ctx.open_file_picker(
            move |result, ctx| {
                let result = result.map_err(|error| error.to_string()).and_then(|paths| {
                    paths
                        .into_iter()
                        .next()
                        .map(PathBuf::from)
                        .ok_or_else(|| "No plugin package was selected".to_string())
                });
                ctx.dispatch_typed_action_for_view(
                    window_id,
                    view_id,
                    &PluginsPageAction::PackagePicked(result),
                );
            },
            FilePickerConfiguration::new(),
        );
    }

    fn set_setting(
        &mut self,
        plugin_id: &PluginId,
        key: &str,
        value: serde_json::Value,
        ctx: &mut ViewContext<Self>,
    ) {
        let result = Self::update_host(ctx, |host| {
            host.inventory()
                .and_then(|plugins| {
                    plugins
                        .into_iter()
                        .find(|plugin| &plugin.manifest.id == plugin_id)
                        .ok_or_else(|| format!("plugin is not installed: {plugin_id}"))
                })
                .and_then(|plugin| {
                    let mut values: BTreeMap<_, _> = plugin.settings;
                    values.insert(key.to_string(), value);
                    host.update_settings(plugin_id, values)
                })
        });
        if let Err(error) = result {
            self.show_toast(error, false, ctx);
        }
        self.rebuild(ctx);
    }

    fn update_host<T>(
        ctx: &mut ViewContext<Self>,
        operation: impl FnOnce(&WarpeerPluginHost) -> Result<T, String>,
    ) -> Result<T, String> {
        let handle = WarpeerPluginHost::handle(ctx);
        handle.update(ctx, |host, host_ctx| {
            let result = operation(host);
            host_ctx.notify();
            result
        })
    }
}

impl Entity for PluginsSettingsPageView {
    type Event = ();
}

impl View for PluginsSettingsPageView {
    fn ui_name() -> &'static str {
        "PluginsSettingsPage"
    }

    fn render(&self, app: &AppContext) -> Box<dyn warpui::Element> {
        self.layout.render(self, app)
    }
}

impl TypedActionView for PluginsSettingsPageView {
    type Action = PluginsPageAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            PluginsPageAction::ChoosePackage => self.choose_package(ctx),
            PluginsPageAction::PackagePicked(Ok(path)) => {
                match WarpeerPluginHost::as_ref(ctx).inspect_local(path) {
                    Ok(review) => self.pending_install = Some(review),
                    Err(error) => self.show_toast(error, false, ctx),
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::PackagePicked(Err(error)) => {
                self.show_toast(error.clone(), false, ctx)
            }
            PluginsPageAction::ConfirmInstall => {
                let Some(review) = self.pending_install.take() else {
                    return;
                };
                let approval = InstallApproval {
                    capability_expansion: !review.expanded_capabilities.is_empty(),
                    downgrade: review.downgrade,
                };
                match WarpeerPluginHost::as_ref(ctx).install(review, approval) {
                    Ok(plugin) => {
                        self.selected_plugin = Some(plugin.manifest.id.clone());
                        self.show_toast(
                            format!(
                                "Installed {} {}",
                                plugin.manifest.name, plugin.manifest.version
                            ),
                            true,
                            ctx,
                        );
                    }
                    Err(error) => self.show_toast(error, false, ctx),
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::CancelInstall => {
                self.pending_install = None;
                self.rebuild(ctx);
            }
            PluginsPageAction::SelectPlugin(id) => {
                self.selected_plugin = Some(id.clone());
                self.rebuild(ctx);
            }
            PluginsPageAction::ToggleActionsMenu(id) => {
                self.layout.toggle_actions_menu(id, ctx);
            }
            PluginsPageAction::ToggleEnabled(id) => {
                let result = Self::update_host(ctx, |host| {
                    host.inventory().and_then(|plugins| {
                        let enabled = plugins
                            .into_iter()
                            .find(|plugin| &plugin.manifest.id == id)
                            .map(|plugin| plugin.enabled)
                            .ok_or_else(|| format!("plugin is not installed: {id}"))?;
                        host.set_enabled(id, !enabled)
                    })
                });
                if let Err(error) = result {
                    self.show_toast(error, false, ctx);
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::Reload(id) => {
                match Self::update_host(ctx, |host| host.restart(id, PluginRestartMode::Reload)) {
                    Ok(()) => self.show_toast("Plugin reloaded".into(), true, ctx),
                    Err(error) => self.show_toast(error, false, ctx),
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::ForceRestart(id) => {
                match Self::update_host(ctx, |host| host.restart(id, PluginRestartMode::Force)) {
                    Ok(()) => self.show_toast("Plugin force restarted".into(), true, ctx),
                    Err(error) => self.show_toast(error, false, ctx),
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::Retry(id) => {
                if let Err(error) = WarpeerPluginHost::as_ref(ctx).recover(id) {
                    self.show_toast(error, false, ctx);
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::SetPermission {
                plugin_id,
                capability,
                enabled,
            } => {
                if let Err(error) = WarpeerPluginHost::as_ref(ctx).set_permission(
                    plugin_id,
                    capability.clone(),
                    *enabled,
                ) {
                    self.show_toast(error, false, ctx);
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::SetSetting {
                plugin_id,
                key,
                value,
            } => self.set_setting(plugin_id, key, value.clone(), ctx),
            PluginsPageAction::Reveal(id) => {
                match WarpeerPluginHost::as_ref(ctx).package_root(id) {
                    Ok(path) => {
                        if let Err(error) = std::process::Command::new("open")
                            .arg("-R")
                            .arg(path)
                            .spawn()
                        {
                            self.show_toast(error.to_string(), false, ctx);
                        }
                    }
                    Err(error) => self.show_toast(error, false, ctx),
                }
            }
            PluginsPageAction::ResetSettings(id) => {
                if let Err(error) = WarpeerPluginHost::as_ref(ctx).reset_settings(id) {
                    self.show_toast(error, false, ctx);
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::RequestRemove(id) => {
                self.pending_remove = Some(id.clone());
                self.rebuild(ctx);
            }
            PluginsPageAction::ConfirmRemove { retain_data } => {
                if let Some(id) = self.pending_remove.take() {
                    if self.selected_plugin.as_ref() == Some(&id) {
                        self.selected_plugin = None;
                    }
                    if let Err(error) = WarpeerPluginHost::as_ref(ctx).uninstall(&id, *retain_data)
                    {
                        self.show_toast(error, false, ctx);
                    }
                }
                self.rebuild(ctx);
            }
            PluginsPageAction::CancelRemove => {
                self.pending_remove = None;
                self.rebuild(ctx);
            }
        }
    }
}

impl PluginsSettingsPageView {
    pub(super) fn actions_menu_is_open(&self, id: &PluginId) -> bool {
        self.layout.actions_menu_is_open(id)
    }

    pub(super) fn close_actions_menu(&mut self) {
        self.layout.close_actions_menu();
    }
}

impl SettingsPageMeta for PluginsSettingsPageView {
    fn section() -> SettingsSection {
        SettingsSection::Plugins
    }

    fn should_render(&self, _ctx: &AppContext) -> bool {
        true
    }

    fn update_filter(&mut self, query: &str, ctx: &mut ViewContext<Self>) -> MatchData {
        let matches = self.layout.update_filter(query, ctx);
        ctx.notify();
        matches
    }

    fn scroll_to_widget(&mut self, widget_id: &'static str) {
        self.layout.scroll_to_widget(widget_id)
    }

    fn clear_highlighted_widget(&mut self) {
        self.layout.clear_highlighted_widget();
    }
}

impl From<ViewHandle<PluginsSettingsPageView>> for SettingsPageViewHandle {
    fn from(view_handle: ViewHandle<PluginsSettingsPageView>) -> Self {
        SettingsPageViewHandle::Plugins(view_handle)
    }
}
