use std::sync::Arc;
use std::thread;
use std::time::Duration;

use warpeer_plugin_protocol::{
    ChromeAction, ChromeActionSource, ChromeMenuItem, CloseContainerRequest,
    ContainerAcknowledgement, ContainerEvent, ContainerExtension, ContainerId, ContainerRejection,
    ContainerSnapshot, ContainerTransaction, ContextId, DiagnosticLevel, HostToPlugin,
    OpenContainerRequest, PageId, PluginId, RequestId, Revision, SurfaceContentSize, SurfaceId,
    SurfacePoint, SurfaceSlot, WorkerId,
};
use warpeer_plugin_runtime::ContainerCommit;

use super::host::HostCore;
use super::surface_store::SurfaceSnapshot;

const WINDOW_MARGIN: f32 = 8.0;
const ANCHOR_GAP: f32 = 10.0;

#[derive(Clone)]
pub(crate) struct PresentedContainer {
    pub container: ContainerSnapshot,
    pub surface: SurfaceSnapshot,
    pub transition_surface: Option<SurfaceSnapshot>,
    pub transition_page: Option<warpeer_plugin_protocol::ContainerPage>,
}

impl HostCore {
    pub(super) fn open_plugin_container(
        self: &Arc<Self>,
        plugin_id: PluginId,
        worker_id: WorkerId,
        request: OpenContainerRequest,
    ) {
        if self.containers.has_plugin(&plugin_id) {
            self.diagnostic(
                &plugin_id,
                DiagnosticLevel::Warning,
                "plugin container rejected: only one window is allowed",
            );
            self.send_container_acknowledgement(
                &plugin_id,
                rejected_acknowledgement(
                    request.request_id,
                    request.container_id,
                    ContainerRejection::ResourceLimit("one window per plugin".into()),
                ),
            );
            return;
        }
        let Some((viewport, origin)) = self.container_context_geometry(&request) else {
            self.reject_container_context(&plugin_id, &request);
            return;
        };
        if !self.container_extension_allowed(&plugin_id, ContainerExtension::Chrome) {
            self.reject_container_compatibility(&plugin_id, &request);
            return;
        }
        let commit = self.containers.open(
            plugin_id.clone(),
            worker_id,
            request,
            viewport,
            origin,
            |surface_id| self.container_surface_authorized(&plugin_id, surface_id),
        );
        self.deliver_container_commit(&plugin_id, commit);
    }

    pub(super) fn transact_plugin_container(
        self: &Arc<Self>,
        plugin_id: PluginId,
        worker_id: WorkerId,
        request: ContainerTransaction,
    ) {
        let commit = self
            .containers
            .transact(&plugin_id, worker_id, request, |surface_id| {
                self.container_surface_authorized(&plugin_id, surface_id)
            });
        self.deliver_container_commit(&plugin_id, commit);
    }

    pub(super) fn close_plugin_container(
        self: &Arc<Self>,
        plugin_id: PluginId,
        worker_id: WorkerId,
        request: CloseContainerRequest,
    ) {
        let commit = self.containers.close(&plugin_id, worker_id, request);
        self.deliver_container_commit(&plugin_id, commit);
    }

    pub(super) fn container_for_context(
        &self,
        context_id: ContextId,
        revision: Revision,
        slot: SurfaceSlot,
    ) -> Option<PresentedContainer> {
        let container = self.containers.for_context(context_id, revision, slot)?;
        let surface_id = container.active_page()?.surface_id;
        let surface = self.surfaces.snapshot(surface_id)?;
        let ledger = self.containers.ledger(container.container_id);
        let transition_surface = ledger
            .as_ref()
            .and_then(|ledger| {
                ledger
                    .transition_surfaces
                    .iter()
                    .copied()
                    .into_iter()
                    .find(|candidate| *candidate != surface_id)
            })
            .and_then(|surface_id| self.surfaces.snapshot(surface_id));
        Some(PresentedContainer {
            container,
            surface,
            transition_surface,
            transition_page: ledger.and_then(|ledger| ledger.transition_page),
        })
    }

    pub(super) fn move_container(&self, id: ContainerId, origin: SurfacePoint) {
        let Some(snapshot) = self.containers.set_origin(id, origin) else {
            return;
        };
        let Some(page) = snapshot.active_page() else {
            return;
        };
        let _ = self.launcher.try_send(
            &snapshot.plugin_id,
            HostToPlugin::ContainerEvent(ContainerEvent::GeometryChanged {
                container_id: id,
                revision: snapshot.revision,
                origin,
                content_size: page.preferred_content_size,
            }),
        );
    }

    pub(super) fn set_container_drag_capture(&self, id: ContainerId, captured: bool) {
        self.containers.set_drag_capture(id, captured);
    }

