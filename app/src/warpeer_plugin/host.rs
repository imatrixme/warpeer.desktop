use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use warpeer_plugin_protocol::{
    Capability, ContainerLimits, ContextId, DiagnosticLevel, HostToPlugin, PaneContext, PluginId,
    PluginSettingsContext, ResourceBudgets, Revision, SurfaceSlot, TerminalHoverContext, WorkerId,
    WorkerState,
};
use warpeer_plugin_runtime::{
    CapabilityGrant, CapabilityRegistry, ContainerStore, DiagnosticEntry, InstallApproval,
    InstallReview, InstalledPlugin, PluginManager, PluginPaths, PluginSupervisor, PreviewCache,
    negotiate_extensions, validate_archive,
};
use warpui::{Entity, ModelContext, SingletonEntity};

use super::bridge_dispatch::PendingRequest;
use super::process::{ProcessLauncher, WorkerOutput};
use super::surface_store::SurfaceStore;

const WORKER_IDLE_GRACE: Duration = Duration::from_secs(30);
const WORKER_RESOURCE_RELEASE_GRACE: Duration = Duration::from_millis(750);
const WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const OUTPUT_QUEUE: usize = 256;
const SURFACE_VISIBILITY_LEASE: Duration = Duration::from_millis(250);

pub(super) fn terminal_trigger_enabled(plugin: &InstalledPlugin, explicit_preview: bool) -> bool {
    match plugin
        .settings
        .get("trigger_mode")
        .and_then(serde_json::Value::as_str)
    {
        Some("Hover") => !explicit_preview,
        Some("Click menu") => explicit_preview,
        _ => true,
    }
}

pub(crate) struct WarpeerPluginHost {
    pub(super) core: Arc<HostCore>,
}

#[derive(Clone)]
pub(crate) struct PluginHostClient {
    pub(super) core: Arc<HostCore>,
}

impl WarpeerPluginHost {
    pub fn new(ctx: &mut ModelContext<Self>) -> anyhow::Result<Self> {
        let manager = Arc::new(PluginManager::new(PluginPaths::new(
            warp_core::paths::state_dir(),
        ))?);
        let (output_tx, output_rx) = async_channel::bounded(OUTPUT_QUEUE);
        let (diagnostic_tx, diagnostic_rx) = async_channel::bounded(1);
        let launcher = ProcessLauncher::new(manager.clone(), output_tx);
        let supervisor = PluginSupervisor::new(
            launcher.clone(),
            ResourceBudgets::default(),
            WORKER_IDLE_GRACE,
        )
        .with_resource_release_grace(WORKER_RESOURCE_RELEASE_GRACE);
        let core = Arc::new(HostCore {
            manager,
            launcher,
            supervisor,
            capabilities: Arc::default(),
            surfaces: Arc::default(),
            containers: Arc::default(),
            hover_contexts: Mutex::default(),
            pane_contexts: Mutex::default(),
            settings_contexts: Mutex::default(),
            pending: Mutex::default(),
            preview_caches: Mutex::default(),
            diagnostic_tx,
        });
        core.ensure_bundled_image_preview()?;
        core.register_installed()?;
        ctx.spawn_stream_local(
            output_rx,
            |host, output, ctx| {
                host.core.accept_output(output);
                ctx.notify();
            },
            |_, _| {},
        );
        ctx.spawn_stream_local(diagnostic_rx, |_, _, ctx| ctx.notify(), |_, _| {});
        spawn_idle_reaper(Arc::downgrade(&core));
        Ok(Self { core })
    }

    pub fn inventory(&self) -> Result<Vec<InstalledPlugin>, String> {
        self.core
            .manager
            .inventory()
            .map_err(|error| error.to_string())
    }

    pub fn client(&self) -> PluginHostClient {
        PluginHostClient {
            core: self.core.clone(),
        }
    }

    pub fn set_enabled(&self, id: &PluginId, enabled: bool) -> Result<(), String> {
        self.core.set_enabled(id, enabled)
    }

