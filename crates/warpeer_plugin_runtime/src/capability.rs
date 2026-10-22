use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use parking_lot::RwLock;
use warpeer_plugin_protocol::{
    BridgeError, Capability, CapabilityRequest, ContextId, PluginId, ResourceHandleId, Revision,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilityGrant {
    pub scopes: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct PluginPermissions {
    declared: BTreeSet<Capability>,
    grants: BTreeMap<Capability, CapabilityGrant>,
    generation: u64,
}

#[derive(Clone, Debug)]
struct ResourceHandle {
    plugin_id: PluginId,
    path: PathBuf,
    context_id: ContextId,
    context_revision: Revision,
    permission_generation: u64,
}

#[derive(Default)]
pub struct CapabilityRegistry {
    plugins: RwLock<HashMap<PluginId, PluginPermissions>>,
    resources: RwLock<HashMap<ResourceHandleId, ResourceHandle>>,
}

impl CapabilityRegistry {
    pub fn register(
        &self,
        plugin_id: PluginId,
        declared: impl IntoIterator<Item = Capability>,
        grants: BTreeMap<Capability, CapabilityGrant>,
    ) {
        self.plugins.write().insert(
            plugin_id,
            PluginPermissions {
                declared: declared.into_iter().collect(),
                grants,
                generation: 0,
            },
        );
    }

    pub fn authorize(&self, request: &CapabilityRequest) -> Result<Capability, BridgeError> {
        let capability = capability_for_method(&request.method).ok_or_else(|| {
            BridgeError::InvalidRequest(format!("unknown bridge method {}", request.method))
        })?;
        let plugins = self.plugins.read();
        let permissions = plugins
            .get(&request.plugin_id)
            .ok_or_else(|| BridgeError::PermissionDenied("plugin is not registered".into()))?;
        if !permissions.declared.contains(&capability) {
            return Err(BridgeError::PermissionDenied(format!(
                "capability {capability:?} was not declared"
            )));
        }
        let grant = permissions.grants.get(&capability).ok_or_else(|| {
            BridgeError::PermissionDenied(format!("capability {capability:?} is not granted"))
        })?;
        if let Some(scope) = &request.scope
            && !grant.scopes.is_empty()
            && !grant.scopes.contains(scope)
        {
            return Err(BridgeError::PermissionDenied(format!(
                "scope {scope} is not granted"
            )));
        }
        Ok(capability)
    }

    pub fn revoke(&self, plugin_id: &PluginId, capability: &Capability) {
        let mut plugins = self.plugins.write();
        if let Some(permissions) = plugins.get_mut(plugin_id) {
            permissions.grants.remove(capability);
            permissions.generation = permissions.generation.saturating_add(1);
        }
        drop(plugins);
        self.resources
            .write()
            .retain(|_, handle| &handle.plugin_id != plugin_id);
    }

    pub fn set_grants(&self, plugin_id: &PluginId, grants: BTreeMap<Capability, CapabilityGrant>) {
        let mut plugins = self.plugins.write();
        if let Some(permissions) = plugins.get_mut(plugin_id) {
            let changed = permissions.grants != grants;
            permissions.grants = grants;
            if changed {
                permissions.generation = permissions.generation.saturating_add(1);
            }
            if changed {
                drop(plugins);
                self.resources
                    .write()
                    .retain(|_, handle| &handle.plugin_id != plugin_id);
            }
        }
    }

    pub fn issue_exact_read_handle(
        &self,
        plugin_id: &PluginId,
        path: &Path,
        context_id: ContextId,
        context_revision: Revision,
    ) -> Result<ResourceHandleId, BridgeError> {
        let canonical = fs::canonicalize(path).map_err(|_| BridgeError::NotFound)?;
        if !canonical.is_file() {
            return Err(BridgeError::NotFound);
        }
        let plugins = self.plugins.read();
        let permissions = plugins
            .get(plugin_id)
            .ok_or_else(|| BridgeError::PermissionDenied("plugin is not registered".into()))?;
        if !permissions.grants.contains_key(&Capability::ResourceRead) {
            return Err(BridgeError::PermissionDenied(
                "resource-read is not granted".into(),
            ));
        }
        let id = ResourceHandleId::new();
        self.resources.write().insert(
            id,
            ResourceHandle {
                plugin_id: plugin_id.clone(),
                path: canonical,
                context_id,
                context_revision,
                permission_generation: permissions.generation,
            },
        );
        Ok(id)
    }

    pub fn read_exact(
        &self,
        plugin_id: &PluginId,
        handle_id: ResourceHandleId,
        context_id: ContextId,
        context_revision: Revision,
        max_bytes: u64,
    ) -> Result<Vec<u8>, BridgeError> {
        let path = self.resolve_exact_path(
            plugin_id,
            handle_id,
            context_id,
            context_revision,
            max_bytes,
        )?;
        fs::read(path).map_err(|error| BridgeError::Internal(error.to_string()))
    }

    pub fn resolve_exact_path(
        &self,
        plugin_id: &PluginId,
        handle_id: ResourceHandleId,
        context_id: ContextId,
        context_revision: Revision,
        max_bytes: u64,
    ) -> Result<PathBuf, BridgeError> {
        let resources = self.resources.read();
        let handle = resources.get(&handle_id).ok_or(BridgeError::NotFound)?;
        let plugins = self.plugins.read();
        let permissions = plugins
            .get(plugin_id)
            .ok_or_else(|| BridgeError::PermissionDenied("plugin is not registered".into()))?;
        if &handle.plugin_id != plugin_id
            || handle.context_id != context_id
            || handle.context_revision != context_revision
        {
            return Err(BridgeError::StaleContext);
        }
        if handle.permission_generation != permissions.generation
            || !permissions.grants.contains_key(&Capability::ResourceRead)
        {
            return Err(BridgeError::PermissionDenied(
                "resource handle was revoked".into(),
            ));
        }
        let metadata = fs::metadata(&handle.path).map_err(|_| BridgeError::NotFound)?;
        if metadata.len() > max_bytes {
            return Err(BridgeError::ResourceLimit(format!(
                "file is {} bytes; maximum is {max_bytes}",
                metadata.len()
            )));
        }
        Ok(handle.path.clone())
    }

    pub fn release(&self, plugin_id: &PluginId, handle_id: ResourceHandleId) {
        self.resources
            .write()
            .retain(|id, handle| id != &handle_id || &handle.plugin_id != plugin_id);
    }

    pub fn unregister(&self, plugin_id: &PluginId) {
        self.plugins.write().remove(plugin_id);
        self.resources
            .write()
            .retain(|_, handle| &handle.plugin_id != plugin_id);
    }
}

fn capability_for_method(method: &str) -> Option<Capability> {
    let namespace = method.split('.').take(2).collect::<Vec<_>>().join(".");
    match namespace.as_str() {
        "terminal.context" => Some(Capability::TerminalContext),
        "resource.read" => Some(Capability::ResourceRead),
        "network.fetch" => Some(Capability::NetworkFetch),
        "pane.surface" => Some(Capability::PaneSurface),
        "pane.metadata" => Some(Capability::PaneMetadata),
        "settings.store" => Some(Capability::SettingsStore),
        _ => None,
    }
}
