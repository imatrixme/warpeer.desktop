#[path = "coordinator/activation.rs"]
mod activation;
#[path = "coordinator/catalog.rs"]
mod catalog;
#[path = "coordinator/herdr.rs"]
mod herdr;
#[path = "coordinator/host.rs"]
mod host;
#[path = "coordinator/pane_sessions.rs"]
mod pane_sessions;

use std::collections::{HashMap, HashSet};

use instant::Instant;
#[cfg(test)]
use pane_sessions::{
    can_observe_agents, lifecycle_from_cli_status, lifecycle_label, provider_from_cli_agent,
};
use warpui::{Entity, EntityId, ModelContext, SingletonEntity, WeakViewHandle};

use super::catalog::WorktreeDirtyState;
use super::identity::{ExecutionHostKey, RepositoryKey, WorktreeKey};
use super::launch::WorktreeAgentLaunch;
use super::observation::{AgentKey, AgentLifecycle, ObservationStore, SourceAuthority};
use super::pane_index::PaneRouteIndex;
use super::projections::{
    AgentActivationKind, AgentProjection, AgentRouteProjection, WorktreeProjection,
};
use super::providers::AgentProvider;
use super::routing::{AgentRoute, RouteConfidence, RouteRegistry};
use super::session_info::AgentSessionInfoCollector;
use super::settings::WorktreeAgentSettings;
use super::sources::{SourceScheduler, SourceVisibility};
use super::ui::WorktreeAgentHeaderView;
#[cfg(test)]
use crate::terminal::CLIAgent;
#[cfg(test)]
use crate::terminal::cli_agent_sessions::CLIAgentSessionStatus;
use crate::terminal::cli_agent_sessions::CLIAgentSessionsModel;

