use std::collections::HashMap;

use parking_lot::Mutex;
use warpeer_plugin_protocol::{
    CloseContainerRequest, ContainerAcknowledgement, ContainerEvent, ContainerId, ContainerLimits,
    ContainerRejection, ContainerSnapshot, ContainerTransaction, OpenContainerRequest,
    PageLifecyclePhase, PluginId, Revision, SurfaceId, SurfacePoint, WorkerId,
};

use crate::ContainerResourceLedger;
use crate::container_budget::ContainerBudgetTracker;
use crate::container_ledger::ContainerRecord;
use crate::container_reducer::{
    applied, apply_operation, lifecycle, navigation_events, rejected, snapshot_size, valid_viewport,
};

#[derive(Clone, Debug)]
pub struct ContainerCommit {
    pub acknowledgement: ContainerAcknowledgement,
    pub snapshot: Option<ContainerSnapshot>,
    pub before_events: Vec<ContainerEvent>,
    pub after_events: Vec<ContainerEvent>,
    pub released_surfaces: Vec<SurfaceId>,
    pub replayed: bool,
}

pub struct ContainerStore {
    pub(crate) records: Mutex<HashMap<ContainerId, ContainerRecord>>,
    pub(crate) budget: ContainerBudgetTracker,
    limits: ContainerLimits,
}

impl Default for ContainerStore {
    fn default() -> Self {
        Self::new(ContainerLimits::default())
    }
}

