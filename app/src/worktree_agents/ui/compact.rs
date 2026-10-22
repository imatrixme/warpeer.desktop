use pathfinder_geometry::vector::vec2f;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill;
use warp_core::ui::theme::color::internal_colors;
use warpui::Element;
use warpui::elements::{
    Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Empty, Flex,
    Hoverable, MainAxisAlignment, MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement,
    ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};

use super::{HeaderAction, HeaderUiSnapshot, provider_icon, status_color};
use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;
use crate::worktree_agents::observation::AgentLifecycle;
use crate::worktree_agents::projections::{AgentProjection, WorktreeProjection};
use crate::worktree_agents::ui::status::lifecycle_priority;

const AVATAR_SIZE: f32 = 18.;
const AVATAR_GLYPH_SIZE: f32 = 11.;
const STATUS_DOT_SIZE: f32 = 4.;
const MAX_VISIBLE_WORKTREES: usize = 3;
const MAX_VISIBLE_AGENTS_PER_WORKTREE: usize = 3;
const HEADER_STRIP_MAX_WIDTH: f32 = 164.;
const WORKTREE_AGENT_OVERLAP: f32 = -4.;
const AGENT_OVERLAP: f32 = -7.;

pub(super) fn render(
    projections: &[WorktreeProjection],
    ui: &HeaderUiSnapshot,
    appearance: &Appearance,
    status_phase: bool,
    reduced_motion: bool,
) -> Box<dyn Element> {
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min);
    for projection in projections.iter().take(MAX_VISIBLE_WORKTREES) {
        let key = projection.key.clone();
        let render_projection = projection.clone();
        let group = Hoverable::new(ui.worktree_mouse_state(&key), move |_| {
            render_worktree_group(&render_projection, appearance, status_phase, reduced_motion)
        })
        .on_hover(move |hovered, ctx, _, _| {
            if hovered {
                ctx.dispatch_typed_action(HeaderAction::SetWorktreeHovered(key.clone()));
            }
        })
        .finish();
        row.add_child(Container::new(group).with_margin_right(4.).finish());
    }
    if projections.len() > MAX_VISIBLE_WORKTREES {
        row.add_child(render_overflow_count(
            projections.len() - MAX_VISIBLE_WORKTREES,
            appearance,
        ));
    }

    ConstrainedBox::new(row.finish())
        .with_max_width(HEADER_STRIP_MAX_WIDTH)
        .finish()
}

fn render_worktree_group(
    projection: &WorktreeProjection,
    appearance: &Appearance,
    status_phase: bool,
    reduced_motion: bool,
) -> Box<dyn Element> {
    let mut agents = projection.agents.clone();
    agents.sort_by_key(|agent| (lifecycle_priority(agent.lifecycle), agent.provider));
    let visible_agents: Vec<_> = agents
        .iter()
        .take(MAX_VISIBLE_AGENTS_PER_WORKTREE)
        .collect();
    let mut row = Flex::row()
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_main_axis_size(MainAxisSize::Min)
        .with_child(render_avatar_stack(
            &visible_agents,
            appearance,
            status_phase,
            reduced_motion,
        ));
    if agents.len() > MAX_VISIBLE_AGENTS_PER_WORKTREE {
        row.add_child(
            Container::new(render_overflow_count(
                agents.len() - MAX_VISIBLE_AGENTS_PER_WORKTREE,
                appearance,
            ))
            .with_margin_left(AGENT_OVERLAP + 1.)
            .finish(),
        );
    }
    let theme = appearance.theme();
    Container::new(row.finish())
        .with_background(theme.surface_2().with_opacity(72))
        .with_border(
            Border::all(1.)
                .with_border_color(internal_colors::fg_overlay_2(theme).with_opacity(55).into()),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
        .with_horizontal_padding(3.)
        .with_vertical_padding(2.)
        .finish()
}

fn render_avatar_stack(
    agents: &[&AgentProjection],
    appearance: &Appearance,
    status_phase: bool,
    reduced_motion: bool,
) -> Box<dyn Element> {
    let first_agent_x = AVATAR_SIZE + WORKTREE_AGENT_OVERLAP;
    let agent_step = AVATAR_SIZE + AGENT_OVERLAP;
    let width = agents.last().map_or(AVATAR_SIZE, |_| {
        first_agent_x + AVATAR_SIZE + agent_step * (agents.len().saturating_sub(1) as f32)
    });
    let base = ConstrainedBox::new(Empty::new().finish())
        .with_width(width)
        .with_height(AVATAR_SIZE)
        .finish();
    let mut stack = Stack::new().with_child(base);
    stack.add_positioned_child(
        render_worktree_avatar(appearance),
        top_left_position(0., 0.),
    );
    for (index, agent) in agents.iter().enumerate() {
        let x = first_agent_x + agent_step * index as f32;
        stack.add_positioned_child(
            render_agent_circle(agent, appearance),
            top_left_position(x, 0.),
        );
    }
    for (index, agent) in agents.iter().enumerate() {
        let x = first_agent_x + agent_step * index as f32 + AVATAR_SIZE - STATUS_DOT_SIZE;
        let y = AVATAR_SIZE - STATUS_DOT_SIZE;
        stack.add_positioned_child(
            render_status_dot(agent, appearance, status_phase, reduced_motion),
            top_left_position(x, y),
        );
    }
    ConstrainedBox::new(stack.finish())
        .with_width(width)
        .with_height(AVATAR_SIZE)
        .finish()
}

fn top_left_position(x: f32, y: f32) -> OffsetPositioning {
    OffsetPositioning::offset_from_parent(
        vec2f(x, y),
        ParentOffsetBounds::Unbounded,
        ParentAnchor::TopLeft,
        ChildAnchor::TopLeft,
    )
}

fn render_worktree_avatar(appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let background = theme.accent().with_opacity(24);
    let glyph = Text::new_inline("🌳", appearance.ui_font_family(), 11.)
        .with_color(theme.main_text_color(background).into())
        .with_style(Properties::default().weight(Weight::Semibold))
        .finish();
    let content = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::Center)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(glyph)
        .finish();
    ConstrainedBox::new(
        Container::new(content)
            .with_background(background)
            .with_border(Border::all(2.).with_border_color(theme.accent().with_opacity(72).into()))
            .with_corner_radius(CornerRadius::with_all(Radius::Percentage(50.)))
            .finish(),
    )
    .with_width(AVATAR_SIZE)
    .with_height(AVATAR_SIZE)
    .finish()
}

