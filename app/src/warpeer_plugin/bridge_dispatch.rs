use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::thread;

use warpeer_plugin_protocol::{
    BridgeError, BridgePayload, Capability, CapabilityRequest, CapabilityResponse, ContextId,
    DiagnosticLevel, HostToPlugin, PluginId, PluginToHost, RequestId, SurfaceBounds, WorkerId,
    WorkerState,
};
use warpeer_plugin_runtime::{CapabilityGrant, FetchCancellation};

use super::host::HostCore;
use super::preview_bridge::load_preview;

pub(super) struct PendingRequest {
    pub plugin_id: PluginId,
    pub capability: Capability,
    pub context_id: Option<ContextId>,
    pub cancellation: FetchCancellation,
}

impl HostCore {
    pub(super) fn set_permission(
        &self,
        id: &PluginId,
        capability: Capability,
        enabled: bool,
    ) -> Result<(), String> {
        let plugin = self
            .manager
            .installed(id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("plugin is not installed: {id}"))?;
        let declaration = plugin
            .manifest
            .capabilities
            .iter()
            .find(|declaration| declaration.capability == capability)
            .ok_or_else(|| format!("plugin does not declare {capability:?}"))?;
        let mut grants = plugin.grants;
        if enabled {
            grants.insert(
                capability.clone(),
                declaration.scopes.iter().cloned().collect::<BTreeSet<_>>(),
            );
        } else {
            grants.remove(&capability);
        }
        let plugin = self
            .manager
            .set_grants(id, grants)
            .map_err(|error| error.to_string())?;
        let runtime_grants = plugin
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
            .collect::<BTreeMap<_, _>>();
        self.capabilities.set_grants(id, runtime_grants);
        if enabled {
            self.activate_plugin_contexts(&plugin);
        } else {
            for pending in self
                .pending
                .lock()
                .values()
                .filter(|pending| &pending.plugin_id == id && pending.capability == capability)
            {
                pending.cancellation.cancel();
            }
        }
        Ok(())
    }