    pub(super) fn activate_container_action(
        &self,
        id: ContainerId,
        page_id: PageId,
        revision: Revision,
        action_id: String,
        menu_item_id: Option<String>,
        source: ChromeActionSource,
    ) -> bool {
        let Some(snapshot) = self.containers.snapshot(id) else {
            return false;
        };
        if snapshot.revision != revision || snapshot.active_page_id != page_id {
            return false;
        }
        let Some(page) = snapshot.active_page() else {
            return false;
        };
        let action = page
            .chrome
            .leading
            .iter()
            .chain(&page.chrome.trailing)
            .flat_map(|group| &group.actions)
            .find(|action| action.id == action_id && action.enabled);
        let valid = action.is_some_and(|action| {
            menu_item_id
                .as_deref()
                .is_none_or(|item_id| enabled_menu_item(action, item_id))
        });
        if !valid {
            self.diagnostic(
                &snapshot.plugin_id,
                DiagnosticLevel::Warning,
                "container chrome action was stale or unavailable",
            );
            return false;
        }
        self.launcher
            .try_send(
                &snapshot.plugin_id,
                HostToPlugin::ContainerEvent(ContainerEvent::ChromeAction {
                    container_id: id,
                    page_id,
                    revision,
                    action_id,
                    menu_item_id,
                    source,
                }),
            )
            .is_ok()
    }

    fn container_extension_allowed(&self, id: &PluginId, extension: ContainerExtension) -> bool {
        self.manager
            .installed(id)
            .ok()
            .flatten()
            .is_some_and(|plugin| {
                plugin.enabled
                    && plugin
                        .manifest
                        .extensions
                        .iter()
                        .any(|requirement| requirement.extension == extension)
            })
    }

    fn container_surface_authorized(&self, plugin_id: &PluginId, surface_id: SurfaceId) -> bool {
        self.surfaces
            .owner(surface_id)
            .is_some_and(|(owner, descriptor, _)| {
                owner == *plugin_id
                    && self.surface_target_active(
                        plugin_id,
                        &descriptor.target,
                        descriptor.revision,
                    )
            })
    }

    pub(super) fn container_context_geometry(
        &self,
        request: &OpenContainerRequest,
    ) -> Option<([f32; 4], SurfacePoint)> {
        if request.slot != SurfaceSlot::TerminalHover {
            return None;
        }
        let context = self.hover_contexts.lock().get(&request.context_id)?.clone();
        if context.revision != request.context_revision {
            return None;
        }
        let content = request.root.preferred_content_size;
        Some((
            context.viewport,
            anchored_origin(context.anchor, context.viewport, content),
        ))
    }

    fn reject_container_context(&self, plugin_id: &PluginId, request: &OpenContainerRequest) {
        self.diagnostic(
            plugin_id,
            DiagnosticLevel::Warning,
            "container target is stale or unavailable",
        );
        self.send_container_acknowledgement(
            plugin_id,
            rejected_acknowledgement(
                request.request_id,
                request.container_id,
                ContainerRejection::InvalidRequest("stale or unavailable context".into()),
            ),
        );
    }

    fn reject_container_compatibility(&self, plugin_id: &PluginId, request: &OpenContainerRequest) {
        self.diagnostic(
            plugin_id,
            DiagnosticLevel::Warning,
            "container compatibility rejected: chrome extension unavailable",
        );
        self.send_container_acknowledgement(
            plugin_id,
            rejected_acknowledgement(
                request.request_id,
                request.container_id,
                ContainerRejection::Compatibility(
                    "container chrome extension was not negotiated".into(),
                ),
            ),
        );
    }

    pub(super) fn deliver_container_commit(
        self: &Arc<Self>,
        plugin_id: &PluginId,
        commit: ContainerCommit,
    ) {
        let completed_transition = commit
            .snapshot
            .as_ref()
            .map(|snapshot| (snapshot.container_id, snapshot.revision, snapshot.worker_id));
        let reduced_motion = commit
            .snapshot
            .as_ref()
            .and_then(ContainerSnapshot::active_page)
            .and_then(|page| self.surfaces.owner(page.surface_id))
            .and_then(|(_, _, environment)| environment)
            .is_some_and(|environment| environment.reduced_motion);
        let transition_delay = commit
            .acknowledgement
            .result
            .as_ref()
            .ok()
            .and_then(|applied| applied.transition)
            .filter(|_| !reduced_motion)
            .map_or(Duration::ZERO, |transition| {
                Duration::from_millis(u64::from(transition.duration_ms))
            });
        for event in commit.before_events {
            let _ = self
                .launcher
                .try_send(plugin_id, HostToPlugin::ContainerEvent(event));
        }
        let rejection = commit.acknowledgement.result.as_ref().err().cloned();
        self.send_container_acknowledgement(plugin_id, commit.acknowledgement);
        if let Some(rejection) = rejection {
            self.diagnostic(
                plugin_id,
                DiagnosticLevel::Warning,
                container_rejection_diagnostic(&rejection),
            );
        }
        if transition_delay.is_zero() {
            self.finish_container_transition(
                plugin_id,
                completed_transition,
                commit.after_events,
                commit.released_surfaces,
            );
            return;
        }
        let plugin_id = plugin_id.clone();
        let core = Arc::downgrade(self);
        thread::spawn(move || {
            thread::sleep(transition_delay);
            if let Some(core) = core.upgrade() {
                core.finish_container_transition(
                    &plugin_id,
                    completed_transition,
                    commit.after_events,
                    commit.released_surfaces,
                );
            }
        });
    }