fn render_agent_circle(agent: &AgentProjection, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let background = Fill::Solid(internal_colors::neutral_4(theme));
    let icon_color = theme.main_text_color(background).into_solid();
    let icon = ConstrainedBox::new(
        provider_icon(agent.provider)
            .to_warpui_icon(Fill::Solid(icon_color))
            .finish(),
    )
    .with_width(AVATAR_GLYPH_SIZE)
    .with_height(AVATAR_GLYPH_SIZE)
    .finish();
    ConstrainedBox::new(
        Container::new(
            Flex::row()
                .with_main_axis_size(MainAxisSize::Max)
                .with_main_axis_alignment(MainAxisAlignment::Center)
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_child(icon)
                .finish(),
        )
        .with_background(background)
        .with_border(
            Border::all(1.).with_border_color(
                if agent.current_pane {
                    Fill::Solid(coloru_with_opacity(
                        super::status::current_pane_color(theme),
                        90,
                    ))
                } else {
                    internal_colors::fg_overlay_5(theme).with_opacity(55)
                }
                .into(),
            ),
        )
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(AVATAR_SIZE / 2.)))
        .finish(),
    )
    .with_width(AVATAR_SIZE)
    .with_height(AVATAR_SIZE)
    .finish()
}

fn render_status_dot(
    agent: &AgentProjection,
    appearance: &Appearance,
    status_phase: bool,
    reduced_motion: bool,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let animated = matches!(
        agent.lifecycle,
        AgentLifecycle::Detected
            | AgentLifecycle::Starting
            | AgentLifecycle::Thinking
            | AgentLifecycle::Working
    ) && !reduced_motion;
    let status_opacity = if animated && !status_phase { 58 } else { 100 };
    ConstrainedBox::new(
        Icon::Ellipse
            .to_warpui_icon(
                Fill::Solid(status_color(agent.lifecycle, theme)).with_opacity(status_opacity),
            )
            .finish(),
    )
    .with_width(STATUS_DOT_SIZE)
    .with_height(STATUS_DOT_SIZE)
    .finish()
}

fn render_overflow_count(count: usize, appearance: &Appearance) -> Box<dyn Element> {
    Text::new_inline(
        format!("+{count}"),
        appearance.ui_font_family(),
        appearance.ui_font_size() - 4.,
    )
    .with_color(
        appearance
            .theme()
            .sub_text_color(appearance.theme().background())
            .into(),
    )
    .with_style(Properties::default().weight(Weight::Semibold))
    .finish()
}