    pub(super) fn accept_message(
        self: &Arc<Self>,
        plugin_id: PluginId,
        worker_id: WorkerId,
        message: PluginToHost,
    ) {
        match message {
            PluginToHost::CapabilityRequest(request) => self.dispatch_capability(request),
            PluginToHost::CancelRequest { request_id } => {
                if let Some(pending) = self.pending.lock().remove(&request_id) {
                    pending.cancellation.cancel();
                }
            }
            PluginToHost::OpenSurface(descriptor) => {
                let allowed = self
                    .manager
                    .installed(&plugin_id)
                    .ok()
                    .flatten()
                    .is_some_and(|plugin| {
                        plugin.enabled
                            && plugin.manifest.surfaces.contains(&descriptor.slot)
                            && plugin.grants.contains_key(&Capability::PaneSurface)
                    });
                if !allowed {
                    self.supervisor
                        .note_violation(&plugin_id, "plugin requested an unavailable surface slot");
                    return;
                }
                if !descriptor.is_valid() {
                    self.supervisor
                        .note_violation(&plugin_id, "plugin surface descriptor is invalid");
                    return;
                }
                if !self.surface_target_active(&plugin_id, &descriptor.target, descriptor.revision)
                {
                    self.supervisor.note_violation(
                        &plugin_id,
                        "plugin surface target is stale or unavailable",
                    );
                    return;
                }
                match self
                    .supervisor
                    .reserve_surface(&plugin_id, descriptor.bounds)
                {
                    Ok(()) => {
                        self.diagnostic(
                            &plugin_id,
                            DiagnosticLevel::Info,
                            format!("surface opened in {:?} slot", descriptor.slot),
                        );
                        self.surfaces.open(plugin_id.clone(), descriptor.clone());
                        self.open_legacy_surface_container(&plugin_id, worker_id, &descriptor);
                    }
                    Err(error) => self
                        .supervisor
                        .note_violation(&plugin_id, error.to_string()),
                }
            }
            PluginToHost::ResizeSurface(request) => {
                let Some((owner, descriptor, environment)) =
                    self.surfaces.owner(request.surface_id)
                else {
                    self.supervisor
                        .note_violation(&plugin_id, "plugin resized an unknown surface");
                    return;
                };
                if owner != plugin_id
                    || !request.is_valid()
                    || descriptor.revision != request.revision
                {
                    self.supervisor
                        .note_violation(&plugin_id, "plugin surface resize request is invalid");
                    return;
                }
                let bounds = SurfaceBounds {
                    x: descriptor.bounds.x,
                    y: descriptor.bounds.y,
                    width: request.content_size.width,
                    height: request.content_size.height,
                    scale: environment
                        .as_ref()
                        .map_or(descriptor.bounds.scale, |environment| {
                            environment.bounds.scale
                        }),
                };
                if let Err(error) = self.supervisor.validate_surface_bounds(&plugin_id, bounds) {
                    self.supervisor
                        .note_violation(&plugin_id, error.to_string());
                    return;
                }
                if self
                    .surfaces
                    .resize(&plugin_id, request, std::time::Instant::now())
                {
                    self.resize_legacy_surface_container(&plugin_id, request);
                    self.diagnostic(
                        &plugin_id,
                        DiagnosticLevel::Info,
                        "surface content resize accepted",
                    );
                }
            }
            PluginToHost::CloseSurface {
                surface_id,
                revision,
            } => {
                self.close_legacy_surface_container(&plugin_id, surface_id);
                if self.surfaces.close(&plugin_id, surface_id, revision) {
                    self.supervisor.release_surface(&plugin_id);
                    self.diagnostic(
                        &plugin_id,
                        DiagnosticLevel::Info,
                        "surface closed by plugin",
                    );
                }
            }
            PluginToHost::SurfaceReleased { .. } => {}
            PluginToHost::OpenContainer(request) => {
                self.open_plugin_container(plugin_id, worker_id, request);
            }
            PluginToHost::TransactContainer(request) => {
                self.transact_plugin_container(plugin_id, worker_id, request);
            }
            PluginToHost::CloseContainer(request) => {
                self.close_plugin_container(plugin_id, worker_id, request);
            }
            PluginToHost::Frame(frame) => {
                let frame_summary = format!(
                    "{}x{} surface revision {} environment revision {} frame revision {}",
                    frame.width_px,
                    frame.height_px,
                    frame.surface_revision.0,
                    frame.environment_revision.0,
                    frame.frame_revision.0,
                );
                if let Err(error) = self.supervisor.accept_frame(
                    &plugin_id,
                    frame.width_px as u64 * frame.height_px as u64,
                    std::time::Instant::now(),
                ) {
                    self.supervisor
                        .note_violation(&plugin_id, error.to_string());
                } else if !self.surfaces.apply_frame(&plugin_id, frame) {
                    self.supervisor.note_violation(
                        &plugin_id,
                        format!("plugin frame did not match active surface: {frame_summary}"),
                    );
                } else {
                    self.diagnostic(&plugin_id, DiagnosticLevel::Info, "surface frame accepted");
                }
            }
            PluginToHost::ReleaseResource(handle) => {
                self.capabilities.release(&plugin_id, handle);
                self.containers
                    .release_capability_handle(&plugin_id, handle);
            }
            PluginToHost::Lifecycle(event) if event.state == WorkerState::Failed => {
                self.fail_plugin_worker(
                    &plugin_id,
                    event.detail.unwrap_or_else(|| "worker failed".into()),
                );
            }
            PluginToHost::Diagnostic { level, message } => {
                self.diagnostic(&plugin_id, level.clone(), message.clone());
                match level {
                    DiagnosticLevel::Error | DiagnosticLevel::Warning => {
                        log::warn!("plugin {plugin_id}: {message}")
                    }
                    _ => log::debug!("plugin {plugin_id}: {message}"),
                }
            }
            PluginToHost::Lifecycle(event) => {
                self.supervisor.note_lifecycle(&plugin_id, event.state);
            }
        }
    }

