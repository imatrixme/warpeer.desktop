use std::sync::Arc;

use warpeer_plugin_protocol::{
    ChromeVisibility, ContainerChrome, ContainerId, ContainerOperation, ContainerPage,
    ContainerTransaction, DiagnosticLevel, OpenContainerRequest, PageId, PluginId, RequestId,
    SurfaceContentSize, SurfaceDescriptor, SurfaceId, SurfaceResizeRequest, WorkerId,
};

use super::host::HostCore;

impl HostCore {
    pub(super) fn open_legacy_surface_container(
        &self,
        plugin_id: &PluginId,
        worker_id: WorkerId,
        descriptor: &SurfaceDescriptor,
    ) {
        let Some(plugin) = self.manager.installed(plugin_id).ok().flatten() else {
            return;
        };
        if !plugin.manifest.extensions.is_empty() {
            return;
        }
        let request = OpenContainerRequest {
            request_id: RequestId::new(),
            container_id: ContainerId::new(),
            context_id: descriptor.target.context_id(),
            context_revision: descriptor.revision,
            slot: descriptor.slot,
            root: ContainerPage {
                id: PageId::new(),
                revision: descriptor.revision,
                surface_id: descriptor.id,
                chrome: ContainerChrome {
                    visibility: ChromeVisibility::Visible,
                    title: Some(plugin.manifest.name),
                    ..ContainerChrome::default()
                },
                preferred_content_size: SurfaceContentSize {
                    width: descriptor.bounds.width,
                    height: descriptor.bounds.height,
                },
            },
            transition: None,
        };
        let Some((viewport, origin)) = self.container_context_geometry(&request) else {
            return;
        };
        let commit = self.containers.open(
            plugin_id.clone(),
            worker_id,
            request,
            viewport,
            origin,
            |surface_id| surface_id == descriptor.id,
        );
        if commit.acknowledgement.result.is_err() {
            self.diagnostic(
                plugin_id,
                DiagnosticLevel::Warning,
                "legacy surface container adaptation failed",
            );
        }
    }

    pub(super) fn close_legacy_surface_container(
        &self,
        plugin_id: &PluginId,
        surface_id: SurfaceId,
    ) {
        let legacy = self
            .manager
            .installed(plugin_id)
            .ok()
            .flatten()
            .is_some_and(|plugin| plugin.manifest.extensions.is_empty());
        if !legacy {
            return;
        }
        for retained_surface in self.containers.remove_surface(surface_id) {
            if retained_surface == surface_id {
                continue;
            }
            let Some((owner, descriptor, _)) = self.surfaces.owner(retained_surface) else {
                continue;
            };
            if owner == *plugin_id
                && self
                    .surfaces
                    .close(plugin_id, retained_surface, descriptor.revision)
            {
                self.supervisor.release_surface(plugin_id);
            }
        }
    }

    pub(super) fn resize_legacy_surface_container(
        self: &Arc<Self>,
        plugin_id: &PluginId,
        request: SurfaceResizeRequest,
    ) {
        let legacy = self
            .manager
            .installed(plugin_id)
            .ok()
            .flatten()
            .is_some_and(|plugin| plugin.manifest.extensions.is_empty());
        if !legacy {
            return;
        }
        let Some(snapshot) = self.containers.for_surface(request.surface_id) else {
            return;
        };
        let Some(mut page) = snapshot.active_page().cloned() else {
            return;
        };
        page.revision = page.revision.next();
        page.preferred_content_size = request.content_size;
        let commit = self.containers.transact(
            plugin_id,
            snapshot.worker_id,
            ContainerTransaction {
                request_id: RequestId::new(),
                container_id: snapshot.container_id,
                expected_revision: snapshot.revision,
                operation: ContainerOperation::Update { page },
                transition: None,
            },
            |surface_id| surface_id == request.surface_id,
        );
        self.deliver_container_commit(plugin_id, commit);
    }
}