    pub fn recover(&self, id: &PluginId) -> Result<(), String> {
        let plugin = self
            .core
            .manager
            .installed(id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("plugin is not installed: {id}"))?;
        if !plugin.enabled {
            return Err("Enable the plugin before retrying it".into());
        }
        self.core.register_plugin(&plugin);
        self.core
            .supervisor
            .recover(id)
            .map_err(|error| error.to_string())?;
        self.core.activate_plugin_contexts(&plugin);
        Ok(())
    }

    pub fn restart(&self, id: &PluginId, mode: super::PluginRestartMode) -> Result<(), String> {
        self.core.restart_plugin(id, mode)
    }

    pub fn inspect_local(&self, path: &Path) -> Result<InstallReview, String> {
        self.core
            .manager
            .inspect_local(path)
            .map_err(|error| error.to_string())
    }

    pub fn install(
        &self,
        review: InstallReview,
        approval: InstallApproval,
    ) -> Result<InstalledPlugin, String> {
        let installed = self
            .core
            .manager
            .install(review, approval)
            .map_err(|error| error.to_string())?;
        let id = installed.manifest.id.clone();
        if self.core.supervisor.state(&id).is_some() {
            self.core.shutdown_plugin(&id, true)?;
        }
        if installed.enabled {
            self.core.register_plugin(&installed);
            self.core
                .supervisor
                .recover(&id)
                .map_err(|error| error.to_string())?;
            self.core.activate_plugin_contexts(&installed);
        }
        Ok(installed)
    }

    pub fn uninstall(&self, id: &PluginId, retain_data: bool) -> Result<(), String> {
        self.core.shutdown_plugin(id, true)?;
        self.core
            .manager
            .uninstall(id, retain_data)
            .map_err(|error| error.to_string())
    }

    pub fn package_root(&self, id: &PluginId) -> Result<PathBuf, String> {
        self.core
            .manager
            .installed(id)
            .map_err(|error| error.to_string())?
            .map(|plugin| plugin.package_root(self.core.manager.paths()))
            .ok_or_else(|| format!("plugin is not installed: {id}"))
    }

    pub fn diagnostics(&self, id: &PluginId) -> Vec<DiagnosticEntry> {
        self.core.supervisor.diagnostics(id)
    }

    pub fn worker_state(&self, id: &PluginId) -> Option<WorkerState> {
        self.core.supervisor.state(id)
    }

    pub fn update_settings(
        &self,
        id: &PluginId,
        values: BTreeMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let plugin = self
            .core
            .manager
            .update_settings(id, values)
            .map_err(|error| error.to_string())?;
        self.core.send_configuration(&plugin);
        Ok(())
    }

    pub fn reset_settings(&self, id: &PluginId) -> Result<(), String> {
        let plugin = self
            .core
            .manager
            .reset_settings(id)
            .map_err(|error| error.to_string())?;
        self.core.send_configuration(&plugin);
        self.core.preview_caches.lock().remove(id);
        Ok(())
    }

    pub fn set_permission(
        &self,
        id: &PluginId,
        capability: Capability,
        enabled: bool,
    ) -> Result<(), String> {
        self.core.set_permission(id, capability, enabled)
    }
}

impl Entity for WarpeerPluginHost {
    type Event = ();
}

impl SingletonEntity for WarpeerPluginHost {}

impl PluginHostClient {
    pub fn terminal_hover(&self, context: TerminalHoverContext) {
        self.core.dispatch_hover(context);
    }

    pub fn cancel_hover(&self, context_id: ContextId, revision: Revision) {
        self.core.cancel_hover(context_id, revision);
    }
}

