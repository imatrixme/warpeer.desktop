use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use warpeer_plugin_protocol::{ContainerLimits, ContainerSnapshot, PluginId};

use crate::container_reducer::{retained_pixels, snapshot_size};

pub(crate) struct ContainerBudgetTracker {
    messages: Mutex<HashMap<PluginId, VecDeque<Instant>>>,
    limits: ContainerLimits,
}

impl ContainerBudgetTracker {
    pub fn new(limits: ContainerLimits) -> Self {
        Self {
            messages: Mutex::new(HashMap::new()),
            limits,
        }
    }

    pub fn admit(&self, plugin_id: &PluginId) -> bool {
        let now = Instant::now();
        let mut all_messages = self.messages.lock();
        let messages = all_messages.entry(plugin_id.clone()).or_default();
        let threshold = now - Duration::from_secs(1);
        while messages.front().is_some_and(|time| *time < threshold) {
            messages.pop_front();
        }
        if messages.len() >= usize::from(self.limits.max_transactions_per_second) {
            return false;
        }
        messages.push_back(now);
        true
    }

    pub fn snapshot_limit(&self, snapshot: &ContainerSnapshot) -> Option<&'static str> {
        if snapshot.pages.len() > usize::from(self.limits.max_retained_surfaces) {
            Some("retained container surfaces")
        } else if retained_pixels(snapshot) > self.limits.max_retained_pixels {
            Some("retained container pixels")
        } else if snapshot_size(snapshot) > self.limits.max_snapshot_bytes {
            Some("container snapshot bytes")
        } else {
            None
        }
    }

    pub fn remove_plugin(&self, plugin_id: &PluginId) {
        self.messages.lock().remove(plugin_id);
    }
}
