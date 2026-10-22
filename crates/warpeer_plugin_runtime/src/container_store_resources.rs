use std::collections::HashMap;

use warpeer_plugin_protocol::{
    ContainerId, ContainerSnapshot, ContextId, PluginId, ResourceHandleId, Revision, SurfaceId,
    SurfacePoint,
};

use crate::container_ledger::ContainerRecord;
use crate::{ContainerResourceLedger, ContainerStore};

impl ContainerStore {
    pub fn for_surface(&self, surface_id: SurfaceId) -> Option<ContainerSnapshot> {
        self.records
            .lock()
            .values()
            .find(|record| {
                record
                    .snapshot
                    .pages
                    .iter()
                    .any(|page| page.surface_id == surface_id)
            })
            .map(|record| record.snapshot.clone())
    }

    pub fn set_origin(&self, id: ContainerId, origin: SurfacePoint) -> Option<ContainerSnapshot> {
        let mut records = self.records.lock();
        let record = records.get_mut(&id)?;
        record.snapshot.origin = origin;
        Some(record.snapshot.clone())
    }

    pub fn set_drag_capture(&self, id: ContainerId, captured: bool) -> bool {
        let mut records = self.records.lock();
        let Some(record) = records.get_mut(&id) else {
            return false;
        };
        record.ledger.drag_capture = captured;
        true
    }

    pub fn attach_capability_handle(
        &self,
        context_id: ContextId,
        handle: ResourceHandleId,
    ) -> bool {
        let mut records = self.records.lock();
        let Some(record) = records
            .values_mut()
            .find(|record| record.snapshot.context_id == context_id)
        else {
            return false;
        };
        record.ledger.capability_handles.insert(handle)
    }

    pub fn release_capability_handle(
        &self,
        plugin_id: &PluginId,
        handle: ResourceHandleId,
    ) -> bool {
        self.records
            .lock()
            .values_mut()
            .filter(|record| &record.snapshot.plugin_id == plugin_id)
            .any(|record| record.ledger.capability_handles.remove(&handle))
    }

    pub fn complete_transition(&self, id: ContainerId, revision: Revision) -> bool {
        let mut records = self.records.lock();
        let Some(record) = records.get_mut(&id) else {
            return false;
        };
        if record.snapshot.revision != revision {
            return false;
        }
        record.ledger.transition_surfaces.clear();
        record.ledger.transition_snapshot_bytes = 0;
        record.ledger.transition_page = None;
        true
    }

    pub fn remove_plugin(&self, plugin_id: &PluginId) -> Vec<SurfaceId> {
        self.budget.remove_plugin(plugin_id);
        let mut records = self.records.lock();
        let ids = records
            .iter()
            .filter_map(|(id, record)| (&record.snapshot.plugin_id == plugin_id).then_some(*id))
            .collect::<Vec<_>>();
        remove_records(&mut records, ids)
    }

    pub fn remove_context(&self, context_id: ContextId) -> Vec<SurfaceId> {
        let mut records = self.records.lock();
        let ids = records
            .iter()
            .filter_map(|(id, record)| (record.snapshot.context_id == context_id).then_some(*id))
            .collect::<Vec<_>>();
        remove_records(&mut records, ids)
    }

    pub fn remove_surface(&self, surface_id: SurfaceId) -> Vec<SurfaceId> {
        let mut records = self.records.lock();
        let ids = records
            .iter()
            .filter_map(|(id, record)| {
                record
                    .ledger
                    .pages
                    .values()
                    .any(|candidate| *candidate == surface_id)
                    .then_some(*id)
            })
            .collect::<Vec<_>>();
        remove_records(&mut records, ids)
    }

    pub fn ledger(&self, id: ContainerId) -> Option<ContainerResourceLedger> {
        self.records
            .lock()
            .get(&id)
            .map(|record| record.ledger.clone())
    }

    pub fn len(&self) -> usize {
        self.records.lock().len()
    }
}

fn remove_records(
    records: &mut HashMap<ContainerId, ContainerRecord>,
    ids: Vec<ContainerId>,
) -> Vec<SurfaceId> {
    ids.into_iter()
        .flat_map(|id| {
            records
                .remove(&id)
                .into_iter()
                .flat_map(|record| record.ledger.pages.into_values())
        })
        .collect()
}
