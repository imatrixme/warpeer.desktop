use warpeer_plugin_protocol::PluginId;
use warpeer_plugin_runtime::{InstalledPlugin, WorkerRestartMode};

use super::host::HostCore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PluginRestartMode {
    Reload,
    Force,
}

impl HostCore {
    pub(super) fn set_enabled(&self, id: &PluginId, enabled: bool) -> Result<(), String> {
        let plugin = self
            .manager
            .set_enabled(id, enabled)
            .map_err(|error| error.to_string())?;
        if enabled {
            self.register_plugin(&plugin);
            self.supervisor
                .recover(id)
                .map_err(|error| error.to_string())?;
            self.activate_plugin_contexts(&plugin);
        } else {
            self.shutdown_plugin(id, false)?;
        }
        Ok(())
    }

    pub(super) fn restart_plugin(
        &self,
        id: &PluginId,
        mode: PluginRestartMode,
    ) -> Result<(), String> {
        let plugin = self.installed_enabled(id)?;
        self.release_plugin_resources(id);
        self.supervisor
            .restart(
                id,
                match mode {
                    PluginRestartMode::Reload => WorkerRestartMode::Graceful,
                    PluginRestartMode::Force => WorkerRestartMode::Force,
                },
            )
            .map_err(|error| error.to_string())?;
        self.launcher.remove(id);
        self.register_plugin(&plugin);
        self.launch_fresh(&plugin)?;
        self.activate_plugin_contexts(&plugin);
        Ok(())
    }

    pub(super) fn shutdown_plugin(&self, id: &PluginId, unregister: bool) -> Result<(), String> {
        self.release_plugin_resources(id);
        let result = if self.supervisor.state(id).is_some() {
            if unregister {
                self.supervisor.unregister(id)
            } else {
                self.supervisor.stop(id)
            }
            .map_err(|error| error.to_string())
        } else {
            Ok(())
        };
        self.launcher.remove(id);
        result
    }

    pub(super) fn fail_plugin_worker(&self, id: &PluginId, detail: impl Into<String>) {
        self.release_plugin_resources(id);
        let _ = self.supervisor.stop(id);
        self.supervisor.note_crash(id, detail);
        self.launcher.remove(id);
    }

    fn installed_enabled(&self, id: &PluginId) -> Result<InstalledPlugin, String> {
        let plugin = self
            .manager
            .installed(id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("plugin is not installed: {id}"))?;
        if plugin.enabled {
            Ok(plugin)
        } else {
            Err("Enable the plugin before restarting it".into())
        }
    }

    fn launch_fresh(&self, plugin: &InstalledPlugin) -> Result<(), String> {
        let triggered = self
            .supervisor
            .trigger(&plugin.manifest.id)
            .map_err(|error| error.to_string())?;
        let start = self.start_message(plugin, triggered.worker_id)?;
        self.launcher.send(&plugin.manifest.id, start)?;
        self.send_configuration(plugin);
        Ok(())
    }

    pub(super) fn release_plugin_resources(&self, plugin_id: &PluginId) {
        let cancellations = {
            let mut pending = self.pending.lock();
            let mut cancellations = Vec::new();
            pending.retain(|_, request| {
                if &request.plugin_id == plugin_id {
                    cancellations.push(request.cancellation.clone());
                    false
                } else {
                    true
                }
            });
            cancellations
        };
        for cancellation in cancellations {
            cancellation.cancel();
        }
        self.surfaces.remove_plugin(plugin_id);
        self.containers.remove_plugin(plugin_id);
        self.supervisor.release_all_surfaces(plugin_id);
        self.preview_caches.lock().remove(plugin_id);
        self.capabilities.unregister(plugin_id);
    }
}
