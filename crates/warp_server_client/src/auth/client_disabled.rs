use std::result::Result as StdResult;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use instant::Duration;
use warp_graphql::mutations::create_anonymous_user::{
    AnonymousUserType, CreateAnonymousUserResult,
};
use warp_graphql::mutations::expire_api_key::ExpireApiKeyResult;
use warp_graphql::mutations::generate_api_key::GenerateApiKeyResult;
use warp_graphql::mutations::mint_custom_token::MintCustomTokenResult;
use warp_graphql::mutations::update_user_settings::UpdateUserSettingsInput;
use warp_graphql::queries::api_keys::ApiKeyProperties;
use warp_graphql::queries::get_user::UserOutput as GqlUserOutput;
use warp_server_auth::credentials::{AuthToken, FirebaseToken, LoginToken};

use super::{
    AgentIdentity, AuthClient, AuthClientImpl, FetchUserResult, MintCustomTokenError,
    SyncedUserSettings, UserAuthenticationError,
};
use crate::ids::ApiKeyUid;

fn unavailable<T>() -> Result<T> {
    Err(anyhow!(
        "hosted authentication is unavailable in this product profile"
    ))
}

fn authentication_unavailable() -> UserAuthenticationError {
    UserAuthenticationError::Unexpected(anyhow!(
        "hosted authentication is unavailable in this product profile"
    ))
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl AuthClient for AuthClientImpl {
    async fn create_anonymous_user(
        &self,
        _: Option<String>,
        _: AnonymousUserType,
    ) -> Result<CreateAnonymousUserResult> {
        unavailable()
    }

    async fn get_or_refresh_access_token(&self) -> Result<AuthToken> {
        self.auth_session.get_or_refresh_access_token().await
    }

    async fn fetch_user(
        &self,
        _: LoginToken,
        _: bool,
    ) -> StdResult<FetchUserResult, UserAuthenticationError> {
        Err(authentication_unavailable())
    }

    async fn fetch_new_custom_token(&self) -> Result<MintCustomTokenResult> {
        unavailable()
    }

    fn on_custom_token_fetched(
        &self,
        _: Result<MintCustomTokenResult>,
    ) -> Result<String, MintCustomTokenError> {
        Err(MintCustomTokenError::Unknown)
    }

    async fn fetch_user_properties<'a>(&self, _: Option<&'a str>) -> Result<GqlUserOutput> {
        unavailable()
    }

    async fn get_user_settings(&self) -> Result<Option<SyncedUserSettings>> {
        unavailable()
    }

    async fn set_is_telemetry_enabled(&self, _: bool) -> Result<()> {
        unavailable()
    }

    async fn set_is_crash_reporting_enabled(&self, _: bool) -> Result<()> {
        unavailable()
    }

    async fn set_is_cloud_conversation_storage_enabled(&self, _: bool) -> Result<()> {
        unavailable()
    }

    async fn update_user_settings(&self, _: UpdateUserSettingsInput) -> Result<()> {
        unavailable()
    }

    async fn set_user_is_onboarded(&self) -> Result<bool> {
        unavailable()
    }

    async fn request_device_code(
        &self,
    ) -> StdResult<oauth2::StandardDeviceAuthorizationResponse, UserAuthenticationError> {
        Err(authentication_unavailable())
    }

    async fn exchange_device_access_token(
        &self,
        _: &oauth2::StandardDeviceAuthorizationResponse,
        _: Duration,
    ) -> StdResult<FirebaseToken, UserAuthenticationError> {
        Err(authentication_unavailable())
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyProperties>> {
        unavailable()
    }

    async fn create_api_key(
        &self,
        _: String,
        _: Option<cynic::Id>,
        _: Option<cynic::Id>,
        _: Option<warp_graphql::scalars::Time>,
    ) -> Result<GenerateApiKeyResult> {
        unavailable()
    }

    async fn expire_api_key(&self, _: &ApiKeyUid) -> Result<ExpireApiKeyResult> {
        unavailable()
    }

    async fn list_agent_identities(&self) -> Result<Vec<AgentIdentity>> {
        unavailable()
    }
}
