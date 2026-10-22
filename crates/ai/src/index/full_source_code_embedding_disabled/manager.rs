use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use warpui_core::{AppContext, Entity, ModelContext, SingletonEntity};

use super::store_client::StoreClient;
use super::{ContentHash, FragmentMetadata, NodeHash, RetrievalID, SnapshotStorage, SyncProgress};
use crate::index::locations::CodeContextLocation;
use crate::workspace::{WorkspaceMetadata, WorkspaceMetadataEvent};

pub enum CodebaseIndexFinishedStatus {
    Completed,
    Failed(CodebaseIndexingError),
}

#[derive(Debug, Error)]
pub enum RetrieveFileError {
    #[error("Codebase index still indexing")]
    IndexSyncing,
    #[error("Codebase index failed: {0:#}")]
    IndexFailed(CodebaseIndexingError),
    #[error("Codebase index not found")]
    IndexNotFound,
}

#[derive(Debug, Error)]
pub enum FragmentMetadataLookupError {
    #[error("Codebase index not found")]
    IndexNotFound,
    #[error("Codebase index has no synced root hash")]
    IndexNotSynced,
    #[error("Codebase index root hash mismatch")]
    RootHashMismatch {
        requested: NodeHash,
        current: NodeHash,
    },
}

#[derive(Debug)]
pub enum CodebaseIndexManagerEvent {
    RetrievalRequestCompleted {
        retrieval_id: RetrievalID,
        fragments: Arc<HashSet<CodeContextLocation>>,
        out_of_sync_delay: Option<Duration>,
    },
    RetrievalRequestFailed {
        retrieval_id: RetrievalID,
        error_message: String,
    },
    SyncStateUpdated {
        root_path: PathBuf,
    },
    IndexMetadataUpdated {
        root_path: PathBuf,
        event: WorkspaceMetadataEvent,
    },
    RemoveExpiredIndexMetadata {
        expired_metadata: Arc<Vec<PathBuf>>,
    },
    NewIndexCreated {
        root_path: PathBuf,
    },
}

#[derive(Debug, Error)]
pub enum CodebaseIndexingError {
    #[error("Build tree error")]
    BuildTreeError,
    #[error("Repo size exceeded max file limit")]
    ExceededMaxFileLimit,
    #[error("Maximum directory depth exceeded")]
    MaxDepthExceeded,
    #[error("Hosted codebase indexing is unavailable")]
    Unavailable,
}

pub struct CodebaseIndexStatus;

impl CodebaseIndexStatus {
    pub fn has_pending(&self) -> bool {
        false
    }

    pub fn has_synced_version(&self) -> bool {
        false
    }

    pub fn last_sync_successful(&self) -> Option<bool> {
        None
    }

    pub fn last_sync_result(&self) -> Option<&CodebaseIndexFinishedStatus> {
        None
    }

    pub fn sync_progress(&self) -> Option<&SyncProgress> {
        None
    }

    pub fn root_hash(&self) -> Option<&NodeHash> {
        None
    }
}

pub enum BuildSource<'a> {
    FromPath(&'a Path),
    FromPersistedMetadata(WorkspaceMetadata),
}

pub struct CodebaseIndexManagerConfig;

impl CodebaseIndexManagerConfig {
    pub fn new(
        _persisted_index_metadata: Vec<WorkspaceMetadata>,
        _max_index_count: Option<usize>,
        _max_files_repo_limit: usize,
        _embedding_generation_batch_size: usize,
        _store_client: Arc<dyn StoreClient>,
        _indexing_enabled: bool,
    ) -> Self {
        Self
    }

    pub fn defer_persisted_index_restore(self) -> Self {
        self
    }
}

pub struct CodebaseIndexManager;

