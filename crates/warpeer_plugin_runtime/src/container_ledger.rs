use std::collections::{BTreeMap, BTreeSet, VecDeque};

use warpeer_plugin_protocol::{
    ContainerAcknowledgement, ContainerPage, ContainerSnapshot, PageId, RequestId,
    ResourceHandleId, SurfaceId,
};

use crate::container_reducer::{retained_pixels, snapshot_size};

const MAX_PROCESSED_REQUESTS: usize = 128;

#[derive(Clone, Debug, Default)]
pub struct ContainerResourceLedger {
    pub pages: BTreeMap<PageId, SurfaceId>,
    pub retained_frames: BTreeSet<SurfaceId>,
    pub transition_surfaces: BTreeSet<SurfaceId>,
    pub transition_page: Option<ContainerPage>,
    pub retained_pixels: u64,
    pub snapshot_bytes: u64,
    pub transition_snapshot_bytes: u64,
    pub action_ids: BTreeSet<String>,
    pub capability_handles: BTreeSet<ResourceHandleId>,
    pub drag_capture: bool,
}

impl ContainerResourceLedger {
    pub(crate) fn rebuild_snapshot_resources(&mut self, snapshot: &ContainerSnapshot) {
        self.pages = snapshot
            .pages
            .iter()
            .map(|page| (page.id, page.surface_id))
            .collect();
        self.retained_frames = self.pages.values().copied().collect();
        self.action_ids = snapshot
            .active_page()
            .into_iter()
            .flat_map(|page| page.chrome.leading.iter().chain(&page.chrome.trailing))
            .flat_map(|group| group.actions.iter().map(|action| action.id.clone()))
            .collect();
        self.retained_pixels = retained_pixels(snapshot);
        self.snapshot_bytes = snapshot_size(snapshot);
    }
}

pub(crate) struct ContainerRecord {
    pub snapshot: ContainerSnapshot,
    pub processed: VecDeque<(RequestId, ContainerAcknowledgement)>,
    pub ledger: ContainerResourceLedger,
}

impl ContainerRecord {
    pub fn remember(&mut self, acknowledgement: ContainerAcknowledgement) {
        if self.processed.len() == MAX_PROCESSED_REQUESTS {
            self.processed.pop_front();
        }
        self.processed
            .push_back((acknowledgement.request_id, acknowledgement));
    }

    pub fn acknowledgement(&self, request_id: RequestId) -> Option<ContainerAcknowledgement> {
        self.processed
            .iter()
            .find(|(candidate, _)| *candidate == request_id)
            .map(|(_, acknowledgement)| acknowledgement.clone())
    }

    pub fn rebuild_ledger(&mut self) {
        self.ledger.rebuild_snapshot_resources(&self.snapshot);
    }
}
