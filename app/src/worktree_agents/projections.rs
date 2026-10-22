use super::catalog::WorktreeDirtyState;
use super::identity::WorktreeKey;
use super::observation::{
    AgentKey, AgentLifecycle, AgentMetrics, AgentRelation, BoundedStatusLabel, SourceAuthority,
    SourceHealth,
};
use super::providers::AgentProvider;
use super::routing::RouteConfidence;
use super::runtimes::AgentRuntimeHint;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentActivationKind {
    FocusPane,
    AttachTmuxPane,
    AttachHerdrAgent,
    ResumeAgent,
    ObserveOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AgentRouteProjection {
    pub(crate) kind: AgentActivationKind,
    pub(crate) confidence: RouteConfidence,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentProjection {
    pub(crate) key: AgentKey,
    pub(crate) provider: AgentProvider,
    pub(crate) lifecycle: AgentLifecycle,
    pub(crate) authority: SourceAuthority,
    pub(crate) source_health: SourceHealth,
    pub(crate) runtime: AgentRuntimeHint,
    pub(crate) relation: Option<AgentRelation>,
    pub(crate) label: Option<BoundedStatusLabel>,
    pub(crate) metrics: AgentMetrics,
    pub(crate) observed_at_ms: u64,
    pub(crate) expires_at_ms: u64,
    pub(crate) current_pane: bool,
    pub(crate) route: Option<AgentRouteProjection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeProjection {
    pub(crate) key: WorktreeKey,
    pub(crate) branch: Option<String>,
    pub(crate) detached_head: Option<String>,
    pub(crate) available: bool,
    pub(crate) dirty: WorktreeDirtyState,
    pub(crate) current: bool,
    pub(crate) agents: Vec<AgentProjection>,
}
