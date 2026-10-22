use std::time::Duration;

use pathfinder_geometry::vector::vec2f;
use warpui::clipboard::ClipboardContent;
use warpui::elements::{
    ChildAnchor, Empty, Hoverable, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Stack,
};
use warpui::{
    AppContext, Element, Entity, EntityId, ModelHandle, SingletonEntity as _, TypedActionView,
    View, ViewContext, WeakViewHandle,
};

use super::coordinator::WorktreeAgentCoordinator;
use super::observation::{AgentKey, AgentLifecycle};
use super::projections::{AgentActivationKind, AgentRouteProjection, WorktreeProjection};
use super::routing::RouteConfidence;
use super::settings::{WorktreeAgentPaneLinkEffect, WorktreeAgentSettings};
use crate::appearance::Appearance;
use crate::settings::TerminalVisualSettings;
use crate::terminal::view::TerminalView;

#[path = "ui/activation.rs"]
mod activation;

#[path = "ui/animation.rs"]
mod animation;

#[path = "ui/compact.rs"]
mod compact;

#[path = "ui/detach_button.rs"]
mod detach_button;

#[path = "ui/overview.rs"]
mod overview;

#[path = "ui/pane_connector.rs"]
mod pane_connector;

#[path = "ui/session_row.rs"]
mod session_row;

#[path = "ui/session_metrics.rs"]
mod session_metrics;

#[path = "ui/sorting.rs"]
mod sorting;

#[path = "ui/status.rs"]
mod status;

#[path = "ui/subagent_row.rs"]
mod subagent_row;

#[path = "ui/state.rs"]
mod state;

pub(crate) use pane_connector::pane_header_position_id;
use state::{HeaderAction, HeaderActionOutcome, HeaderUiSnapshot, HeaderUiState};
use status::{provider_icon, status_color};

const OVERVIEW_OFFSET_Y: f32 = 5.;
const HOVER_IN_DELAY: Duration = Duration::from_millis(280);
const HOVER_OUT_DELAY: Duration = Duration::from_millis(220);

pub(crate) struct WorktreeAgentHeaderView {
    terminal_view_id: EntityId,
    terminal_view: WeakViewHandle<TerminalView>,
    coordinator: ModelHandle<WorktreeAgentCoordinator>,
    ui: HeaderUiState,
}

impl WorktreeAgentHeaderView {
    pub(crate) fn new(
        terminal_view_id: EntityId,
        terminal_view: WeakViewHandle<TerminalView>,
        ctx: &mut ViewContext<Self>,
    ) -> Self {
        let coordinator = WorktreeAgentCoordinator::handle(ctx);
        ctx.subscribe_to_model(&coordinator, |view, coordinator, _, ctx| {
            let owned_elsewhere = coordinator.read(ctx, |coordinator, _| {
                coordinator
                    .active_header_panel()
                    .is_some_and(|owner| owner != view.terminal_view_id)
            });
            if owned_elsewhere {
                view.ui.dismiss();
            }
            view.refresh_status_animation(ctx);
            ctx.emit(());
            ctx.notify();
        });
        ctx.observe(&TerminalVisualSettings::handle(ctx), |view, _, ctx| {
            view.refresh_status_animation(ctx);
            ctx.notify();
        });
        ctx.observe(&WorktreeAgentSettings::handle(ctx), |view, _, ctx| {
            view.ui.dismiss();
            let terminal_view_id = view.terminal_view_id;
            view.coordinator.update(ctx, |coordinator, ctx| {
                if coordinator.set_header_panel_active(terminal_view_id, false, ctx) {
                    coordinator.clear_agent_hover(ctx);
                }
            });
            if !view.ui.sort_overridden {
                view.ui.reset_sort(WorktreeAgentSettings::as_ref(ctx));
            }
            view.refresh_status_animation(ctx);
            ctx.emit(());
            ctx.notify();
        });
        let consumer = ctx.handle();
        coordinator.update(ctx, |coordinator, _| {
            coordinator.register_header_consumer(terminal_view_id, consumer);
        });
        let mut ui = HeaderUiState::default();
        ui.reset_sort(WorktreeAgentSettings::as_ref(ctx));
        let mut view = Self {
            terminal_view_id,
            terminal_view,
            coordinator,
            ui,
        };
        view.refresh_status_animation(ctx);
        view
    }