    fn dispatch_capability(self: &Arc<Self>, request: CapabilityRequest) {
        self.diagnostic(
            &request.plugin_id,
            DiagnosticLevel::Info,
            format!("capability request: {}", request.method),
        );
        let capability = match self.capabilities.authorize(&request) {
            Ok(capability) => capability,
            Err(error) => {
                self.supervisor
                    .note_violation(&request.plugin_id, error.to_string());
                self.send_response(&request.plugin_id, request.request_id, Err(error));
                return;
            }
        };
        match request.method.as_str() {
            "terminal.context.current" => self.context_response(request),
            "settings.store.get" => self.settings_response(request),
            "settings.store.set" => self.settings_update_response(request),
            "resource.read.preview-image" | "network.fetch.preview-image" => {
                self.spawn_preview_request(request, capability)
            }
            _ => self.send_response(
                &request.plugin_id,
                request.request_id,
                Err(BridgeError::InvalidRequest(request.method)),
            ),
        }
    }

    fn context_response(&self, request: CapabilityRequest) {
        let result = request
            .context_id
            .and_then(|id| self.hover_contexts.lock().get(&id).cloned())
            .filter(|context| Some(context.revision) == request.context_revision)
            .ok_or(BridgeError::StaleContext)
            .and_then(|context| {
                serde_json::to_value(context)
                    .map(|value| BridgePayload::Json { value })
                    .map_err(|error| BridgeError::Internal(error.to_string()))
            });
        self.send_response(&request.plugin_id, request.request_id, result);
    }

    fn settings_response(&self, request: CapabilityRequest) {
        let result = self
            .manager
            .installed(&request.plugin_id)
            .map_err(|error| BridgeError::Internal(error.to_string()))
            .and_then(|plugin| plugin.ok_or(BridgeError::NotFound))
            .and_then(|plugin| {
                serde_json::to_value(plugin.settings)
                    .map(|value| BridgePayload::Json { value })
                    .map_err(|error| BridgeError::Internal(error.to_string()))
            });
        self.send_response(&request.plugin_id, request.request_id, result);
    }

    fn settings_update_response(&self, request: CapabilityRequest) {
        let result =
            serde_json::from_value::<BTreeMap<String, serde_json::Value>>(request.payload.clone())
                .map_err(|error| BridgeError::InvalidRequest(error.to_string()))
                .and_then(|values| {
                    self.manager
                        .update_settings(&request.plugin_id, values)
                        .map_err(|error| BridgeError::InvalidRequest(error.to_string()))
                })
                .and_then(|plugin| {
                    self.send_configuration(&plugin);
                    serde_json::to_value(serde_json::json!({
                        "revision": plugin.settings_revision,
                        "values": plugin.settings,
                    }))
                    .map(|value| BridgePayload::Json { value })
                    .map_err(|error| BridgeError::Internal(error.to_string()))
                });
        self.send_response(&request.plugin_id, request.request_id, result);
    }

    fn spawn_preview_request(self: &Arc<Self>, request: CapabilityRequest, capability: Capability) {
        let Some(context) = request
            .context_id
            .and_then(|id| self.hover_contexts.lock().get(&id).cloned())
            .filter(|context| Some(context.revision) == request.context_revision)
        else {
            self.send_response(
                &request.plugin_id,
                request.request_id,
                Err(BridgeError::StaleContext),
            );
            return;
        };
        let cancellation = FetchCancellation::default();
        self.pending.lock().insert(
            request.request_id,
            PendingRequest {
                plugin_id: request.plugin_id.clone(),
                capability,
                context_id: request.context_id,
                cancellation: cancellation.clone(),
            },
        );
        let core = Arc::downgrade(self);
        thread::spawn(move || {
            let result = load_preview(&core, &request, &context, &cancellation);
            if let Some(core) = core.upgrade() {
                core.pending.lock().remove(&request.request_id);
                let result = if cancellation.is_cancelled() {
                    Err(BridgeError::Cancelled)
                } else {
                    result
                };
                core.diagnostic(
                    &request.plugin_id,
                    if result.is_ok() {
                        DiagnosticLevel::Info
                    } else {
                        DiagnosticLevel::Warning
                    },
                    if result.is_ok() {
                        "preview resource capability completed"
                    } else {
                        "preview resource capability failed"
                    },
                );
                core.send_response(&request.plugin_id, request.request_id, result);
            }
        });
    }

    fn send_response(
        &self,
        plugin_id: &PluginId,
        request_id: RequestId,
        result: Result<BridgePayload, BridgeError>,
    ) {
        let _ = self.launcher.send(
            plugin_id,
            HostToPlugin::CapabilityResponse(CapabilityResponse { request_id, result }),
        );
    }
}
