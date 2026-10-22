pub mod manager;
pub mod search_shaping;
pub mod store_client;

use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use string_offset::ByteOffset;
use warp_core::sync_queue::{IsTransientError, SyncQueueTaskTrait};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Hosted codebase indexing is unavailable")]
    Unavailable,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IsTransientError for Error {
    fn is_transient(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NodeHash(String);

impl fmt::Display for NodeHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for NodeHash {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl From<NodeHash> for warp_graphql::full_source_code_embedding::NodeHash {
    fn from(value: NodeHash) -> Self {
        Self(value.0)
    }
}

impl TryFrom<warp_graphql::full_source_code_embedding::NodeHash> for NodeHash {
    type Error = Error;

    fn try_from(
        value: warp_graphql::full_source_code_embedding::NodeHash,
    ) -> Result<Self, Self::Error> {
        Ok(Self(value.0))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn from_content(content: &str) -> Self {
        Self(format!("{:x}", Sha256::digest(content.as_bytes())))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ContentHash {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl From<ContentHash> for warp_graphql::full_source_code_embedding::ContentHash {
    fn from(value: ContentHash) -> Self {
        Self(value.0)
    }
}

impl TryFrom<warp_graphql::full_source_code_embedding::ContentHash> for ContentHash {
    type Error = Error;

    fn try_from(
        value: warp_graphql::full_source_code_embedding::ContentHash,
    ) -> Result<Self, Self::Error> {
        Ok(Self(value.0))
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EmbeddingConfig {
    OpenAiTextSmall3_256,
    VoyageCode3_512,
    Voyage3_5_Lite_512,
    #[default]
    Voyage3_5_512,
    Voyage4_512,
}

impl From<EmbeddingConfig> for warp_graphql::full_source_code_embedding::EmbeddingConfig {
    fn from(value: EmbeddingConfig) -> Self {
        match value {
            EmbeddingConfig::OpenAiTextSmall3_256 => Self::OpenaiTextSmall3256,
            EmbeddingConfig::VoyageCode3_512 => Self::VoyageCode3512,
            EmbeddingConfig::Voyage3_5_Lite_512 => Self::Voyage35Lite512,
            EmbeddingConfig::Voyage3_5_512 => Self::Voyage35512,
            EmbeddingConfig::Voyage4_512 => Self::Voyage4512,
        }
    }
}

impl TryFrom<warp_graphql::full_source_code_embedding::EmbeddingConfig> for EmbeddingConfig {
    type Error = Error;

    fn try_from(
        value: warp_graphql::full_source_code_embedding::EmbeddingConfig,
    ) -> Result<Self, Self::Error> {
        Ok(match value {
            warp_graphql::full_source_code_embedding::EmbeddingConfig::OpenaiTextSmall3256 => {
                Self::OpenAiTextSmall3_256
            }
            warp_graphql::full_source_code_embedding::EmbeddingConfig::VoyageCode3512 => {
                Self::VoyageCode3_512
            }
            warp_graphql::full_source_code_embedding::EmbeddingConfig::Voyage35Lite512 => {
                Self::Voyage3_5_Lite_512
            }
            warp_graphql::full_source_code_embedding::EmbeddingConfig::Voyage35512 => {
                Self::Voyage3_5_512
            }
            warp_graphql::full_source_code_embedding::EmbeddingConfig::Voyage4512 => {
                Self::Voyage4_512
            }
        })
    }
}

#[derive(Clone, Debug)]
pub struct RepoMetadata {
    pub path: Option<String>,
}

impl From<RepoMetadata> for warp_graphql::full_source_code_embedding::RepoMetadata {
    fn from(value: RepoMetadata) -> Self {
        Self { path: value.path }
    }
}

#[derive(Clone, Copy)]
pub struct CodebaseContextConfig {
    pub embedding_config: EmbeddingConfig,
    pub embedding_cadence: Duration,
}

#[derive(Clone)]
pub struct FragmentLocation {
    absolute_path: PathBuf,
    byte_range: Range<ByteOffset>,
}

#[derive(Clone)]
pub struct Fragment {
    content: String,
    content_hash: ContentHash,
    location: FragmentLocation,
}

impl Fragment {
    pub fn from_byte_range(
        content: String,
        content_hash: ContentHash,
        absolute_path: PathBuf,
        byte_range: Range<ByteOffset>,
    ) -> Self {
        Self {
            content,
            content_hash,
            location: FragmentLocation {
                absolute_path,
                byte_range,
            },
        }
    }

    pub fn content_hash(&self) -> &ContentHash {
        &self.content_hash
    }

    pub fn absolute_path(&self) -> &Path {
        &self.location.absolute_path
    }

    pub fn byte_range(&self) -> Range<ByteOffset> {
        self.location.byte_range.clone()
    }
}

impl From<Fragment> for warp_graphql::full_source_code_embedding::Fragment {
    fn from(value: Fragment) -> Self {
        Self {
            content: value.content,
            content_hash: value.content_hash.into(),
        }
    }
}

impl From<Fragment> for warp_graphql::queries::rerank_fragments::RerankFragmentInput {
    fn from(value: Fragment) -> Self {
        Self {
            content: value.content,
            content_hash: value.content_hash.into(),
            location: warp_graphql::queries::rerank_fragments::FragmentLocationInput {
                byte_start: value.location.byte_range.start.as_usize() as i32,
                byte_end: value.location.byte_range.end.as_usize() as i32,
                file_path: value.location.absolute_path.to_string_lossy().into_owned(),
            },
        }
    }
}

impl TryFrom<warp_graphql::queries::rerank_fragments::RerankFragment> for Fragment {
    type Error = Error;

    fn try_from(
        value: warp_graphql::queries::rerank_fragments::RerankFragment,
    ) -> Result<Self, Self::Error> {
        Ok(Self::from_byte_range(
            value.content,
            value.content_hash.try_into()?,
            PathBuf::from(value.location.file_path),
            ByteOffset::from(value.location.byte_start as usize)
                ..ByteOffset::from(value.location.byte_end as usize),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FragmentMetadataLocation {
    pub start_line: usize,
    pub end_line: usize,
    pub byte_range: Range<ByteOffset>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FragmentMetadata {
    pub absolute_path: PathBuf,
    pub location: FragmentMetadataLocation,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RetrievalID(pub(crate) usize);

#[derive(Clone, Copy, Debug)]
pub enum SyncProgress {
    Discovering {
        total_nodes: usize,
    },
    Syncing {
        completed_nodes: usize,
        total_nodes: usize,
    },
}

#[derive(Clone)]
pub struct SnapshotStorage(PathBuf);

impl SnapshotStorage {
    pub fn app_default() -> Option<Self> {
        None
    }

    pub fn from_dir(path: PathBuf) -> Option<Self> {
        Some(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

pub enum SyncTask {
    Disabled,
}

impl SyncQueueTaskTrait for SyncTask {
    type Error = Error;
    type Result = ();
    type Fut = std::future::Ready<Result<(), Error>>;

    fn run(&mut self) -> Self::Fut {
        std::future::ready(Err(Error::Unavailable))
    }
}
