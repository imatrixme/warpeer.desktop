use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CloseContainerRequest, ContainerAcknowledgement, ContainerEvent, ContainerLimits,
    ContainerTransaction, ContextId, ExtensionSupport, OpenContainerRequest, PluginId, RequestId,
    ResourceHandleId, Revision, SubscriptionId, SurfaceDescriptor, SurfaceEnvironment,
    SurfaceFrame, SurfaceId, SurfaceInput, SurfaceResizeRequest, WorkerId,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ProtocolVersion(pub u16);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeAbi {
    pub major: u16,
    pub minor: u16,
}

impl RuntimeAbi {
    pub fn as_semver(self) -> semver::Version {
        semver::Version::new(self.major.into(), self.minor.into(), 0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerState {
    Starting,
    Ready,
    Idle,
    Running,
    Suspended,
    Stopping,
    Stopped,
    Failed,
    WatchdogTerminated,
    Uninstalled,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LifecycleEvent {
    pub plugin_id: PluginId,
    pub worker_id: WorkerId,
    pub state: WorkerState,
    pub detail: Option<String>,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CapabilityRequest {
    pub request_id: RequestId,
    pub plugin_id: PluginId,
    pub context_id: Option<ContextId>,
    pub context_revision: Option<Revision>,
    pub method: String,
    pub scope: Option<String>,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CapabilityResponse {
    pub request_id: RequestId,
    pub result: Result<BridgePayload, BridgeError>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BridgePayload {
    Json {
        value: Value,
    },
    Bytes {
        media_type: String,
        bytes: Vec<u8>,
    },
    ImageResource {
        url: String,
        source_url: String,
        max_bytes: u64,
        max_width: u16,
        max_height: u16,
    },
    RgbaImage {
        media_type: String,
        width: u16,
        height: u16,
        pixels: Vec<u8>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "code", content = "detail", rename_all = "kebab-case")]
pub enum BridgeError {
    #[error("request was cancelled")]
    Cancelled,
    #[error("capability is not granted: {0}")]
    PermissionDenied(String),
    #[error("context is stale")]
    StaleContext,
    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),
    #[error("resource was not found")]
    NotFound,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("plugin bridge failed: {0}")]
    Internal(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TerminalHoverContext {
    pub context_id: ContextId,
    pub revision: Revision,
    pub terminal_id: String,
    pub pane_id: String,
    pub workspace_id: Option<String>,
    pub working_directory: Option<String>,
    pub resource_text: String,
    pub resolved_resource: Option<String>,
    pub explicit_hyperlink: bool,
    #[serde(default)]
    pub explicit_preview: bool,
    pub anchor: [f32; 4],
    pub viewport: [f32; 4],
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PaneContext {
    pub context_id: ContextId,
    pub revision: Revision,
    pub pane_id: String,
    pub workspace_id: Option<String>,
    pub working_directory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PluginSettingsContext {
    pub context_id: ContextId,
    pub revision: Revision,
    pub plugin_id: PluginId,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SubscriptionEvent {
    pub subscription_id: SubscriptionId,
    pub revision: Revision,
    pub topic: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
pub enum HostToPlugin {
    Start {
        plugin_id: PluginId,
        worker_id: WorkerId,
        #[serde(default)]
        extensions: Vec<ExtensionSupport>,
        #[serde(default)]
        container_limits: ContainerLimits,
    },
    Suspend,
    Resume,
    Stop,
    Uninstall,
    Configuration {
        revision: Revision,
        values: Value,
    },
    TerminalHover(TerminalHoverContext),
    PaneContext(PaneContext),
    PluginSettingsContext(PluginSettingsContext),
    CancelContext {
        context_id: ContextId,
        revision: Revision,
    },
    CapabilityResponse(CapabilityResponse),
    SurfaceEnvironment(SurfaceEnvironment),
    SurfaceInput {
        surface_id: SurfaceId,
        input: SurfaceInput,
    },
    ReleaseSurface {
        surface_id: SurfaceId,
        revision: Revision,
    },
    ContainerAcknowledgement(ContainerAcknowledgement),
    ContainerEvent(ContainerEvent),
    Subscription(SubscriptionEvent),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "kebab-case")]
pub enum PluginToHost {
    Lifecycle(LifecycleEvent),
    CapabilityRequest(CapabilityRequest),
    CancelRequest {
        request_id: RequestId,
    },
    OpenSurface(SurfaceDescriptor),
    ResizeSurface(SurfaceResizeRequest),
    CloseSurface {
        surface_id: SurfaceId,
        revision: Revision,
    },
    SurfaceReleased {
        surface_id: SurfaceId,
        revision: Revision,
    },
    OpenContainer(OpenContainerRequest),
    TransactContainer(ContainerTransaction),
    CloseContainer(CloseContainerRequest),
    Frame(SurfaceFrame),
    ReleaseResource(ResourceHandleId),
    Diagnostic {
        level: DiagnosticLevel,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticLevel {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Envelope<T> {
    pub protocol: ProtocolVersion,
    pub abi: RuntimeAbi,
    pub message: T,
}

#[derive(Clone, Copy, Debug)]
pub struct ProtocolLimits {
    pub max_payload_bytes: u64,
    pub max_queued_messages: usize,
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: 8 * 1024 * 1024,
            max_queued_messages: 128,
        }
    }
}

impl ProtocolLimits {
    pub fn validate<T: Serialize>(&self, envelope: &Envelope<T>) -> Result<(), ProtocolError> {
        let size = serde_json::to_vec(envelope)
            .map_err(ProtocolError::Serialize)?
            .len() as u64;
        if size > self.max_payload_bytes {
            return Err(ProtocolError::PayloadTooLarge {
                actual: size,
                maximum: self.max_payload_bytes,
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("failed to serialize plugin message: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("plugin message is {actual} bytes; maximum is {maximum}")]
    PayloadTooLarge { actual: u64, maximum: u64 },
}