pub(super) struct HostCore {
    pub(super) manager: Arc<PluginManager>,
    pub(super) launcher: ProcessLauncher,
    pub(super) supervisor: PluginSupervisor<ProcessLauncher>,
    pub(super) capabilities: Arc<CapabilityRegistry>,
    pub(super) surfaces: Arc<SurfaceStore>,
    pub(super) containers: Arc<ContainerStore>,
    pub(super) hover_contexts: Mutex<HashMap<ContextId, TerminalHoverContext>>,
    pub(super) pane_contexts: Mutex<HashMap<ContextId, PaneContext>>,
    pub(super) settings_contexts: Mutex<HashMap<ContextId, PluginSettingsContext>>,
    pub(super) pending: Mutex<HashMap<warpeer_plugin_protocol::RequestId, PendingRequest>>,
    pub(super) preview_caches: Mutex<HashMap<PluginId, PreviewCache>>,
    pub(super) diagnostic_tx: async_channel::Sender<()>,
}

impl HostCore {
    fn ensure_bundled_image_preview(&self) -> anyhow::Result<()> {
        static PACKAGE: &[u8] =
            include_bytes!("../../../plugins/image-preview/dist/image-preview.warpeerplugin");
        let package = validate_archive(Cursor::new(PACKAGE))?;
        self.manager.install_bundled_if_newer(package)?;
        Ok(())
    }

    fn register_installed(&self) -> anyhow::Result<()> {
        for plugin in self
            .manager
            .inventory()?
            .into_iter()
            .filter(|plugin| plugin.enabled)
        {
            self.register_plugin(&plugin);
        }
        Ok(())
    }

    pub(super) fn register_plugin(&self, plugin: &InstalledPlugin) {
        self.supervisor
            .register(plugin.manifest.id.clone(), plugin.manifest.budgets);
        let grants = plugin
            .grants
            .iter()
            .map(|(capability, scopes)| {
                (
                    capability.clone(),
                    CapabilityGrant {
                        scopes: scopes.clone(),
                    },
                )
            })
            .collect();
        self.capabilities.register(
            plugin.manifest.id.clone(),
            plugin
                .manifest
                .capabilities
                .iter()
                .map(|declaration| declaration.capability.clone()),
            grants,
        );
    }

    pub(super) fn start_message(
        &self,
        plugin: &InstalledPlugin,
        worker_id: WorkerId,
    ) -> Result<HostToPlugin, String> {
        let extensions = negotiate_extensions(&plugin.manifest.extensions)
            .map_err(|error| format!("plugin container compatibility failed: {error:?}"))?;
        Ok(HostToPlugin::Start {
            plugin_id: plugin.manifest.id.clone(),
            worker_id,
            extensions,
            container_limits: ContainerLimits::default(),
        })
    }

    fn dispatch_hover(&self, context: TerminalHoverContext) {
        self.hover_contexts
            .lock()
            .insert(context.context_id, context.clone());
        let Ok(plugins) = self.manager.inventory() else {
            return;
        };
        for plugin in plugins.into_iter().filter(|plugin| {
            plugin.enabled
                && plugin.grants.contains_key(&Capability::TerminalContext)
                && plugin
                    .manifest
                    .surfaces
                    .contains(&SurfaceSlot::TerminalHover)
        }) {
            let target = context
                .resolved_resource
                .as_deref()
                .unwrap_or(&context.resource_text);
            if !plugin.manifest.accepts_terminal_hover(target) {
                self.diagnostic(
                    &plugin.manifest.id,
                    DiagnosticLevel::Info,
                    "terminal hover ignored by manifest matcher",
                );
                continue;
            }
            if !terminal_trigger_enabled(&plugin, context.explicit_preview) {
                self.diagnostic(
                    &plugin.manifest.id,
                    DiagnosticLevel::Info,
                    "terminal interaction ignored by configured trigger mode",
                );
                continue;
            }
            self.diagnostic(
                &plugin.manifest.id,
                DiagnosticLevel::Info,
                if context.explicit_preview {
                    "explicit terminal preview matched manifest"
                } else {
                    "terminal hover matched manifest"
                },
            );
            let triggered = match self.supervisor.trigger(&plugin.manifest.id) {
                Ok(triggered) => triggered,
                Err(error) => {
                    self.diagnostic(
                        &plugin.manifest.id,
                        DiagnosticLevel::Warning,
                        format!("worker trigger rejected: {error}"),
                    );
                    continue;
                }
            };
            if triggered.launched {
                let start = match self.start_message(&plugin, triggered.worker_id) {
                    Ok(start) => start,
                    Err(error) => {
                        self.supervisor.note_crash(&plugin.manifest.id, error);
                        continue;
                    }
                };
                if let Err(error) = self.launcher.send(&plugin.manifest.id, start) {
                    self.supervisor.note_crash(&plugin.manifest.id, error);
                    continue;
                }
                self.send_configuration(&plugin);
            }
            match self.launcher.send(
                &plugin.manifest.id,
                HostToPlugin::TerminalHover(context.clone()),
            ) {
                Ok(()) => self.diagnostic(
                    &plugin.manifest.id,
                    DiagnosticLevel::Info,
                    "terminal hover dispatched to worker",
                ),
                Err(error) => self.diagnostic(
                    &plugin.manifest.id,
                    DiagnosticLevel::Warning,
                    format!("terminal hover dispatch failed: {error}"),
                ),
            }
        }
    }

