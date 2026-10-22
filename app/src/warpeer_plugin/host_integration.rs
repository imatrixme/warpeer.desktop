use std::time::{Duration, Instant};

use serde_json::Value;
use warpeer_plugin_protocol::{Capability, PluginId, WorkerId};
use warpeer_plugin_runtime::PluginSource;

use super::WarpeerPluginHost;

impl WarpeerPluginHost {
    pub(crate) fn integration_source(&self, id: &PluginId) -> Option<PluginSource> {
        self.core
            .manager
            .installed(id)
            .ok()
            .flatten()
            .map(|plugin| plugin.source)
    }

    pub(crate) fn integration_enabled(&self, id: &PluginId) -> Option<bool> {
        self.core
            .manager
            .installed(id)
            .ok()
            .flatten()
            .map(|plugin| plugin.enabled)
    }

    pub(crate) fn integration_setting(&self, id: &PluginId, key: &str) -> Option<Value> {
        self.core
            .manager
            .installed(id)
            .ok()
            .flatten()
            .and_then(|plugin| plugin.settings.get(key).cloned())
    }

    pub(crate) fn integration_has_grant(&self, id: &PluginId, capability: &Capability) -> bool {
        self.core
            .manager
            .installed(id)
            .ok()
            .flatten()
            .is_some_and(|plugin| plugin.grants.contains_key(capability))
    }

    pub(crate) fn integration_surface_count(&self) -> usize {
        self.core.surfaces.len()
    }

    pub(crate) fn integration_container_count(&self) -> usize {
        self.core.containers.len()
    }

    pub(crate) fn integration_pending_request_count(&self) -> usize {
        self.core.pending.lock().len()
    }

    pub(crate) fn integration_hover_context_count(&self) -> usize {
        self.core.hover_contexts.lock().len()
    }

    pub(crate) fn integration_preview_cache_bytes(&self, id: &PluginId) -> u64 {
        self.core
            .preview_caches
            .lock()
            .get(id)
            .map_or(0, |cache| cache.bytes())
    }

    pub(crate) fn integration_connection_count(&self) -> usize {
        self.core.launcher.connection_count()
    }

    pub(crate) fn integration_worker_id(&self, id: &PluginId) -> Option<WorkerId> {
        self.core.supervisor.worker_id(id)
    }

    pub(crate) fn integration_worker_memory_bytes(&self, id: &PluginId) -> Option<u64> {
        self.core.launcher.worker_memory_bytes(id)
    }

    pub(crate) fn integration_reap_idle(&self) {
        self.core
            .supervisor
            .reap_idle(Instant::now() + Duration::from_secs(31));
    }

    pub(crate) fn integration_note_crash(&self, id: &PluginId, detail: &str) {
        self.core.fail_plugin_worker(id, detail);
    }

    pub(crate) fn integration_terminate_worker(&self, id: &PluginId) -> Result<(), String> {
        self.core.launcher.terminate_worker(id)
    }
}
