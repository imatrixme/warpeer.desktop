use uuid::Uuid;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::mcp::templatable::{JsonTemplate, TemplatableMCPServer};

#[derive(Clone, Debug)]
pub struct GalleryMCPServer {
    uuid: Uuid,
    title: String,
    description: String,
    version: i32,
    instructions_in_markdown: Option<String>,
    json_template: JsonTemplate,
}

impl GalleryMCPServer {
    pub fn new(
        uuid: Uuid,
        title: String,
        description: String,
        version: i32,
        instructions_in_markdown: Option<String>,
        json_template: JsonTemplate,
    ) -> Self {
        Self {
            uuid,
            title,
            description,
            version,
            instructions_in_markdown,
            json_template,
        }
    }

    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn description(&self) -> String {
        self.description.clone()
    }

    pub fn version(&self) -> i32 {
        self.version
    }

    pub fn json_template(&self) -> &JsonTemplate {
        &self.json_template
    }

    pub fn instructions_in_markdown(&self) -> Option<&String> {
        self.instructions_in_markdown.as_ref()
    }
}

impl TryFrom<GalleryMCPServer> for TemplatableMCPServer {
    type Error = String;

    fn try_from(server: GalleryMCPServer) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: Uuid::new_v4(),
            name: server.title,
            description: Some(server.description),
            template: server.json_template,
            version: chrono::Local::now().timestamp(),
            gallery_data: None,
        })
    }
}

pub struct MCPGalleryManager;

impl MCPGalleryManager {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    pub fn get_gallery(&self) -> Vec<GalleryMCPServer> {
        Vec::new()
    }

    pub fn get_gallery_item(&self, _gallery_uuid: Uuid) -> Option<&GalleryMCPServer> {
        None
    }

    pub fn get_templatable_mcp_server(&self, _gallery_uuid: Uuid) -> Option<&TemplatableMCPServer> {
        None
    }
}

pub enum MCPGalleryManagerEvent {
    ItemsRefreshed,
}

impl Entity for MCPGalleryManager {
    type Event = MCPGalleryManagerEvent;
}

impl SingletonEntity for MCPGalleryManager {}
