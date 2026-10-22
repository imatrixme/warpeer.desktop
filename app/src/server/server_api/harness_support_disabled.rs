use std::collections::HashMap;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use super::ServerApi;
#[cfg(feature = "local_fs")]
pub use super::presigned_upload::FileUploadBody;
pub use super::presigned_upload::UploadBody;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::artifacts::Artifact;

fn unavailable<T>() -> Result<T> {
    Err(anyhow!(
        "hosted agent harness support is unavailable in this product profile"
    ))
}

#[serde_with::serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadTarget {
    pub url: String,
    pub method: String,
    #[serde(default)]
    #[serde_as(deserialize_as = "serde_with::DefaultOnNull")]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    #[serde_as(deserialize_as = "serde_with::DefaultOnNull")]
    pub fields: Vec<UploadField>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadField {
    pub name: String,
    pub value: UploadFieldValue,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UploadFieldValue {
    Static {
        value: String,
    },
    #[serde(rename = "content_crc32c")]
    ContentCrc32C,
    ContentData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotUploadMode {
    #[default]
    Legacy,
    Checkpoint,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotUploadRequest {
    #[serde(skip_serializing_if = "is_default_mode")]
    pub mode: SnapshotUploadMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation: Option<String>,
    pub files: Vec<SnapshotFileInfo>,
}

fn is_default_mode(mode: &SnapshotUploadMode) -> bool {
    *mode == SnapshotUploadMode::default()
}

impl SnapshotUploadRequest {
    pub fn legacy(files: Vec<SnapshotFileInfo>) -> Self {
        Self {
            mode: SnapshotUploadMode::Legacy,
            generation: None,
            files,
        }
    }

    pub fn checkpoint(generation: CheckpointGeneration, files: Vec<SnapshotFileInfo>) -> Self {
        Self {
            mode: SnapshotUploadMode::Checkpoint,
            generation: Some(generation.into_inner()),
            files,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct CheckpointGeneration(String);

impl CheckpointGeneration {
    pub(crate) fn from_validated(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for CheckpointGeneration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CommitSnapshotRequest {
    pub generation: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CommitSnapshotResponse {
    pub generation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotFileInfo {
    pub filename: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SnapshotUploadResponse {
    pub uploads: Vec<UploadTarget>,
}

#[derive(serde::Serialize)]
pub struct ResolvePromptAttachedSkill {
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ResolvePromptRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<ResolvePromptAttachedSkill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments_dir: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct ResolvedHarnessPrompt {
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub resumption_prompt: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ReportArtifactResponse {
    pub artifact_uid: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ShutdownError {
    category: String,
    message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ReportShutdownRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ShutdownError>,
}

impl ReportShutdownRequest {
    pub fn clean() -> Self {
        Self { error: None }
    }

    pub fn abnormal(category: String, message: String) -> Self {
        Self {
            error: Some(ShutdownError { category, message }),
        }
    }
}

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait HarnessSupportClient: 'static + Send + Sync {
    async fn create_external_conversation(&self, format: &str) -> Result<AIConversationId>;
    async fn get_transcript_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget>;
    async fn get_block_snapshot_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> Result<UploadTarget>;
    async fn resolve_prompt(&self, request: ResolvePromptRequest) -> Result<ResolvedHarnessPrompt>;
    async fn report_artifact(&self, artifact: &Artifact) -> Result<ReportArtifactResponse>;
    async fn notify_user(&self, message: &str) -> Result<()>;
    async fn finish_task(&self, success: bool, summary: &str) -> Result<()>;
    async fn report_clean_shutdown(&self) -> Result<()>;
    async fn report_error_shutdown(
        &self,
        error_category: String,
        error_message: String,
    ) -> Result<()>;
    async fn get_snapshot_upload_targets(
        &self,
        request: &SnapshotUploadRequest,
    ) -> Result<Vec<UploadTarget>>;
    async fn commit_snapshot(
        &self,
        request: &CommitSnapshotRequest,
    ) -> Result<CommitSnapshotResponse>;
    async fn fetch_transcript(&self) -> Result<bytes::Bytes>;
    fn http_client(&self) -> &http_client::Client;
}

impl ServerApi {
    pub(crate) async fn get_public_api_response_for_task(
        &self,
        task_id: &AmbientAgentTaskId,
        path: &str,
    ) -> Result<http_client::Response> {
        let _ = (task_id, path);
        unavailable()
    }

    pub(crate) async fn post_public_api_response_for_task<B>(
        &self,
        task_id: &AmbientAgentTaskId,
        path: &str,
        body: &B,
    ) -> Result<http_client::Response>
    where
        B: serde::Serialize,
    {
        let _ = (task_id, path, body);
        unavailable()
    }

    pub(crate) async fn resolve_prompt_for_task(
        &self,
        task_id: &AmbientAgentTaskId,
        request: ResolvePromptRequest,
    ) -> Result<ResolvedHarnessPrompt> {
        let _ = (task_id, request);
        unavailable()
    }

    pub(crate) async fn fetch_transcript_for_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> Result<bytes::Bytes> {
        let _ = task_id;
        unavailable()
    }
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl HarnessSupportClient for ServerApi {
    async fn create_external_conversation(&self, _format: &str) -> Result<AIConversationId> {
        unavailable()
    }

    async fn get_transcript_upload_target(
        &self,
        _conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        unavailable()
    }

    async fn get_block_snapshot_upload_target(
        &self,
        _conversation_id: &AIConversationId,
    ) -> Result<UploadTarget> {
        unavailable()
    }

    async fn resolve_prompt(
        &self,
        _request: ResolvePromptRequest,
    ) -> Result<ResolvedHarnessPrompt> {
        unavailable()
    }

    async fn report_artifact(&self, _artifact: &Artifact) -> Result<ReportArtifactResponse> {
        unavailable()
    }

    async fn notify_user(&self, _message: &str) -> Result<()> {
        unavailable()
    }

    async fn finish_task(&self, _success: bool, _summary: &str) -> Result<()> {
        unavailable()
    }

    async fn report_clean_shutdown(&self) -> Result<()> {
        unavailable()
    }

    async fn report_error_shutdown(
        &self,
        _error_category: String,
        _error_message: String,
    ) -> Result<()> {
        unavailable()
    }

    async fn get_snapshot_upload_targets(
        &self,
        _request: &SnapshotUploadRequest,
    ) -> Result<Vec<UploadTarget>> {
        unavailable()
    }

    async fn commit_snapshot(
        &self,
        _request: &CommitSnapshotRequest,
    ) -> Result<CommitSnapshotResponse> {
        unavailable()
    }

    async fn fetch_transcript(&self) -> Result<bytes::Bytes> {
        unavailable()
    }

    fn http_client(&self) -> &http_client::Client {
        self.base_client.http_client()
    }
}

pub async fn upload_to_target(
    _http_client: &http_client::Client,
    _target: &UploadTarget,
    _body: impl UploadBody,
) -> Result<()> {
    unavailable()
}