    pub(crate) fn dismiss(&mut self, ctx: &mut ViewContext<Self>) {
        self.ui.dismiss();
        let terminal_view_id = self.terminal_view_id;
        self.coordinator.update(ctx, |coordinator, ctx| {
            if coordinator.set_header_panel_active(terminal_view_id, false, ctx) {
                coordinator.clear_agent_hover(ctx);
            }
        });
        ctx.notify();
    }

    fn sync_panel_ownership(&mut self, ctx: &mut ViewContext<Self>) {
        let active = self.ui.panel_hovered();
        let terminal_view_id = self.terminal_view_id;
        self.coordinator.update(ctx, |coordinator, ctx| {
            let changed = coordinator.set_header_panel_active(terminal_view_id, active, ctx);
            if !active && changed {
                coordinator.clear_agent_hover(ctx);
            }
        });
    }

    pub(crate) fn pane_link_titlebar_opacity(&self, app: &AppContext) -> Option<u8> {
        if !self
            .coordinator
            .as_ref(app)
            .hovered_agent_targets_pane(self.terminal_view_id)
        {
            return None;
        }
        match *WorktreeAgentSettings::as_ref(app).pane_link_effect {
            WorktreeAgentPaneLinkEffect::Off => None,
            WorktreeAgentPaneLinkEffect::Highlight => Some(24),
            WorktreeAgentPaneLinkEffect::Pulse => Some(18),
        }
    }

    fn pane_connector(&self, app: &AppContext) -> Option<(AgentKey, EntityId)> {
        if !matches!(
            *WorktreeAgentSettings::as_ref(app).pane_link_effect,
            WorktreeAgentPaneLinkEffect::Pulse
        ) {
            return None;
        }
        let coordinator = self.coordinator.as_ref(app);
        let target_pane = coordinator.hovered_agent_target_pane()?;
        if target_pane == self.terminal_view_id {
            return None;
        }
        Some((coordinator.hovered_agent_key()?.clone(), target_pane))
    }

    pub(crate) fn has_active_agents(&self, app: &AppContext) -> bool {
        *WorktreeAgentSettings::as_ref(app).show_in_pane_header
            && self
                .coordinator
                .as_ref(app)
                .projection_for_terminal(self.terminal_view_id)
                .iter()
                .any(|projection| {
                    projection.agents.iter().any(|agent| {
                        should_show_agent(agent.lifecycle, agent.route, agent.relation.is_some())
                    })
                })
    }
}

fn render_header(view: &WorktreeAgentHeaderView, app: &AppContext) -> Box<dyn Element> {
    let settings = WorktreeAgentSettings::as_ref(app);
    if !*settings.show_in_pane_header {
        return Empty::new().finish();
    }
    let coordinator = view.coordinator.as_ref(app);
    let now_ms = coordinator.now_ms();
    let mut projections = coordinator.projection_for_terminal(view.terminal_view_id);
    for projection in &mut projections {
        projection.agents.retain(|agent| {
            should_show_agent(agent.lifecycle, agent.route, agent.relation.is_some())
        });
    }
    projections.retain(|projection| !projection.agents.is_empty());
    if projections.is_empty() {
        return Empty::new().finish();
    }
    let ui = view.ui.snapshot(&projections, now_ms);
    let appearance = Appearance::as_ref(app);
    let compact_projections = projections.clone();
    let compact_ui = ui.clone();
    let status_phase = ui.status_phase;
    let reduced_motion = ui.reduced_motion;
    let compact = Hoverable::new(ui.compact_mouse_state.clone(), move |_| {
        compact::render(
            &compact_projections,
            &compact_ui,
            appearance,
            status_phase,
            reduced_motion,
        )
    })
    .with_hover_in_delay(HOVER_IN_DELAY)
    .with_hover_out_delay(HOVER_OUT_DELAY)
    .on_hover(move |hovered, ctx, _, _| {
        ctx.dispatch_typed_action(HeaderAction::SetCompactHovered(hovered));
    })
    .finish();

    let mut stack = Stack::new().with_child(compact);
    if ui.visible {
        let overview_projections =
            selected_worktree_projections(&projections, ui.hovered_worktree());
        stack.add_positioned_overlay_child(
            overview::render(
                overview_projections,
                ui,
                view.pane_connector(app),
                appearance,
            ),
            OffsetPositioning::offset_from_parent(
                vec2f(0., OVERVIEW_OFFSET_Y),
                ParentOffsetBounds::WindowByPosition,
                ParentAnchor::BottomLeft,
                ChildAnchor::TopLeft,
            ),
        );
    }
    stack.finish()
}

