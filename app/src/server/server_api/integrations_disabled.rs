use anyhow::{Result, anyhow};
use async_trait::async_trait;
use warp_graphql::mutations::create_simple_integration::CreateSimpleIntegrationOutput;
use warp_graphql::queries::get_integrations_using_environment::GetIntegrationsUsingEnvironmentOutput;
use warp_graphql::queries::get_oauth_connect_tx_status::OauthConnectTxStatus;
use warp_graphql::queries::get_simple_integrations::SimpleIntegrationsOutput;
use warp_graphql::queries::suggest_cloud_environment_image::SuggestCloudEnvironmentImageResult;
use warp_graphql::queries::user_github_info::UserGithubInfoResult;
use warp_graphql::queries::user_repo_auth_status::UserRepoAuthStatusOutput;

use super::ServerApi;

#[cfg(not(target_family = "wasm"))]
pub trait IntegrationsClientBounds: Send + Sync {}

#[cfg(not(target_family = "wasm"))]
impl<T: 'static + Send + Sync> IntegrationsClientBounds for T {}

#[cfg(target_family = "wasm")]
pub trait IntegrationsClientBounds {}

#[cfg(target_family = "wasm")]
impl<T: 'static> IntegrationsClientBounds for T {}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
pub trait IntegrationsClient: 'static + IntegrationsClientBounds {
    async fn check_user_repo_auth_status(
        &self,
        repos: Vec<(String, String)>,
    ) -> Result<UserRepoAuthStatusOutput>;

    #[allow(clippy::too_many_arguments)]
    async fn create_or_update_simple_integration(
        &self,
        integration_type: String,
        is_update: bool,
        environment_uid: Option<String>,
        base_prompt: Option<String>,
        model_id: Option<String>,
        mcp_servers_json: Option<String>,
        remove_mcp_server_names: Option<Vec<String>>,
        worker_host: Option<String>,
        enabled: bool,
    ) -> Result<CreateSimpleIntegrationOutput>;

    async fn list_simple_integrations(
        &self,
        providers: Vec<String>,
    ) -> Result<SimpleIntegrationsOutput>;

    async fn poll_oauth_connect_status(&self, tx_id: String) -> Result<OauthConnectTxStatus>;

    async fn get_integrations_using_environment(
        &self,
        environment_id: String,
    ) -> Result<GetIntegrationsUsingEnvironmentOutput>;

    async fn get_user_github_info(&self) -> Result<UserGithubInfoResult>;

    async fn suggest_cloud_environment_image(
        &self,
        repos: Vec<(String, String)>,
    ) -> Result<SuggestCloudEnvironmentImageResult>;
}

fn unavailable<T>() -> Result<T> {
    Err(anyhow!(
        "hosted integrations are unavailable in this product profile"
    ))
}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
impl IntegrationsClient for ServerApi {
    async fn check_user_repo_auth_status(
        &self,
        repos: Vec<(String, String)>,
    ) -> Result<UserRepoAuthStatusOutput> {
        drop(repos);
        unavailable()
    }

    async fn create_or_update_simple_integration(
        &self,
        integration_type: String,
        is_update: bool,
        environment_uid: Option<String>,
        base_prompt: Option<String>,
        model_id: Option<String>,
        mcp_servers_json: Option<String>,
        remove_mcp_server_names: Option<Vec<String>>,
        worker_host: Option<String>,
        enabled: bool,
    ) -> Result<CreateSimpleIntegrationOutput> {
        drop(integration_type);
        let _ = is_update;
        drop(environment_uid);
        drop(base_prompt);
        drop(model_id);
        drop(mcp_servers_json);
        drop(remove_mcp_server_names);
        drop(worker_host);
        let _ = enabled;
        unavailable()
    }

    async fn list_simple_integrations(
        &self,
        providers: Vec<String>,
    ) -> Result<SimpleIntegrationsOutput> {
        drop(providers);
        unavailable()
    }

    async fn poll_oauth_connect_status(&self, tx_id: String) -> Result<OauthConnectTxStatus> {
        drop(tx_id);
        unavailable()
    }

    async fn get_integrations_using_environment(
        &self,
        environment_id: String,
    ) -> Result<GetIntegrationsUsingEnvironmentOutput> {
        drop(environment_id);
        unavailable()
    }

    async fn get_user_github_info(&self) -> Result<UserGithubInfoResult> {
        unavailable()
    }

    async fn suggest_cloud_environment_image(
        &self,
        repos: Vec<(String, String)>,
    ) -> Result<SuggestCloudEnvironmentImageResult> {
        drop(repos);
        unavailable()
    }
}