impl ContainerStore {
    pub fn new(limits: ContainerLimits) -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            budget: ContainerBudgetTracker::new(limits),
            limits,
        }
    }

    pub fn limits(&self) -> ContainerLimits {
        self.limits
    }

    pub fn has_plugin(&self, plugin_id: &PluginId) -> bool {
        self.records
            .lock()
            .values()
            .any(|record| &record.snapshot.plugin_id == plugin_id)
    }

    pub fn open(
        &self,
        plugin_id: PluginId,
        worker_id: WorkerId,
        request: OpenContainerRequest,
        viewport: [f32; 4],
        origin: SurfacePoint,
        surface_authorized: impl Fn(SurfaceId) -> bool,
    ) -> ContainerCommit {
        if !self.budget.admit(&plugin_id) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::ResourceLimit("container transaction rate".into()),
            );
        }
        let mut records = self.records.lock();
        if records.contains_key(&request.container_id) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::InvalidRequest("container already exists".into()),
            );
        }
        if !valid_viewport(viewport)
            || !request.root.is_valid(self.limits)
            || request
                .transition
                .is_some_and(|transition| !transition.is_valid(self.limits))
        {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::InvalidRequest("invalid container root or viewport".into()),
            );
        }
        if !surface_authorized(request.root.surface_id) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::InvalidRequest(
                    "root surface is not owned by the plugin".into(),
                ),
            );
        }
        let revision = Revision(1);
        let snapshot = ContainerSnapshot {
            plugin_id,
            worker_id,
            container_id: request.container_id,
            revision,
            context_id: request.context_id,
            context_revision: request.context_revision,
            slot: request.slot,
            origin,
            viewport,
            active_page_id: request.root.id,
            pages: vec![request.root],
            transition: request.transition,
        };
        if let Some(limit) = self.budget.snapshot_limit(&snapshot) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::ResourceLimit(limit.into()),
            );
        }
        let acknowledgement = applied(
            request.request_id,
            request.container_id,
            revision,
            snapshot.active_page().expect("root page exists"),
            request.transition,
        );
        let mut record = ContainerRecord {
            snapshot: snapshot.clone(),
            processed: Default::default(),
            ledger: ContainerResourceLedger::default(),
        };
        record.rebuild_ledger();
        record.remember(acknowledgement.clone());
        records.insert(request.container_id, record);
        ContainerCommit {
            acknowledgement,
            snapshot: Some(snapshot.clone()),
            before_events: vec![lifecycle(&snapshot, PageLifecyclePhase::WillAppear)],
            after_events: vec![lifecycle(&snapshot, PageLifecyclePhase::DidAppear)],
            released_surfaces: Vec::new(),
            replayed: false,
        }
    }

    pub fn transact(
        &self,
        plugin_id: &PluginId,
        worker_id: WorkerId,
        request: ContainerTransaction,
        surface_authorized: impl Fn(SurfaceId) -> bool,
    ) -> ContainerCommit {
        if !self.budget.admit(plugin_id) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::ResourceLimit("container transaction rate".into()),
            );
        }
        let mut records = self.records.lock();
        let Some(record) = records.get_mut(&request.container_id) else {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::UnknownContainer,
            );
        };
        if let Some(acknowledgement) = record.acknowledgement(request.request_id) {
            return ContainerCommit {
                acknowledgement,
                snapshot: Some(record.snapshot.clone()),
                before_events: Vec::new(),
                after_events: Vec::new(),
                released_surfaces: Vec::new(),
                replayed: true,
            };
        }
        if &record.snapshot.plugin_id != plugin_id || record.snapshot.worker_id != worker_id {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::WrongWorker,
            );
        }
        if record.snapshot.revision != request.expected_revision {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::StaleRevision,
            );
        }
        if request
            .transition
            .is_some_and(|transition| !transition.is_valid(self.limits))
        {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::InvalidRequest("invalid transition".into()),
            );
        }

        let previous = record.snapshot.clone();
        let mut next = previous.clone();
        let mut released_surfaces = Vec::new();
        let operation = apply_operation(
            &mut next,
            &request.operation,
            self.limits,
            &surface_authorized,
            &mut released_surfaces,
        );
        if let Err(error) = operation {
            let acknowledgement = ContainerAcknowledgement {
                request_id: request.request_id,
                container_id: request.container_id,
                result: Err(error),
            };
            record.remember(acknowledgement.clone());
            return ContainerCommit {
                acknowledgement,
                snapshot: Some(previous),
                before_events: Vec::new(),
                after_events: Vec::new(),
                released_surfaces: Vec::new(),
                replayed: false,
            };
        }
        next.revision = next.revision.next();
        next.transition = request.transition;
        if let Some(limit) = self.budget.snapshot_limit(&next) {
            let acknowledgement = ContainerAcknowledgement {
                request_id: request.request_id,
                container_id: request.container_id,
                result: Err(ContainerRejection::ResourceLimit(limit.into())),
            };
            record.remember(acknowledgement.clone());
            return ContainerCommit {
                acknowledgement,
                snapshot: Some(previous),
                before_events: Vec::new(),
                after_events: Vec::new(),
                released_surfaces: Vec::new(),
                replayed: false,
            };
        }
        let transition_snapshot_bytes =
            snapshot_size(&previous).saturating_add(snapshot_size(&next));
        if request.transition.is_some()
            && transition_snapshot_bytes > self.limits.max_transition_snapshot_bytes
        {
            let acknowledgement = ContainerAcknowledgement {
                request_id: request.request_id,
                container_id: request.container_id,
                result: Err(ContainerRejection::ResourceLimit(
                    "container transition snapshot bytes".into(),
                )),
            };
            record.remember(acknowledgement.clone());
            return ContainerCommit {
                acknowledgement,
                snapshot: Some(previous),
                before_events: Vec::new(),
                after_events: Vec::new(),
                released_surfaces: Vec::new(),
                replayed: false,
            };
        }
        let acknowledgement = applied(
            request.request_id,
            request.container_id,
            next.revision,
            next.active_page().expect("operation keeps an active page"),
            request.transition,
        );
        let (before_events, after_events) = navigation_events(&previous, &next);
        record.snapshot = next.clone();
        record.rebuild_ledger();
        if request.transition.is_some() && previous.active_page_id != next.active_page_id {
            record.ledger.transition_surfaces = [
                previous.active_page().map(|page| page.surface_id),
                next.active_page().map(|page| page.surface_id),
            ]
            .into_iter()
            .flatten()
            .collect();
            record.ledger.transition_snapshot_bytes = transition_snapshot_bytes;
            record.ledger.transition_page = previous.active_page().cloned();
        } else {
            record.ledger.transition_surfaces.clear();
            record.ledger.transition_snapshot_bytes = 0;
            record.ledger.transition_page = None;
        }
        record.remember(acknowledgement.clone());
        ContainerCommit {
            acknowledgement,
            snapshot: Some(next),
            before_events,
            after_events,
            released_surfaces,
            replayed: false,
        }
    }

    pub fn close(
        &self,
        plugin_id: &PluginId,
        worker_id: WorkerId,
        request: CloseContainerRequest,
    ) -> ContainerCommit {
        if !self.budget.admit(plugin_id) {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::ResourceLimit("container transaction rate".into()),
            );
        }
        let mut records = self.records.lock();
        let Some(record) = records.get(&request.container_id) else {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::UnknownContainer,
            );
        };
        if &record.snapshot.plugin_id != plugin_id || record.snapshot.worker_id != worker_id {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::WrongWorker,
            );
        }
        if record.snapshot.revision != request.expected_revision {
            return rejected(
                request.request_id,
                request.container_id,
                ContainerRejection::StaleRevision,
            );
        }
        let record = records
            .remove(&request.container_id)
            .expect("validated container exists");
        let page = record.snapshot.active_page().expect("container has root");
        ContainerCommit {
            acknowledgement: applied(
                request.request_id,
                request.container_id,
                record.snapshot.revision.next(),
                page,
                None,
            ),
            snapshot: None,
            before_events: vec![lifecycle(
                &record.snapshot,
                PageLifecyclePhase::WillDisappear,
            )],
            after_events: vec![ContainerEvent::Dismissed {
                container_id: request.container_id,
                revision: record.snapshot.revision.next(),
            }],
            released_surfaces: record.ledger.pages.values().copied().collect(),
            replayed: false,
        }
    }

    pub fn snapshot(&self, id: ContainerId) -> Option<ContainerSnapshot> {
        self.records
            .lock()
            .get(&id)
            .map(|record| record.snapshot.clone())
    }

    pub fn for_context(
        &self,
        context_id: warpeer_plugin_protocol::ContextId,
        context_revision: Revision,
        slot: warpeer_plugin_protocol::SurfaceSlot,
    ) -> Option<ContainerSnapshot> {
        self.records
            .lock()
            .values()
            .find(|record| {
                record.snapshot.context_id == context_id
                    && record.snapshot.context_revision == context_revision
                    && record.snapshot.slot == slot
            })
            .map(|record| record.snapshot.clone())
    }
}