    fn finish_container_transition(
        &self,
        plugin_id: &PluginId,
        completed: Option<(ContainerId, Revision, WorkerId)>,
        after_events: Vec<ContainerEvent>,
        released_surfaces: Vec<SurfaceId>,
    ) {
        let current = completed.and_then(|(container_id, revision, worker_id)| {
            self.containers
                .snapshot(container_id)
                .filter(|snapshot| snapshot.revision == revision && snapshot.worker_id == worker_id)
                .map(|snapshot| (container_id, revision, snapshot))
        });
        if completed.is_none() || current.is_some() {
            for event in after_events {
                let _ = self
                    .launcher
                    .try_send(plugin_id, HostToPlugin::ContainerEvent(event));
            }
        }
        if let Some((container_id, revision, _)) = current.as_ref() {
            self.containers
                .complete_transition(*container_id, *revision);
        }
        self.release_container_surfaces(
            plugin_id,
            current.as_ref().map(|(_, _, snapshot)| snapshot),
            released_surfaces,
        );
    }

    fn send_container_acknowledgement(
        &self,
        plugin_id: &PluginId,
        acknowledgement: ContainerAcknowledgement,
    ) {
        let _ = self.launcher.try_send(
            plugin_id,
            HostToPlugin::ContainerAcknowledgement(acknowledgement),
        );
    }

    fn release_container_surfaces(
        &self,
        plugin_id: &PluginId,
        snapshot: Option<&ContainerSnapshot>,
        released: Vec<SurfaceId>,
    ) {
        for surface_id in released {
            if snapshot.is_some_and(|snapshot| {
                snapshot
                    .pages
                    .iter()
                    .any(|page| page.surface_id == surface_id)
            }) {
                continue;
            }
            let Some((owner, descriptor, _)) = self.surfaces.owner(surface_id) else {
                continue;
            };
            if owner == *plugin_id
                && self
                    .surfaces
                    .close(plugin_id, surface_id, descriptor.revision)
            {
                self.supervisor.release_surface(plugin_id);
                let _ = self.launcher.try_send(
                    plugin_id,
                    HostToPlugin::ReleaseSurface {
                        surface_id,
                        revision: descriptor.revision,
                    },
                );
            }
        }
    }
}

fn enabled_menu_item(action: &ChromeAction, item_id: &str) -> bool {
    fn contains_enabled(items: &[ChromeMenuItem], item_id: &str) -> bool {
        items.iter().any(|item| {
            (item.id == item_id && item.enabled) || contains_enabled(&item.children, item_id)
        })
    }

    contains_enabled(&action.menu, item_id)
}

fn anchored_origin(
    anchor: [f32; 4],
    viewport: [f32; 4],
    content: SurfaceContentSize,
) -> SurfacePoint {
    let [viewport_x, viewport_y, viewport_width, viewport_height] = viewport;
    let [anchor_x, anchor_y, anchor_width, anchor_height] = anchor;
    let max_x = (viewport_x + viewport_width - content.width - WINDOW_MARGIN)
        .max(viewport_x + WINDOW_MARGIN);
    let x = (anchor_x + anchor_width * 0.5 - content.width * 0.5)
        .clamp(viewport_x + WINDOW_MARGIN, max_x);
    let below = anchor_y + anchor_height + ANCHOR_GAP;
    let y = if below + content.height <= viewport_y + viewport_height - WINDOW_MARGIN {
        below
    } else {
        (anchor_y - content.height - ANCHOR_GAP).max(viewport_y + WINDOW_MARGIN)
    };
    SurfacePoint { x, y }
}

fn rejected_acknowledgement(
    request_id: RequestId,
    container_id: ContainerId,
    reason: ContainerRejection,
) -> ContainerAcknowledgement {
    ContainerAcknowledgement {
        request_id,
        container_id,
        result: Err(reason),
    }
}

fn container_rejection_diagnostic(rejection: &ContainerRejection) -> String {
    match rejection {
        ContainerRejection::Compatibility(_) => "container compatibility rejected".into(),
        ContainerRejection::StaleRevision => "container stale revision rejected".into(),
        ContainerRejection::InvalidRequest(reason) => {
            format!("container invalid request rejected: {reason}")
        }
        ContainerRejection::ResourceLimit(limit) => {
            format!("container resource limit reached: {limit}")
        }
        ContainerRejection::UnknownContainer => "container identity is unavailable".into(),
        ContainerRejection::WrongWorker => "container command came from an old worker".into(),
    }
}

#[cfg(test)]
#[path = "container_host_tests.rs"]
mod tests;
