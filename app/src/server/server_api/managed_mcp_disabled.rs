use anyhow::{Result, anyhow};
use async_trait::async_trait;
use warp_graphql::mutations::create_managed_mcp_client_config::CreateManagedMcpClientConfigOutput;

use super::ServerApi;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait ManagedMcpClient: 'static + Send + Sync {
    async fn create_managed_mcp_client_config(
        &self,
        uid: String,
    ) -> Result<CreateManagedMcpClientConfigOutput>;
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ManagedMcpClient for ServerApi {
    async fn create_managed_mcp_client_config(
        &self,
        uid: String,
    ) -> Result<CreateManagedMcpClientConfigOutput> {
        drop(uid);
        Err(anyhow!(
            "managed MCP is unavailable in this product profile"
        ))
    }
}