impl CodebaseIndexManager {
    pub fn new(
        _persisted_index_metadata: Vec<WorkspaceMetadata>,
        _max_index_count: Option<usize>,
        _max_files_repo_limit: usize,
        _embedding_generation_batch_size: usize,
        _store_client: Arc<dyn StoreClient>,
        _indexing_enabled: bool,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    pub fn new_with_config(
        _config: CodebaseIndexManagerConfig,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    pub fn new_with_snapshot_storage(
        _config: CodebaseIndexManagerConfig,
        _snapshot_storage: Option<SnapshotStorage>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    #[cfg(feature = "test-util")]
    pub fn new_for_test(
        _store_client: Arc<dyn StoreClient>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    pub fn clean_up_deleted_indices(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn drop_index(&mut self, _root_path: PathBuf, _ctx: &mut ModelContext<Self>) {}

    pub fn handle_active_session_changed(&mut self, _active_directory: &Path) {}

    pub fn update_max_limits(
        &mut self,
        _new_max_indices: Option<usize>,
        _new_max_files_per_repo: usize,
        _new_embedding_generation_batch_size: usize,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn can_create_new_indices(&self) -> bool {
        false
    }

    pub fn handle_session_bootstrapped(&mut self, _working_directory: &Path) {}

    pub fn get_codebase_index_statuses<'a>(
        &'a self,
        _app: &'a AppContext,
    ) -> impl Iterator<Item = (&'a PathBuf, CodebaseIndexStatus)> {
        std::iter::empty()
    }

    pub fn get_codebase_index_status_for_path(
        &self,
        _root_path: &Path,
        _app: &AppContext,
    ) -> Option<CodebaseIndexStatus> {
        None
    }

    pub fn fragment_metadatas_from_hashes(
        &self,
        _repo_path: &Path,
        _root_hash: &NodeHash,
        _content_hashes: &[ContentHash],
        _app: &AppContext,
    ) -> Result<HashMap<ContentHash, Vec<FragmentMetadata>>, FragmentMetadataLookupError> {
        Err(FragmentMetadataLookupError::IndexNotFound)
    }

    pub fn get_codebase_paths(&self) -> impl Iterator<Item = &PathBuf> {
        std::iter::empty()
    }

    pub fn num_active_indices(&self) -> usize {
        0
    }

    pub fn is_indexing_enabled(&self) -> bool {
        false
    }

    pub fn start_persisted_index_restore(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn index_directory(&mut self, _directory: PathBuf, _ctx: &mut ModelContext<Self>) -> bool {
        false
    }

    pub fn build_and_sync_codebase_index(
        &mut self,
        _build_source: BuildSource<'_>,
        _ctx: &mut ModelContext<Self>,
    ) -> bool {
        false
    }

    pub fn reset_codebase_indexing(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn root_path_for_codebase(&self, _path: &Path) -> Option<PathBuf> {
        None
    }

    pub fn with_indexed_codebase<T>(
        &mut self,
        path: &Path,
        _on_found: impl FnOnce(&mut Self, &Path, &mut ModelContext<Self>) -> T,
        on_missing: impl FnOnce(&mut Self, &Path, &mut ModelContext<Self>) -> T,
        ctx: &mut ModelContext<Self>,
    ) -> T {
        on_missing(self, path, ctx)
    }

    pub fn try_manual_resync_codebase(&self, _repo_path: &Path, _ctx: &mut ModelContext<Self>) {}

    pub fn retrieve_relevant_files(
        &self,
        _query: String,
        _repo_path: &Path,
        _ctx: &mut ModelContext<Self>,
    ) -> Result<RetrievalID, RetrieveFileError> {
        Err(RetrieveFileError::IndexNotFound)
    }

    pub fn abort_retrieval_request(
        &self,
        _repo_path: &Path,
        _retrieval_id: RetrievalID,
        _ctx: &mut ModelContext<Self>,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    pub fn write_snapshot(&mut self, _working_directory: &Path, _ctx: &mut ModelContext<Self>) {}

    pub fn trigger_incremental_sync_for_path(
        &mut self,
        _directory_path: &Path,
        _ctx: &mut ModelContext<Self>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Entity for CodebaseIndexManager {
    type Event = CodebaseIndexManagerEvent;
}

impl SingletonEntity for CodebaseIndexManager {}
