use std::collections::{HashMap, HashSet};

use futures::StreamExt as _;
use warpui::{EntityId, ModelContext};

use super::{WorktreeAgentCoordinator, WorktreeAgentCoordinatorEvent, WorktreeMetadata};
use crate::worktree_agents::catalog::{WorktreeDirtyState, WorktreeMenuEntry};
use crate::worktree_agents::catalog_dirty::{DirtyProbeResult, query_dirty};
use crate::worktree_agents::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};

const DIRTY_PROBE_CONCURRENCY: usize = 4;

impl WorktreeAgentCoordinator {
    pub(crate) fn publish_catalog(
        &mut self,
        terminal_view_id: EntityId,
        host: ExecutionHostKey,
        entries: &[WorktreeMenuEntry],
        ctx: &mut ModelContext<Self>,
    ) {
        let Some(common_dir) = entries
            .iter()
            .map(|entry| entry.common_dir.as_str())
            .find(|common_dir| !common_dir.is_empty())
        else {
            return;
        };
        let repository = RepositoryKey::new(host, common_dir);
        let previous = self.repository_metadata(&repository);
        let mut current_worktree = None;
        let mut next_keys = HashSet::new();
        for entry in entries {
            let key = WorktreeKey::new(repository.clone(), &entry.path);
            next_keys.insert(key.clone());
            let dirty = previous
                .get(&key)
                .filter(|metadata| same_catalog_metadata(metadata, entry))
                .map_or(WorktreeDirtyState::Unknown, |metadata| metadata.dirty);
            self.worktree_metadata.insert(
                key.clone(),
                WorktreeMetadata {
                    branch: entry.branch.clone(),
                    detached_head: entry.detached.then(|| entry.head.clone()).flatten(),
                    locked: entry.locked,
                    available: !entry.prunable,
                    dirty,
                },
            );
            if entry.is_current {
                current_worktree = Some(key);
            }
        }
        self.worktree_metadata
            .retain(|key, _| key.repository() != &repository || next_keys.contains(key));

        let Some(current_worktree) = current_worktree else {
            return;
        };
        let catalog_changed = previous != self.repository_metadata(&repository);
        let generation = if catalog_changed {
            let generation = self
                .catalog_generations
                .get(&repository)
                .copied()
                .unwrap_or_default()
                .wrapping_add(1)
                .max(1);
            self.catalog_generations
                .insert(repository.clone(), generation);
            generation
        } else {
            self.catalog_generations
                .get(&repository)
                .copied()
                .unwrap_or(1)
        };
        let pane_changed = self.pane_worktrees.get(&terminal_view_id) != Some(&current_worktree);
        let preserves_bound_agent = self
            .pane_agent_keys
            .get(&terminal_view_id)
            .is_some_and(|key| key.worktree == current_worktree);
        if pane_changed && !preserves_bound_agent {
            self.remove_pane_agent(terminal_view_id);
        }
        self.pane_worktrees
            .insert(terminal_view_id, current_worktree.clone());
        self.pane_index
            .publish_worktree(terminal_view_id, current_worktree);
        self.materialize_pending_agent(terminal_view_id);
        self.start_dirty_probes(repository, generation, ctx);
        self.start_herdr_polling(ctx);
        self.start_host_polling(ctx);
        if catalog_changed || pane_changed {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }

    pub(crate) fn clear_catalog(
        &mut self,
        terminal_view_id: EntityId,
        ctx: &mut ModelContext<Self>,
    ) {
        let removed_worktree = self.pane_worktrees.remove(&terminal_view_id).is_some();
        let removed_index = self.pane_index.remove(terminal_view_id);
        let removed_agent = self.remove_pane_agent(terminal_view_id);
        if removed_worktree || removed_index || removed_agent {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }

    pub(crate) fn menu_entries_for_terminal(
        &self,
        terminal_view_id: EntityId,
    ) -> Vec<WorktreeMenuEntry> {
        let Some(current) = self.pane_worktrees.get(&terminal_view_id) else {
            return Vec::new();
        };
        let repository = current.repository();
        let now_ms = self.now_ms();
        let mut entries: Vec<_> = self
            .worktree_metadata
            .iter()
            .filter(|(key, _)| key.repository() == repository)
            .map(|(key, metadata)| WorktreeMenuEntry {
                common_dir: repository.common_dir().to_string_lossy().into_owned(),
                path: key.path().to_string_lossy().into_owned(),
                branch: metadata.branch.clone(),
                head: metadata.detached_head.clone(),
                is_current: key == current,
                detached: metadata.detached_head.is_some(),
                locked: metadata.locked,
                prunable: !metadata.available,
                dirty: metadata.dirty,
                agent_count: self
                    .observations
                    .values()
                    .filter(|observation| {
                        observation.key.worktree == *key && observation.expires_at_ms > now_ms
                    })
                    .count(),
            })
            .collect();
        entries.sort_by(|left, right| {
            right
                .is_current
                .cmp(&left.is_current)
                .then_with(|| left.path.cmp(&right.path))
        });
        entries
    }

    fn repository_metadata(
        &self,
        repository: &RepositoryKey,
    ) -> HashMap<WorktreeKey, WorktreeMetadata> {
        self.worktree_metadata
            .iter()
            .filter(|(key, _)| key.repository() == repository)
            .map(|(key, metadata)| (key.clone(), metadata.clone()))
            .collect()
    }

    fn start_dirty_probes(
        &mut self,
        repository: RepositoryKey,
        generation: u64,
        ctx: &mut ModelContext<Self>,
    ) {
        if !can_probe_dirty(repository.host())
            || self.dirty_probe_in_flight.contains_key(&repository)
        {
            return;
        }
        let paths: Vec<_> = self
            .worktree_metadata
            .iter()
            .filter(|(key, metadata)| {
                key.repository() == &repository
                    && metadata.available
                    && metadata.dirty == WorktreeDirtyState::Unknown
            })
            .map(|(key, _)| key.path().to_path_buf())
            .collect();
        if paths.is_empty() {
            return;
        }
        self.dirty_probe_in_flight
            .insert(repository.clone(), generation);
        ctx.spawn(
            async move {
                let results = futures::stream::iter(paths.into_iter().map(|path| async move {
                    let result = query_dirty(&path).await;
                    (path, result)
                }))
                .buffer_unordered(DIRTY_PROBE_CONCURRENCY)
                .collect::<Vec<_>>()
                .await;
                (repository, generation, results)
            },
            |coordinator, (repository, generation, results), ctx| {
                if coordinator.dirty_probe_in_flight.get(&repository) == Some(&generation) {
                    coordinator.dirty_probe_in_flight.remove(&repository);
                }
                if !accepts_dirty_generation(
                    coordinator.catalog_generations.get(&repository).copied(),
                    generation,
                ) {
                    let next_generation = coordinator
                        .catalog_generations
                        .get(&repository)
                        .copied()
                        .unwrap_or_default();
                    coordinator.start_dirty_probes(repository, next_generation, ctx);
                    return;
                }
                let mut changed = false;
                for (path, result) in results {
                    let key = WorktreeKey::new(repository.clone(), path);
                    let Some(metadata) = coordinator.worktree_metadata.get_mut(&key) else {
                        continue;
                    };
                    let dirty = match result {
                        DirtyProbeResult::Clean => WorktreeDirtyState::Clean,
                        DirtyProbeResult::Dirty => WorktreeDirtyState::Dirty,
                        DirtyProbeResult::Unavailable => WorktreeDirtyState::Unavailable,
                    };
                    if metadata.dirty != dirty {
                        metadata.dirty = dirty;
                        changed = true;
                    }
                }
                if changed {
                    ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
                    ctx.notify();
                }
            },
        );
    }
}

fn same_catalog_metadata(metadata: &WorktreeMetadata, entry: &WorktreeMenuEntry) -> bool {
    metadata.branch == entry.branch
        && metadata.detached_head == entry.detached.then(|| entry.head.clone()).flatten()
        && metadata.locked == entry.locked
        && metadata.available != entry.prunable
}

fn can_probe_dirty(host: &ExecutionHostKey) -> bool {
    matches!(host, ExecutionHostKey::Local)
}

fn accepts_dirty_generation(current: Option<u64>, incoming: u64) -> bool {
    current == Some(incoming)
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
