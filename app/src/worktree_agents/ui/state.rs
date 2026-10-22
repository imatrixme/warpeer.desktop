use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use warpui::elements::MouseStateHandle;

use crate::worktree_agents::identity::WorktreeKey;
use crate::worktree_agents::observation::AgentKey;
use crate::worktree_agents::projections::WorktreeProjection;
use crate::worktree_agents::settings::{WorktreeAgentSettings, WorktreeAgentSortKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HeaderActionOutcome {
    Activated,
    AlreadyFocused,
    Stale,
    ObserveOnly,
}

#[derive(Clone)]
pub(crate) struct HeaderUiSnapshot {
    pub(super) compact_mouse_state: MouseStateHandle,
    panel_mouse_state: MouseStateHandle,
    worktree_mouse_states: HashMap<WorktreeKey, MouseStateHandle>,
    agent_mouse_states: HashMap<AgentKey, MouseStateHandle>,
    detach_mouse_states: HashMap<AgentKey, MouseStateHandle>,
    copy_path_mouse_states: HashMap<WorktreeKey, MouseStateHandle>,
    copy_branch_mouse_states: HashMap<WorktreeKey, MouseStateHandle>,
    outcomes: HashMap<AgentKey, HeaderActionOutcome>,
    pub(super) visible: bool,
    pub(super) status_phase: bool,
    pub(super) reduced_motion: bool,
    panel_progress: f32,
    sort_key: WorktreeAgentSortKey,
    sort_descending: bool,
    now_ms: u64,
    hovered_worktree: Option<WorktreeKey>,
}

impl HeaderUiSnapshot {
    pub(crate) fn panel_mouse_state(&self) -> MouseStateHandle {
        self.panel_mouse_state.clone()
    }

    pub(crate) fn worktree_mouse_state(&self, key: &WorktreeKey) -> MouseStateHandle {
        self.worktree_mouse_states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn agent_mouse_state(&self, key: &AgentKey) -> MouseStateHandle {
        self.agent_mouse_states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn detach_mouse_state(&self, key: &AgentKey) -> MouseStateHandle {
        self.detach_mouse_states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn outcome(&self, key: &AgentKey) -> Option<HeaderActionOutcome> {
        self.outcomes.get(key).copied()
    }

    pub(crate) fn panel_progress(&self) -> f32 {
        self.panel_progress
    }

    pub(crate) fn copy_path_mouse_state(&self, key: &WorktreeKey) -> MouseStateHandle {
        self.copy_path_mouse_states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn copy_branch_mouse_state(&self, key: &WorktreeKey) -> MouseStateHandle {
        self.copy_branch_mouse_states
            .get(key)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn sort_key(&self) -> WorktreeAgentSortKey {
        self.sort_key
    }

    pub(crate) fn sort_descending(&self) -> bool {
        self.sort_descending
    }

    pub(crate) fn now_ms(&self) -> u64 {
        self.now_ms
    }

    pub(crate) fn hovered_worktree(&self) -> Option<&WorktreeKey> {
        self.hovered_worktree.as_ref()
    }
}

pub(super) struct HeaderUiState {
    pub(super) compact_mouse_state: MouseStateHandle,
    pub(super) panel_mouse_state: MouseStateHandle,
    worktree_mouse_states: RefCell<HashMap<WorktreeKey, MouseStateHandle>>,
    agent_mouse_states: RefCell<HashMap<AgentKey, MouseStateHandle>>,
    detach_mouse_states: RefCell<HashMap<AgentKey, MouseStateHandle>>,
    copy_path_mouse_states: RefCell<HashMap<WorktreeKey, MouseStateHandle>>,
    copy_branch_mouse_states: RefCell<HashMap<WorktreeKey, MouseStateHandle>>,
    pub(super) outcomes: HashMap<AgentKey, HeaderActionOutcome>,
    pub(super) compact_hovered: bool,
    pub(super) panel_hovered: bool,
    pub(super) hovered_worktree: Option<WorktreeKey>,
    pub(super) status_phase: bool,
    pub(super) reduced_motion: bool,
    pub(super) animation_generation: usize,
    pub(super) animation_in_flight: bool,
    pub(super) panel_progress: f32,
    pub(super) panel_animation_generation: usize,
    pub(super) panel_animation_in_flight: bool,
    pub(super) sort_key: WorktreeAgentSortKey,
    pub(super) sort_descending: bool,
    pub(super) sort_overridden: bool,
}

impl Default for HeaderUiState {
    fn default() -> Self {
        Self {
            compact_mouse_state: MouseStateHandle::default(),
            panel_mouse_state: MouseStateHandle::default(),
            worktree_mouse_states: RefCell::new(HashMap::new()),
            agent_mouse_states: RefCell::new(HashMap::new()),
            detach_mouse_states: RefCell::new(HashMap::new()),
            copy_path_mouse_states: RefCell::new(HashMap::new()),
            copy_branch_mouse_states: RefCell::new(HashMap::new()),
            outcomes: HashMap::new(),
            compact_hovered: false,
            panel_hovered: false,
            hovered_worktree: None,
            status_phase: false,
            reduced_motion: false,
            animation_generation: 0,
            animation_in_flight: false,
            panel_progress: 0.,
            panel_animation_generation: 0,
            panel_animation_in_flight: false,
            sort_key: WorktreeAgentSortKey::Auto,
            sort_descending: true,
            sort_overridden: false,
        }
    }
}

impl HeaderUiState {
    pub(super) fn set_compact_hovered(&mut self, hovered: bool) {
        self.compact_hovered = hovered;
    }

    pub(super) fn set_panel_hovered(&mut self, hovered: bool) {
        self.panel_hovered = hovered;
    }

    pub(super) fn panel_hovered(&self) -> bool {
        self.compact_hovered || self.panel_hovered
    }

    pub(super) fn set_outcome(&mut self, key: AgentKey, outcome: HeaderActionOutcome) {
        self.outcomes.insert(key, outcome);
    }

    pub(super) fn reset_sort(&mut self, settings: &WorktreeAgentSettings) {
        self.sort_key = *settings.default_sort;
        self.sort_descending = *settings.default_sort_descending;
        self.sort_overridden = false;
    }

    pub(super) fn cycle_sort(&mut self) {
        self.sort_key = self.sort_key.next();
        self.sort_overridden = true;
    }

    pub(super) fn toggle_sort_direction(&mut self) {
        if self.sort_key != WorktreeAgentSortKey::Auto {
            self.sort_descending = !self.sort_descending;
            self.sort_overridden = true;
        }
    }

    pub(super) fn dismiss(&mut self) {
        self.compact_hovered = false;
        self.panel_hovered = false;
        self.hovered_worktree = None;
        self.panel_progress = 0.;
        self.panel_animation_generation = self.panel_animation_generation.wrapping_add(1);
        self.panel_animation_in_flight = false;
    }

    pub(super) fn snapshot(
        &self,
        projections: &[WorktreeProjection],
        now_ms: u64,
    ) -> HeaderUiSnapshot {
        let agent_keys: HashSet<_> = projections
            .iter()
            .flat_map(|projection| projection.agents.iter().map(|agent| agent.key.clone()))
            .collect();
        let mut agent_mouse_states = self.agent_mouse_states.borrow_mut();
        agent_mouse_states.retain(|key, _| agent_keys.contains(key));
        for key in agent_keys {
            agent_mouse_states.entry(key.clone()).or_default();
        }
        let mut detach_mouse_states = self.detach_mouse_states.borrow_mut();
        detach_mouse_states.retain(|key, _| agent_mouse_states.contains_key(key));
        for key in agent_mouse_states.keys() {
            detach_mouse_states.entry(key.clone()).or_default();
        }

        let worktree_keys: HashSet<_> = projections
            .iter()
            .map(|projection| projection.key.clone())
            .collect();
        update_worktree_mouse_states(&self.worktree_mouse_states, &worktree_keys);
        update_worktree_mouse_states(&self.copy_path_mouse_states, &worktree_keys);
        update_worktree_mouse_states(&self.copy_branch_mouse_states, &worktree_keys);

        HeaderUiSnapshot {
            compact_mouse_state: self.compact_mouse_state.clone(),
            panel_mouse_state: self.panel_mouse_state.clone(),
            worktree_mouse_states: self.worktree_mouse_states.borrow().clone(),
            agent_mouse_states: agent_mouse_states.clone(),
            detach_mouse_states: detach_mouse_states.clone(),
            copy_path_mouse_states: self.copy_path_mouse_states.borrow().clone(),
            copy_branch_mouse_states: self.copy_branch_mouse_states.borrow().clone(),
            outcomes: self.outcomes.clone(),
            visible: self.compact_hovered || self.panel_hovered || self.panel_progress > 0.,
            status_phase: self.status_phase,
            reduced_motion: self.reduced_motion,
            panel_progress: self.panel_progress,
            sort_key: self.sort_key,
            sort_descending: self.sort_descending,
            now_ms,
            hovered_worktree: self.hovered_worktree.clone(),
        }
    }
}

fn update_worktree_mouse_states(
    states: &RefCell<HashMap<WorktreeKey, MouseStateHandle>>,
    keys: &HashSet<WorktreeKey>,
) {
    let mut states = states.borrow_mut();
    states.retain(|key, _| keys.contains(key));
    for key in keys {
        states.entry(key.clone()).or_default();
    }
}

#[derive(Clone, Debug)]
pub(crate) enum HeaderAction {
    SetCompactHovered(bool),
    SetPanelHovered(bool),
    SetWorktreeHovered(WorktreeKey),
    SetAgentHovered { key: AgentKey, hovered: bool },
    Dismiss,
    Activate { key: AgentKey, generation: u64 },
    Detach(AgentKey),
    CycleSort,
    ToggleSortDirection,
    CopyPath(WorktreeKey),
    CopyBranch(String),
}
