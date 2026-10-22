use std::time::Instant;

use warpeer_plugin_protocol::{DiagnosticLevel, PluginId};

use super::host::{HostCore, WarpeerPluginHost};

#[derive(Clone, Debug)]
pub(crate) struct PluginDiagnosticRecord {
    pub plugin_id: PluginId,
    pub level: DiagnosticLevel,
    pub message: String,
    pub timestamp: Instant,
}

impl WarpeerPluginHost {
    pub fn diagnostic_records(&self) -> Vec<PluginDiagnosticRecord> {
        let Ok(plugins) = self.core.manager.inventory() else {
            return Vec::new();
        };
        let mut records = plugins
            .into_iter()
            .flat_map(|plugin| {
                let plugin_id = plugin.manifest.id;
                self.core
                    .supervisor
                    .diagnostics(&plugin_id)
                    .into_iter()
                    .map(move |entry| PluginDiagnosticRecord {
                        plugin_id: plugin_id.clone(),
                        level: entry.level,
                        message: entry.message,
                        timestamp: entry.timestamp,
                    })
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|entry| entry.timestamp);
        records
    }

    pub fn clear_diagnostics(&self) {
        if let Ok(plugins) = self.core.manager.inventory() {
            for plugin in plugins {
                self.core.supervisor.clear_diagnostics(&plugin.manifest.id);
            }
        }
    }
}

impl HostCore {
    pub(super) fn diagnostic(
        &self,
        plugin_id: &PluginId,
        level: DiagnosticLevel,
        message: impl Into<String>,
    ) {
        if self.supervisor.record_diagnostic(plugin_id, level, message) {
            let _ = self.diagnostic_tx.try_send(());
        }
    }
}
