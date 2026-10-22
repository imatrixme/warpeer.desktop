use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::client::TaskIdentityToken;

const GCP_WORKLOAD_IDENTITY_FEDERATION_VERSION: u8 = 1;
pub(crate) const TOKEN_TYPE_ID_TOKEN: &str = "urn:ietf:params:oauth:token-type:id_token";
pub(crate) const TOKEN_TYPE_JWT: &str = "urn:ietf:params:oauth:token-type:jwt";

#[derive(Debug, Clone)]
pub struct GcpFederationConfig {
    pub project_number: String,
    pub pool_id: String,
    pub provider_id: String,
    pub service_account_email: Option<String>,
    pub token_lifetime: Option<Duration>,
}

pub struct GcpCredentials;

impl GcpCredentials {
    pub fn federated(
        _task_id: &str,
        _config: &GcpFederationConfig,
    ) -> Result<Self, PrepareGcpCredentialsError> {
        Err(PrepareGcpCredentialsError::Unavailable)
    }

    pub fn config_file_path(&self) -> &Path {
        Path::new("")
    }

    pub fn env_vars(&self) -> HashMap<OsString, OsString> {
        HashMap::new()
    }

    pub fn cleanup(self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrepareGcpCredentialsError {
    #[error("GCP workload identity credentials are unavailable in this product profile")]
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GcpWorkloadIdentityFederationToken {
    pub version: u8,
    pub success: bool,
    pub token_type: String,
    pub id_token: String,
    pub expiration_time: i64,
}

impl GcpWorkloadIdentityFederationToken {
    pub(crate) fn new(token: TaskIdentityToken, token_type: String) -> Self {
        Self {
            version: GCP_WORKLOAD_IDENTITY_FEDERATION_VERSION,
            success: true,
            token_type,
            id_token: token.token,
            expiration_time: token.expires_at.timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GcpWorkloadIdentityFederationError {
    pub version: u8,
    pub success: bool,
    pub code: String,
    pub message: String,
}

impl GcpWorkloadIdentityFederationError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            version: GCP_WORKLOAD_IDENTITY_FEDERATION_VERSION,
            success: false,
            code: "TOKEN_ISSUANCE_FAILED".into(),
            message: message.into(),
        }
    }
}

pub(crate) fn gcp_workload_identity_federation_audience(
    project_number: &str,
    pool_id: &str,
    provider_id: &str,
) -> String {
    format!(
        "//iam.googleapis.com/projects/{project_number}/locations/global/workloadIdentityPools/{pool_id}/providers/{provider_id}"
    )
}
