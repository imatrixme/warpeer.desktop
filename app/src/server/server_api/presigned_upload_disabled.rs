use std::future::Future;
#[cfg(feature = "local_fs")]
use std::path::PathBuf;

use anyhow::{Result, anyhow};
pub use warp_server_client::HttpStatusError;

#[cfg(feature = "local_fs")]
use super::ai::FileArtifactUploadTargetInfo;
use super::harness_support::UploadTarget;

fn unavailable<T>() -> Result<T> {
    Err(anyhow!(
        "hosted uploads are unavailable in this product profile"
    ))
}

pub trait UploadBody {
    fn length(&self) -> u64;

    #[cfg(not(target_family = "wasm"))]
    fn compute_crc32c_base64(&self) -> impl Future<Output = Result<String>> + Send;

    fn into_reqwest_body(self) -> impl Future<Output = Result<reqwest::Body>> + Send;
}

impl UploadBody for Vec<u8> {
    fn length(&self) -> u64 {
        self.len() as u64
    }

    #[cfg(not(target_family = "wasm"))]
    async fn compute_crc32c_base64(&self) -> Result<String> {
        unavailable()
    }

    async fn into_reqwest_body(self) -> Result<reqwest::Body> {
        Ok(reqwest::Body::from(self))
    }
}

#[cfg(feature = "local_fs")]
#[derive(Debug, Clone)]
pub struct FileUploadBody {
    path: PathBuf,
}

#[cfg(feature = "local_fs")]
impl FileUploadBody {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[cfg(feature = "local_fs")]
impl UploadBody for FileUploadBody {
    fn length(&self) -> u64 {
        std::fs::metadata(&self.path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    }

    async fn compute_crc32c_base64(&self) -> Result<String> {
        unavailable()
    }

    async fn into_reqwest_body(self) -> Result<reqwest::Body> {
        unavailable()
    }
}

pub(crate) async fn upload_to_target(
    http_client: &http_client::Client,
    target: &UploadTarget,
    body: impl UploadBody,
) -> Result<()> {
    let _ = (http_client, target, body.length());
    unavailable()
}

#[cfg(feature = "local_fs")]
pub(crate) async fn upload_file_to_target(
    http_client: &http_client::Client,
    target: &FileArtifactUploadTargetInfo,
    body: impl UploadBody,
) -> Result<String> {
    let _ = (http_client, target, body.length());
    unavailable()
}
