mod catalog;
mod catalog_dirty;
mod coordinator;
mod identity;
mod launch;
mod navigation;
mod observation;
mod pane_index;
mod projections;
mod prompt_catalog;
mod providers;
mod routing;
mod runtimes;
mod session_info;
mod settings;
mod sources;
mod ui;

pub(crate) use catalog::{
    WorktreeDirtyState, WorktreeMenuEntry, context_chip, decode_menu_entries,
    filter_on_click_values, normalize_chip_value, prepend_prompt_chip,
};
pub(crate) use coordinator::WorktreeAgentCoordinator;
pub(crate) use identity::ExecutionHostKey;
#[cfg(feature = "integration_tests")]
pub(crate) use launch::NativeResumeTarget;
pub(crate) use launch::{CurrentPaneLaunchResult, WorktreeAgentLaunch};
pub(crate) use navigation::WorktreeNavigationDecision;
pub(crate) use observation::AgentKey;
pub(crate) use prompt_catalog::publish_prompt_catalog;
#[cfg(feature = "integration_tests")]
pub(crate) use providers::AgentProvider;
pub(crate) use runtimes::herdr_focus_command;
pub(crate) use settings::{
    WorktreeAgentPaneLinkEffect, WorktreeAgentSettings, WorktreeAgentSortKey,
};
pub(crate) use ui::{WorktreeAgentHeaderView, pane_header_position_id};
use warpui::AppContext;

pub(crate) fn init(ctx: &mut AppContext) {
    if crate::product_profile::allows_worktree_agent_control_center()
        && !ctx.has_singleton_model::<WorktreeAgentCoordinator>()
    {
        ctx.add_singleton_model(WorktreeAgentCoordinator::new);
    }
}
