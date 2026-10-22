use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use super::MCPServerState;
use super::templatable::TemplatableMCPServer;
use super::templatable_installation::TemplatableMCPServerInstallation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpIntegration {
    Figma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigmaMcpStatus {
    NotInstalled,
    Installed,
    Enabling,
    Running,
}

#[derive(Default)]
pub struct TemplatableMCPServerManager;

impl TemplatableMCPServerManager {
    pub fn new(
        _locally_installed_servers: HashMap<Uuid, TemplatableMCPServerInstallation>,
        _running_server_uuids: Vec<Uuid>,
        _running_legacy_server_uuids: &[Uuid],
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self
    }

    pub fn get_installed_templatable_servers(
        &self,
    ) -> &HashMap<Uuid, TemplatableMCPServerInstallation> {
        static EMPTY: std::sync::LazyLock<HashMap<Uuid, TemplatableMCPServerInstallation>> =
            std::sync::LazyLock::new(HashMap::new);
        &EMPTY
    }

    pub fn get_installed_server(
        &self,
        _installation_uuid: &Uuid,
    ) -> Option<&TemplatableMCPServerInstallation> {
        None
    }

    pub fn get_figma_installation_uuid(&self) -> Option<Uuid> {
        None
    }

    pub fn get_figma_mcp_status(&self) -> FigmaMcpStatus {
        FigmaMcpStatus::NotInstalled
    }

    pub fn get_template_uuid(&self, _installation_uuid: Uuid) -> Option<Uuid> {
        None
    }

    pub fn is_server_active(&self, _installation_uuid: Uuid) -> bool {
        false
    }

    pub fn is_server_active_or_pending(&self, _uuid: Uuid) -> bool {
        false
    }

    pub fn get_server_state(&self, _installation_uuid: Uuid) -> Option<MCPServerState> {
        None
    }

    pub fn get_server_error_message(&self, _installation_uuid: Uuid) -> Option<&str> {
        None
    }

    pub fn tool_input_schema(
        &self,
        _installation_id: Option<Uuid>,
        _tool_name: &str,
    ) -> Option<Arc<serde_json::Map<String, serde_json::Value>>> {
        None
    }

    pub fn server_from_tool(&self, _tool: String) -> Option<&Uuid> {
        None
    }

    pub fn server_from_resource(&self, _name: &str, _uri: Option<&str>) -> Option<&Uuid> {
        None
    }

    pub fn get_all_cloud_synced_mcp_servers(_ctx: &AppContext) -> HashMap<Uuid, String> {
        HashMap::new()
    }

    pub fn get_mcp_name(_uuid: &Uuid, _app: &AppContext) -> Option<String> {
        None
    }

    pub fn is_mcp_server_running(&self, integration: McpIntegration) -> bool {
        match integration {
            McpIntegration::Figma => false,
        }
    }

    pub fn get_templatable_mcp_server(&self, _uuid: Uuid) -> Option<&TemplatableMCPServer> {
        None
    }

    pub fn handle_oauth_callback(&mut self, _url: &url::Url) -> anyhow::Result<()> {
        anyhow::bail!("MCP is unavailable in this product profile")
    }

    pub fn sync_builtin_servers(&mut self, _force_respawn: bool, _ctx: &mut ModelContext<Self>) {}

    pub fn install_figma_from_gallery(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn enable_figma_mcp(&mut self, _ctx: &mut ModelContext<Self>) {}
}

#[derive(Debug)]
pub enum TemplatableMCPServerManagerEvent {
    StateChanged { uuid: Uuid, state: MCPServerState },
    AuthenticationRequired { uuid: Uuid },
    CredentialsChanged { uuid: Uuid },
    ServerInstallationAdded(Uuid),
    ServerInstallationDeleted(Uuid),
    TemplatableMCPServersUpdated,
    LegacyServerConverted,
}

impl Entity for TemplatableMCPServerManager {
    type Event = TemplatableMCPServerManagerEvent;
}

impl SingletonEntity for TemplatableMCPServerManager {}