fn selected_worktree_projections(
    projections: &[WorktreeProjection],
    hovered_worktree: Option<&super::identity::WorktreeKey>,
) -> Vec<WorktreeProjection> {
    hovered_worktree
        .and_then(|key| projections.iter().find(|projection| projection.key == *key))
        .or_else(|| projections.iter().find(|projection| projection.current))
        .or_else(|| projections.first())
        .cloned()
        .into_iter()
        .collect()
}

fn should_show_agent(
    lifecycle: AgentLifecycle,
    route: Option<AgentRouteProjection>,
    subagent: bool,
) -> bool {
    lifecycle.is_active()
        && route.is_some_and(|route| {
            route.confidence == RouteConfidence::Exact
                && (matches!(
                    route.kind,
                    AgentActivationKind::FocusPane
                        | AgentActivationKind::AttachTmuxPane
                        | AgentActivationKind::AttachHerdrAgent
                ) || subagent && route.kind == AgentActivationKind::ResumeAgent)
        })
}

impl Entity for WorktreeAgentHeaderView {
    type Event = ();
}

impl View for WorktreeAgentHeaderView {
    fn ui_name() -> &'static str {
        "WorktreeAgentHeaderView"
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        render_header(self, app)
    }
}

impl TypedActionView for WorktreeAgentHeaderView {
    type Action = HeaderAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            HeaderAction::SetCompactHovered(hovered) => {
                if *hovered && self.ui.panel_progress == 0. {
                    self.ui.reset_sort(WorktreeAgentSettings::as_ref(ctx));
                }
                self.ui.set_compact_hovered(*hovered);
                self.sync_panel_ownership(ctx);
                self.refresh_panel_animation(ctx);
            }
            HeaderAction::SetPanelHovered(hovered) => {
                self.ui.set_panel_hovered(*hovered);
                self.sync_panel_ownership(ctx);
                self.refresh_panel_animation(ctx);
            }
            HeaderAction::SetWorktreeHovered(key) => {
                self.ui.hovered_worktree = Some(key.clone());
            }
            HeaderAction::SetAgentHovered { key, hovered } => {
                self.coordinator.update(ctx, |coordinator, ctx| {
                    coordinator.set_agent_hovered(key, *hovered, ctx);
                });
            }
            HeaderAction::Dismiss => self.dismiss(ctx),
            HeaderAction::Activate { key, generation } => {
                let outcome = activation::activate(self, key, *generation, ctx);
                self.ui.set_outcome(key.clone(), outcome);
            }
            HeaderAction::Detach(key) => activation::detach(self, key, ctx),
            HeaderAction::CycleSort => self.ui.cycle_sort(),
            HeaderAction::ToggleSortDirection => self.ui.toggle_sort_direction(),
            HeaderAction::CopyPath(key) => {
                ctx.clipboard().write(ClipboardContent::plain_text(
                    key.path().to_string_lossy().into_owned(),
                ));
                activation::show_notice("Worktree path copied", ctx);
            }
            HeaderAction::CopyBranch(branch) => {
                ctx.clipboard()
                    .write(ClipboardContent::plain_text(branch.clone()));
                activation::show_notice("Branch copied", ctx);
            }
        }
        ctx.notify();
    }
}

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
