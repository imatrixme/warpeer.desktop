use std::fmt::Display;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct MCPResourceDescriptor {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MCPToolDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Arc<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallMCPToolResult {
    Success { result: MCPToolCallOutput },
    Error(String),
    Cancelled,
}

impl Display for CallMCPToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallMCPToolResult::Success { result } => {
                write!(f, "MCP tool call completed: [{result:?}]")
            }
            CallMCPToolResult::Error(error) => write!(f, "MCP tool call error: {error}"),
            CallMCPToolResult::Cancelled => write!(f, "MCP tool call cancelled"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadMCPResourceResult {
    Success {
        resource_contents: Vec<MCPResourceContents>,
    },
    Error(String),
    Cancelled,
}

impl Display for ReadMCPResourceResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadMCPResourceResult::Success { resource_contents } => {
                write!(f, "MCP resource read completed: [{resource_contents:?}]")
            }
            ReadMCPResourceResult::Error(error) => write!(f, "MCP resource error: {error}"),
            ReadMCPResourceResult::Cancelled => write!(f, "MCP resource read cancelled"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPToolCallOutput {
    #[serde(default)]
    pub content: Vec<MCPToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl MCPToolCallOutput {
    pub fn success(content: Vec<MCPToolContent>) -> Self {
        Self {
            content,
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }

    pub fn structured(value: serde_json::Value) -> Self {
        Self {
            content: Vec::new(),
            structured_content: Some(value),
            is_error: Some(false),
            meta: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MCPToolContent {
    #[serde(flatten)]
    pub raw: MCPToolContentBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Value>,
}

impl MCPToolContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            raw: MCPToolContentBody::Text {
                text: text.into(),
                meta: None,
            },
            annotations: None,
        }
    }

    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            raw: MCPToolContentBody::Image {
                data: data.into(),
                mime_type: mime_type.into(),
                meta: None,
            },
            annotations: None,
        }
    }

    pub fn resource(resource: MCPResourceContents) -> Self {
        Self {
            raw: MCPToolContentBody::Resource {
                meta: None,
                resource,
            },
            annotations: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MCPToolContentBody {
    Text {
        text: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    Resource {
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
        resource: MCPResourceContents,
    },
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    ResourceLink(MCPResourceLink),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPResourceLink {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<serde_json::Value>>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MCPResourceContents {
    #[serde(rename_all = "camelCase")]
    TextResourceContents {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        text: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
    #[serde(rename_all = "camelCase")]
    BlobResourceContents {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        blob: String,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<serde_json::Value>,
    },
}

impl MCPResourceContents {
    pub fn text(text: impl Into<String>, uri: impl Into<String>) -> Self {
        Self::TextResourceContents {
            uri: uri.into(),
            mime_type: Some("text".to_string()),
            text: text.into(),
            meta: None,
        }
    }

    pub fn blob(blob: impl Into<String>, uri: impl Into<String>) -> Self {
        Self::BlobResourceContents {
            uri: uri.into(),
            mime_type: None,
            blob: blob.into(),
            meta: None,
        }
    }
}