#[derive(Clone, Copy, Debug)]
pub(crate) enum WorktreeAgentCoordinatorEvent {
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentActivationDecision {
    FocusPane(EntityId),
    Launch(WorktreeAgentLaunch),
    AlreadyFocused,
    Stale,
    ObserveOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorktreeMetadata {
    branch: Option<String>,
    detached_head: Option<String>,
    locked: bool,
    available: bool,
    dirty: WorktreeDirtyState,
}

#[derive(Clone, Debug)]
struct PendingPaneAgent {
    provider: AgentProvider,
    session_id: String,
    cwd: Option<String>,
    lifecycle: AgentLifecycle,
    authority: SourceAuthority,
    metrics: crate::worktree_agents::observation::AgentMetrics,
}

pub(crate) struct WorktreeAgentCoordinator {
    collection_enabled: bool,
    observations: ObservationStore,
    session_info: AgentSessionInfoCollector,
    routes: RouteRegistry,
    pane_index: PaneRouteIndex,
    pane_worktrees: HashMap<EntityId, WorktreeKey>,
    pane_agent_keys: HashMap<EntityId, AgentKey>,
    hovered_agent: Option<AgentKey>,
    active_header_panel: Option<EntityId>,
    pending_agents: HashMap<EntityId, PendingPaneAgent>,
    worktree_metadata: HashMap<WorktreeKey, WorktreeMetadata>,
    catalog_generations: HashMap<RepositoryKey, u64>,
    dirty_probe_in_flight: HashMap<RepositoryKey, u64>,
    header_consumers: HashMap<EntityId, WeakViewHandle<WorktreeAgentHeaderView>>,
    herdr_agent_keys: HashSet<AgentKey>,
    herdr_source: SourceScheduler,
    herdr_task_in_flight: bool,
    herdr_generation: u64,
    claude_agent_keys: HashSet<AgentKey>,
    claude_process_ids: HashSet<u32>,
    codex_agent_keys: HashSet<AgentKey>,
    codex_process_ids: HashSet<u32>,
    inferred_agent_keys: HashSet<AgentKey>,
    host_source_polls: host::HostSourcePolls,
    host_poll_in_flight: bool,
    host_generation: u64,
    started_at: Instant,
}

impl WorktreeAgentCoordinator {
    pub(crate) fn new(ctx: &mut ModelContext<Self>) -> Self {
        let collection_enabled = *WorktreeAgentSettings::as_ref(ctx).show_in_pane_header;
        ctx.subscribe_to_model(
            &CLIAgentSessionsModel::handle(ctx),
            Self::handle_cli_agent_event,
        );
        ctx.subscribe_to_model(
            &WorktreeAgentSettings::handle(ctx),
            |coordinator, _, _, ctx| coordinator.handle_collection_setting_changed(ctx),
        );
        Self {
            collection_enabled,
            observations: ObservationStore::default(),
            session_info: AgentSessionInfoCollector::default(),
            routes: RouteRegistry::default(),
            pane_index: PaneRouteIndex::default(),
            pane_worktrees: HashMap::new(),
            pane_agent_keys: HashMap::new(),
            hovered_agent: None,
            active_header_panel: None,
            pending_agents: HashMap::new(),
            worktree_metadata: HashMap::new(),
            catalog_generations: HashMap::new(),
            dirty_probe_in_flight: HashMap::new(),
            header_consumers: HashMap::new(),
            herdr_agent_keys: HashSet::new(),
            herdr_source: herdr::source_scheduler(),
            herdr_task_in_flight: false,
            herdr_generation: 0,
            claude_agent_keys: HashSet::new(),
            claude_process_ids: HashSet::new(),
            codex_agent_keys: HashSet::new(),
            codex_process_ids: HashSet::new(),
            inferred_agent_keys: HashSet::new(),
            host_source_polls: host::HostSourcePolls::default(),
            host_poll_in_flight: false,
            host_generation: 0,
            started_at: Instant::now(),
        }
    }

    pub(crate) fn projection_for_terminal(
        &self,
        terminal_view_id: EntityId,
    ) -> Vec<WorktreeProjection> {
        let Some(current_worktree) = self.pane_worktrees.get(&terminal_view_id) else {
            return Vec::new();
        };
        let repository = current_worktree.repository();
        let now_ms = self.now_ms();
        let mut projections: HashMap<WorktreeKey, Vec<AgentProjection>> = HashMap::new();
        for observation in self
            .observations
            .values()
            .filter(|observation| observation.key.worktree.repository() == repository)
        {
            let lifecycle = if observation.expires_at_ms <= now_ms {
                AgentLifecycle::Stale
            } else {
                observation.lifecycle
            };
            let current_pane = self.routes.get(&observation.key).is_some_and(|record| {
                record.confidence == RouteConfidence::Exact
                    && record.expires_at_ms > now_ms
                    && matches!(
                        &record.route,
                        AgentRoute::ExistingPane {
                            terminal_view_id: route_terminal_view_id,
                            correlation,
                        } if *route_terminal_view_id == terminal_view_id
                            && self.pane_index.validate_agent(
                                &observation.key,
                                *route_terminal_view_id,
                                *correlation,
                            )
                    )
            });
            let route = self.routes.get(&observation.key).map(|record| {
                let confidence = if record.expires_at_ms <= now_ms {
                    RouteConfidence::Stale
                } else {
                    record.confidence
                };
                AgentRouteProjection {
                    kind: activation_kind(&record.route),
                    confidence,
                    generation: record.generation,
                }
            });
            projections
                .entry(observation.key.worktree.clone())
                .or_default()
                .push(AgentProjection {
                    key: observation.key.clone(),
                    provider: observation.key.provider,
                    lifecycle,
                    authority: observation.authority,
                    source_health: self.source_health_for(observation.source_id),
                    runtime: observation.runtime,
                    relation: observation.relation.clone(),
                    label: observation.label.clone(),
                    metrics: observation.metrics.clone(),
                    observed_at_ms: observation.observed_at_ms,
                    expires_at_ms: observation.expires_at_ms,
                    current_pane,
                    route,
                });
        }
        let mut projections: Vec<_> = projections
            .into_iter()
            .map(|(key, agents)| {
                let metadata = self.worktree_metadata.get(&key);
                WorktreeProjection {
                    current: key == *current_worktree,
                    key,
                    branch: metadata.and_then(|metadata| metadata.branch.clone()),
                    detached_head: metadata.and_then(|metadata| metadata.detached_head.clone()),
                    available: metadata.is_none_or(|metadata| metadata.available),
                    dirty: metadata.map_or(WorktreeDirtyState::Unknown, |metadata| metadata.dirty),
                    agents,
                }
            })
            .collect();
        projections.sort_by(|left, right| {
            right
                .current
                .cmp(&left.current)
                .then_with(|| left.key.path().cmp(right.key.path()))
        });
        projections
    }

    pub(crate) fn register_header_consumer(
        &mut self,
        terminal_view_id: EntityId,
        consumer: WeakViewHandle<WorktreeAgentHeaderView>,
    ) {
        self.header_consumers.insert(terminal_view_id, consumer);
    }

    pub(crate) fn active_header_panel(&self) -> Option<EntityId> {
        self.active_header_panel
    }

    pub(crate) fn set_header_panel_active(
        &mut self,
        terminal_view_id: EntityId,
        active: bool,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        let next = if active {
            Some(terminal_view_id)
        } else if self.active_header_panel == Some(terminal_view_id) {
            None
        } else {
            return false;
        };
        if self.active_header_panel == next {
            return false;
        }
        self.active_header_panel = next;
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
        true
    }

    pub(crate) fn set_agent_hovered(
        &mut self,
        key: &AgentKey,
        hovered: bool,
        ctx: &mut ModelContext<Self>,
    ) {
        let next = if hovered {
            Some(key.clone())
        } else if self.hovered_agent.as_ref() == Some(key) {
            None
        } else {
            return;
        };
        if self.hovered_agent == next {
            return;
        }
        self.hovered_agent = next;
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
    }

    pub(crate) fn clear_agent_hover(&mut self, ctx: &mut ModelContext<Self>) {
        if self.hovered_agent.take().is_some() {
            ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
            ctx.notify();
        }
    }

    pub(crate) fn hovered_agent_target_pane(&self) -> Option<EntityId> {
        self.attached_pane_for_agent(self.hovered_agent.as_ref()?)
    }

    pub(crate) fn hovered_agent_key(&self) -> Option<&AgentKey> {
        self.hovered_agent.as_ref()
    }

    pub(crate) fn hovered_agent_targets_pane(&self, terminal_view_id: EntityId) -> bool {
        self.hovered_agent_target_pane() == Some(terminal_view_id)
    }

    pub(crate) fn refresh_sources(&mut self, ctx: &mut ModelContext<Self>) {
        if !self.collection_enabled {
            return;
        }
        self.start_herdr_polling(ctx);
        self.start_host_polling(ctx);
    }

    fn handle_collection_setting_changed(&mut self, ctx: &mut ModelContext<Self>) {
        let collection_enabled = *WorktreeAgentSettings::as_ref(ctx).show_in_pane_header;
        if self.collection_enabled == collection_enabled {
            return;
        }
        self.collection_enabled = collection_enabled;
        if collection_enabled {
            self.restore_cli_agent_sessions(ctx);
            self.refresh_sources(ctx);
        } else {
            self.suspend_agent_collection(ctx);
        }
    }

    fn suspend_agent_collection(&mut self, ctx: &mut ModelContext<Self>) {
        self.host_generation = self.host_generation.wrapping_add(1).max(1);
        self.host_poll_in_flight = false;
        self.herdr_generation = self.herdr_generation.wrapping_add(1).max(1);
        self.herdr_task_in_flight = false;
        self.host_source_polls = host::HostSourcePolls::default();
        self.herdr_source = herdr::source_scheduler();
        self.observations = ObservationStore::default();
        self.session_info = AgentSessionInfoCollector::default();
        self.routes = RouteRegistry::default();
        for terminal_view_id in self.pane_agent_keys.keys() {
            self.pane_index.unbind_agent(*terminal_view_id);
        }
        self.pane_agent_keys.clear();
        self.hovered_agent = None;
        self.active_header_panel = None;
        self.pending_agents.clear();
        self.herdr_agent_keys.clear();
        self.claude_agent_keys.clear();
        self.claude_process_ids.clear();
        self.codex_agent_keys.clear();
        self.codex_process_ids.clear();
        self.inferred_agent_keys.clear();
        ctx.emit(WorktreeAgentCoordinatorEvent::Changed);
        ctx.notify();
    }

    pub(crate) fn now_ms(&self) -> u64 {
        self.started_at
            .elapsed()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn apply_observation(&mut self, observation: super::observation::AgentObservation) -> bool {
        let observation = self.session_info.collect(observation);
        self.observations.apply(observation)
    }

    fn source_health_for(&self, source_id: &'static str) -> super::observation::SourceHealth {
        if source_id.starts_with("warp-cli-agent-") {
            super::observation::SourceHealth::Healthy
        } else if source_id == herdr::source_id() {
            self.herdr_source.health()
        } else {
            self.host_source_polls.health_for(source_id)
        }
    }

    pub(super) fn local_source_visibility(
        &mut self,
        ctx: &ModelContext<Self>,
    ) -> Option<SourceVisibility> {
        if !self.collection_enabled {
            return None;
        }
        self.header_consumers
            .retain(|_, consumer| consumer.upgrade(ctx).is_some());
        if self.header_consumers.keys().any(|terminal_view_id| {
            self.pane_worktrees
                .get(terminal_view_id)
                .is_some_and(|key| matches!(key.repository().host(), ExecutionHostKey::Local))
        }) {
            Some(SourceVisibility::Foreground)
        } else if self
            .pane_worktrees
            .values()
            .any(|key| matches!(key.repository().host(), ExecutionHostKey::Local))
        {
            Some(SourceVisibility::Background)
        } else {
            None
        }
    }
}

fn activation_kind(route: &AgentRoute) -> AgentActivationKind {
    match route {
        AgentRoute::ExistingPane { .. } => AgentActivationKind::FocusPane,
        AgentRoute::Tmux { .. } => AgentActivationKind::AttachTmuxPane,
        AgentRoute::Herdr { .. } => AgentActivationKind::AttachHerdrAgent,
        AgentRoute::NativeResume(_) => AgentActivationKind::ResumeAgent,
        AgentRoute::ObserveOnly => AgentActivationKind::ObserveOnly,
    }
}

impl Entity for WorktreeAgentCoordinator {
    type Event = WorktreeAgentCoordinatorEvent;
}

impl SingletonEntity for WorktreeAgentCoordinator {}

#[cfg(test)]
#[path = "coordinator_tests.rs"]
mod tests;
