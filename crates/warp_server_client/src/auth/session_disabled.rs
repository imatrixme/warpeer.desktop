use std::fmt;
use std::result::Result as StdResult;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use instant::Duration;
use warp_server_auth::auth_state::AuthState;
use warp_server_auth::credentials::{AuthToken, Credentials, FirebaseToken, LoginToken};

use super::UserAuthenticationError;

#[derive(Clone)]
pub enum AuthEvent {
    StagingAccessBlocked,
    NeedsReauth,
    UserAccountDisabled,
    AccessTokenRefreshed {
        #[cfg_attr(target_family = "wasm", allow(dead_code))]
        token: String,
    },
    IapChallengeReceived,
}

impl fmt::Debug for AuthEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StagingAccessBlocked => f.write_str("StagingAccessBlocked"),
            Self::NeedsReauth => f.write_str("NeedsReauth"),
            Self::UserAccountDisabled => f.write_str("UserAccountDisabled"),
            Self::AccessTokenRefreshed { .. } => f
                .debug_struct("AccessTokenRefreshed")
                .field("token", &"<redacted>")
                .finish(),
            Self::IapChallengeReceived => f.write_str("IapChallengeReceived"),
        }
    }
}

pub struct AuthSession {
    auth_state: Arc<AuthState>,
}

impl AuthSession {
    pub fn new(
        client: Arc<http_client::Client>,
        auth_state: Arc<AuthState>,
        event_sender: async_channel::Sender<AuthEvent>,
    ) -> Self {
        drop(client);
        drop(event_sender);
        Self { auth_state }
    }

    pub fn allowed_to_refresh_token(&self) -> bool {
        false
    }

    pub async fn get_or_refresh_access_token(&self) -> Result<AuthToken> {
        let _ = &self.auth_state;
        bail!("hosted authentication is unavailable in this product profile")
    }

    pub async fn exchange_credentials(
        &self,
        token: LoginToken,
    ) -> StdResult<Credentials, UserAuthenticationError> {
        drop(token);
        Err(Self::unavailable())
    }

    pub async fn request_device_code(
        &self,
    ) -> StdResult<oauth2::StandardDeviceAuthorizationResponse, UserAuthenticationError> {
        Err(Self::unavailable())
    }

    pub async fn exchange_device_access_token(
        &self,
        details: &oauth2::StandardDeviceAuthorizationResponse,
        timeout: Duration,
    ) -> StdResult<FirebaseToken, UserAuthenticationError> {
        let _ = details;
        let _ = timeout;
        Err(Self::unavailable())
    }

    fn unavailable() -> UserAuthenticationError {
        UserAuthenticationError::Unexpected(anyhow!(
            "hosted authentication is unavailable in this product profile"
        ))
    }
}