    pub(super) fn send_configuration(&self, plugin: &InstalledPlugin) {
        let _ = self.launcher.send(
            &plugin.manifest.id,
            HostToPlugin::Configuration {
                revision: Revision(plugin.settings_revision),
                values: serde_json::to_value(&plugin.settings).unwrap_or_default(),
            },
        );
    }

    fn cancel_hover(&self, context_id: ContextId, revision: Revision) {
        self.hover_contexts.lock().remove(&context_id);
        self.cancel_surface_context(context_id, revision);
    }

    fn accept_output(self: &Arc<Self>, output: WorkerOutput) {
        match output {
            WorkerOutput::Failed {
                plugin_id,
                worker_id,
                detail,
            } => {
                if self.supervisor.worker_id(&plugin_id) != Some(worker_id) {
                    return;
                }
                self.fail_plugin_worker(&plugin_id, detail);
            }
            WorkerOutput::Diagnostic {
                plugin_id,
                worker_id,
                detail,
            } => {
                if self.supervisor.worker_id(&plugin_id) != Some(worker_id) {
                    return;
                }
                self.diagnostic(&plugin_id, DiagnosticLevel::Warning, detail.clone());
                log::warn!("plugin {plugin_id}: {detail}");
            }
            WorkerOutput::Message {
                plugin_id,
                worker_id,
                envelope,
            } => {
                if self.supervisor.worker_id(&plugin_id) != Some(worker_id) {
                    return;
                }
                if envelope.protocol != warpeer_plugin_protocol::CURRENT_PROTOCOL_VERSION
                    || envelope.abi != warpeer_plugin_protocol::CURRENT_RUNTIME_ABI
                {
                    self.supervisor
                        .note_crash(&plugin_id, "incompatible worker protocol");
                    return;
                }
                self.accept_message(plugin_id, worker_id, envelope.message);
            }
        }
    }
}

impl Drop for HostCore {
    fn drop(&mut self) {
        if let Ok(plugins) = self.manager.inventory() {
            for plugin in plugins {
                let _ = self.supervisor.stop(&plugin.manifest.id);
            }
        }
    }
}

fn spawn_idle_reaper(core: Weak<HostCore>) {
    thread::Builder::new()
        .name("warpeer-plugin-reaper".into())
        .spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                let Some(core) = core.upgrade() else {
                    break;
                };
                let now = Instant::now();
                core.supervisor
                    .reap_unresponsive(now, WORKER_STARTUP_TIMEOUT);
                core.supervisor.reap_idle(now);
                core.occlude_unpresented_surfaces(now, SURFACE_VISIBILITY_LEASE);
            }
        })
        .expect("spawn Warpeer plugin idle reaper");
}
