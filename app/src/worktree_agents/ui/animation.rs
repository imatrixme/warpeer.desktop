use std::collections::HashSet;
use std::time::Duration;

use warpui::r#async::Timer;
use warpui::{SingletonEntity as _, ViewContext};

use super::WorktreeAgentHeaderView;
use crate::appearance::Appearance;
use crate::terminal::visual_runtime::TerminalVisualConfig;
use crate::worktree_agents::observation::AgentLifecycle;
use crate::worktree_agents::settings::WorktreeAgentSettings;

const STATUS_ANIMATION_INTERVAL: Duration = Duration::from_millis(800);
const PANEL_ANIMATION_INTERVAL: Duration = Duration::from_millis(16);
const PANEL_ANIMATION_STEP: f32 = 0.1;

impl WorktreeAgentHeaderView {
    pub(super) fn refresh_status_animation(&mut self, ctx: &mut ViewContext<Self>) {
        if !*WorktreeAgentSettings::as_ref(ctx).show_in_pane_header {
            self.ui.animation_generation = self.ui.animation_generation.wrapping_add(1);
            self.ui.animation_in_flight = false;
            self.ui.status_phase = false;
            self.ui.dismiss();
            return;
        }
        let projections = self
            .coordinator
            .as_ref(ctx)
            .projection_for_terminal(self.terminal_view_id);
        let keys: HashSet<_> = projections
            .iter()
            .flat_map(|projection| projection.agents.iter().map(|agent| agent.key.clone()))
            .collect();
        self.ui.outcomes.retain(|key, _| keys.contains(key));
        if projections.is_empty() {
            self.ui.dismiss();
        }
        let theme = Appearance::as_ref(ctx).theme();
        let reduced_motion = TerminalVisualConfig::from_app(
            ctx,
            theme.background().into(),
            theme.accent().into(),
            theme.foreground().into(),
        )
        .reduce_motion;
        self.ui.reduced_motion = reduced_motion;
        let active = projections.iter().any(|projection| {
            projection.agents.iter().any(|agent| {
                matches!(
                    agent.lifecycle,
                    AgentLifecycle::Starting | AgentLifecycle::Thinking | AgentLifecycle::Working
                )
            })
        });
        if reduced_motion || !active {
            self.ui.animation_generation = self.ui.animation_generation.wrapping_add(1);
            self.ui.animation_in_flight = false;
            self.ui.status_phase = false;
            return;
        }
        if self.ui.animation_in_flight {
            return;
        }
        self.ui.animation_generation = self.ui.animation_generation.wrapping_add(1);
        self.ui.animation_in_flight = true;
        let generation = self.ui.animation_generation;
        ctx.spawn(
            async move {
                Timer::after(STATUS_ANIMATION_INTERVAL).await;
                generation
            },
            Self::advance_status_animation,
        );
    }

    fn advance_status_animation(&mut self, generation: usize, ctx: &mut ViewContext<Self>) {
        if generation != self.ui.animation_generation {
            return;
        }
        self.ui.animation_in_flight = false;
        self.ui.status_phase = !self.ui.status_phase;
        ctx.notify();
        self.refresh_status_animation(ctx);
    }

    pub(super) fn refresh_panel_animation(&mut self, ctx: &mut ViewContext<Self>) {
        let target_visible = self.ui.compact_hovered || self.ui.panel_hovered;
        let target = if target_visible { 1. } else { 0. };
        self.ui.panel_animation_generation = self.ui.panel_animation_generation.wrapping_add(1);
        let generation = self.ui.panel_animation_generation;
        if self.ui.reduced_motion || self.ui.panel_progress == target {
            self.ui.panel_progress = target;
            self.ui.panel_animation_in_flight = false;
            return;
        }
        self.ui.panel_animation_in_flight = true;
        ctx.spawn(
            async move {
                Timer::after(PANEL_ANIMATION_INTERVAL).await;
                (generation, target_visible)
            },
            Self::advance_panel_animation,
        );
    }

    fn advance_panel_animation(
        &mut self,
        (generation, target_visible): (usize, bool),
        ctx: &mut ViewContext<Self>,
    ) {
        if generation != self.ui.panel_animation_generation {
            return;
        }
        let target = if target_visible { 1. } else { 0. };
        let direction = if target_visible { 1. } else { -1. };
        self.ui.panel_progress =
            (self.ui.panel_progress + direction * PANEL_ANIMATION_STEP).clamp(0., 1.);
        ctx.notify();
        if self.ui.panel_progress == target {
            self.ui.panel_animation_in_flight = false;
            return;
        }
        ctx.spawn(
            async move {
                Timer::after(PANEL_ANIMATION_INTERVAL).await;
                (generation, target_visible)
            },
            Self::advance_panel_animation,
        );
    }
}
