use pathfinder_geometry::vector::vec2f;
use warp_core::ui::color::coloru_with_opacity;
use warp_core::ui::theme::Fill as ThemeFill;
use warpui::Element;
use warpui::elements::{
    Border, ChildAnchor, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment,
    DispatchEventResult, Empty, EventHandler, Expanded, Flex, Hoverable, MainAxisAlignment,
    MainAxisSize, OffsetPositioning, ParentAnchor, ParentElement, ParentOffsetBounds, Radius,
    SavePosition, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;

use super::{
    HeaderAction, HeaderActionOutcome, HeaderUiSnapshot, detach_button, pane_connector,
    provider_icon, session_metrics, status_color,
};
use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;
#[cfg(test)]
use crate::worktree_agents::observation::SourceAuthority;
use crate::worktree_agents::observation::{AgentLifecycle, SourceHealth};
use crate::worktree_agents::projections::{
    AgentActivationKind, AgentProjection, AgentRouteProjection,
};
use crate::worktree_agents::providers::AgentProvider;
use crate::worktree_agents::routing::RouteConfidence;
use crate::worktree_agents::runtimes::AgentRuntimeHint;

const AGENT_ICON_SIZE: f32 = 18.;
const STATUS_DOT_SIZE: f32 = 4.;

pub(super) fn render(
    agent: AgentProjection,
    worktree_available: bool,
    ui: &HeaderUiSnapshot,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let position_id = pane_connector::agent_row_position_id(&agent.key);
    let mouse_state = ui.agent_mouse_state(&agent.key);
    let detach_mouse_state = ui.detach_mouse_state(&agent.key);
    let outcome = ui.outcome(&agent.key);
    let route = agent.route;
    let now_ms = ui.now_ms();
    let clickable = is_clickable(route);
    let render_agent = agent.clone();
    let hover_key = agent.key.clone();
    let row = Hoverable::new(mouse_state, move |state| {
        render_content(
            &render_agent,
            worktree_available,
            outcome,
            state.is_hovered(),
            clickable,
            detach_mouse_state.clone(),
            now_ms,
            appearance,
        )
    })
    .on_hover(move |hovered, ctx, _, _| {
        ctx.dispatch_typed_action(HeaderAction::SetAgentHovered {
            key: hover_key.clone(),
            hovered,
        });
    })
    .finish();
    let row = if clickable {
        let key = agent.key;
        let generation = route.map(|route| route.generation).unwrap_or_default();
        EventHandler::new(row)
            .on_left_mouse_down(move |ctx, _, _| {
                ctx.dispatch_typed_action(HeaderAction::Activate {
                    key: key.clone(),
                    generation,
                });
                DispatchEventResult::StopPropagation
            })
            .finish()
    } else {
        row
    };
    SavePosition::new(row, &position_id).finish()
}

fn render_content(
    agent: &AgentProjection,
    worktree_available: bool,
    outcome: Option<HeaderActionOutcome>,
    hovered: bool,
    clickable: bool,
    detach_mouse_state: warpui::elements::MouseStateHandle,
    now_ms: u64,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let task_text = agent
        .metrics
        .session_title
        .as_ref()
        .or(agent.label.as_ref())
        .map(|label| label.as_str().to_string())
        .unwrap_or_else(|| format!("Untitled {} session", provider_name(agent.provider)));
    let header = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Expanded::new(
                1.,
                Text::new_inline(task_text, appearance.ui_font_family(), 11.)
                    .with_clip(ClipConfig::ellipsis())
                    .with_style(Properties::default().weight(Weight::Semibold))
                    .with_color(theme.main_text_color(theme.surface_2()).into())
                    .finish(),
            )
            .finish(),
        )
        .with_child(status_pill(agent.lifecycle, appearance))
        .finish();
    let identity = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Text::new_inline(
                provider_name(agent.provider),
                appearance.ui_font_family(),
                9.,
            )
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_color(theme.sub_text_color(theme.surface_2()).into())
            .finish(),
        )
        .with_child(
            Container::new(
                Text::new_inline(
                    session_short_id(&agent.key.session_id),
                    appearance.ui_font_family(),
                    9.,
                )
                .with_color(theme.sub_text_color(theme.surface_2()).into())
                .finish(),
            )
            .with_margin_left(5.)
            .finish(),
        )
        .with_child(
            Container::new(
                Text::new_inline(runtime_name(agent.runtime), appearance.ui_font_family(), 9.)
                    .with_color(theme.sub_text_color(theme.surface_2()).into())
                    .finish(),
            )
            .with_margin_left(5.)
            .finish(),
        )
        .with_child(Expanded::new(1., Empty::new().finish()).finish())
        .with_child(attachment_pill(agent, appearance))
        .finish();
    let action = Text::new_inline(
        action_label(
            agent.provider,
            agent.source_health,
            worktree_available,
            agent.current_pane,
            agent.route,
            outcome,
        ),
        appearance.ui_font_family(),
        9.,
    )
    .with_color(if clickable {
        theme.accent().into()
    } else {
        theme.sub_text_color(theme.surface_2()).into()
    })
    .finish();
    let mut action_row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Expanded::new(1., Empty::new().finish()).finish())
        .with_child(action);
    if is_attached(agent.route) {
        action_row.add_child(
            Container::new(detach_button::render(
                agent.key.clone(),
                detach_mouse_state,
                appearance,
            ))
            .with_margin_left(5.)
            .finish(),
        );
    }
    let text = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(header)
        .with_child(Container::new(identity).with_margin_top(3.).finish())
        .with_child(
            Container::new(session_metrics::render(agent, now_ms, appearance))
                .with_margin_top(3.)
                .finish(),
        )
        .with_child(action_row.finish())
        .finish();
    let content = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_main_axis_alignment(MainAxisAlignment::Start)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            Container::new(agent_icon(agent, appearance))
                .with_margin_right(8.)
                .finish(),
        )
        .with_child(Expanded::new(1., text).finish())
        .finish();
    let border_fill = if agent.current_pane {
        theme.accent().with_opacity(46)
    } else {
        theme.outline()
    };
    let card = Container::new(content)
        .with_background(if hovered {
            theme.accent().with_opacity(13)
        } else {
            theme.surface_2().with_opacity(78)
        })
        .with_border(Border::all(1.).with_border_fill(border_fill))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_horizontal_padding(8.)
        .with_padding_top(if agent.current_pane { 18. } else { 6. })
        .with_padding_bottom(6.)
        .finish();
    let mut stack = Stack::new().with_child(card);
    if agent.current_pane {
        stack.add_positioned_child(
            current_pane_badge(appearance),
            OffsetPositioning::offset_from_parent(
                vec2f(6., 3.),
                ParentOffsetBounds::Unbounded,
                ParentAnchor::TopLeft,
                ChildAnchor::TopLeft,
            ),
        );
    }
    Container::new(stack.finish())
        .with_margin_bottom(3.)
        .finish()
}

fn current_pane_badge(appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let color = super::status::current_pane_color(theme);
    Container::new(
        Text::new_inline("Current pane", appearance.ui_font_family(), 8.)
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_color(theme.main_text_color(ThemeFill::Solid(color)).into())
            .finish(),
    )
    .with_background(ThemeFill::Solid(color).with_opacity(88))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
    .with_horizontal_padding(5.)
    .with_vertical_padding(1.)
    .finish()
}

fn agent_icon(agent: &AgentProjection, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let icon_color = theme.main_text_color(theme.surface_2()).into_solid();
    let icon = ConstrainedBox::new(
        provider_icon(agent.provider)
            .to_warpui_icon(ThemeFill::Solid(icon_color))
            .finish(),
    )
    .with_width(AGENT_ICON_SIZE)
    .with_height(AGENT_ICON_SIZE)
    .finish();
    let dot = ConstrainedBox::new(
        Icon::Ellipse
            .to_warpui_icon(ThemeFill::Solid(status_color(agent.lifecycle, theme)))
            .finish(),
    )
    .with_width(STATUS_DOT_SIZE)
    .with_height(STATUS_DOT_SIZE)
    .finish();
    let mut stack = Stack::new().with_child(icon);
    stack.add_positioned_child(
        dot,
        OffsetPositioning::offset_from_parent(
            vec2f(1., 1.),
            ParentOffsetBounds::Unbounded,
            ParentAnchor::BottomRight,
            ChildAnchor::BottomRight,
        ),
    );
    ConstrainedBox::new(stack.finish())
        .with_width(AGENT_ICON_SIZE)
        .with_height(AGENT_ICON_SIZE)
        .finish()
}

fn status_pill(lifecycle: AgentLifecycle, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    Container::new(
        Text::new_inline(lifecycle_name(lifecycle), appearance.ui_font_family(), 8.)
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_color(theme.main_text_color(theme.surface_3()).into())
            .finish(),
    )
    .with_background(coloru_with_opacity(status_color(lifecycle, theme), 18))
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.)))
    .with_horizontal_padding(5.)
    .with_vertical_padding(1.)
    .with_margin_left(5.)
    .finish()
}

fn attachment_pill(agent: &AgentProjection, appearance: &Appearance) -> Box<dyn Element> {
    let theme = appearance.theme();
    let attached_elsewhere = agent.route.is_some_and(|route| {
        route.kind == AgentActivationKind::FocusPane && route.confidence == RouteConfidence::Exact
    });
    let (label, active) = if agent.current_pane {
        ("Current pane", true)
    } else if attached_elsewhere {
        ("Attached elsewhere", true)
    } else {
        ("Available", false)
    };
    Container::new(
        Text::new_inline(label, appearance.ui_font_family(), 8.)
            .with_style(Properties::default().weight(Weight::Semibold))
            .with_color(if active {
                theme.accent().into()
            } else {
                theme.sub_text_color(theme.surface_3()).into()
            })
            .finish(),
    )
    .with_background(if active {
        theme.accent().with_opacity(16)
    } else {
        theme.surface_3().with_opacity(78)
    })
    .with_corner_radius(CornerRadius::with_all(Radius::Pixels(5.)))
    .with_horizontal_padding(5.)
    .with_vertical_padding(1.)
    .with_margin_right(5.)
    .finish()
}

pub(super) fn is_clickable(route: Option<AgentRouteProjection>) -> bool {
    route.is_some_and(|route| {
        route.confidence == RouteConfidence::Exact
            && matches!(
                route.kind,
                AgentActivationKind::FocusPane
                    | AgentActivationKind::AttachTmuxPane
                    | AgentActivationKind::AttachHerdrAgent
                    | AgentActivationKind::ResumeAgent
            )
    })
}

pub(super) fn is_attached(route: Option<AgentRouteProjection>) -> bool {
    route.is_some_and(|route| {
        route.kind == AgentActivationKind::FocusPane && route.confidence == RouteConfidence::Exact
    })
}

fn session_short_id(session_id: &str) -> String {
    let value: String = session_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(8)
        .collect();
    if value.is_empty() {
        "session".to_string()
    } else {
        format!("#{value}")
    }
}

fn provider_name(provider: AgentProvider) -> &'static str {
    provider.display_name()
}

fn lifecycle_name(lifecycle: AgentLifecycle) -> &'static str {
    match lifecycle {
        AgentLifecycle::Detected => "State unknown",
        AgentLifecycle::Starting => "Starting",
        AgentLifecycle::Thinking => "Thinking",
        AgentLifecycle::Working => "Working",
        AgentLifecycle::NeedsInput => "Needs input",
        AgentLifecycle::ReadyForReview => "Awaiting input",
        AgentLifecycle::Completed => "Completed",
        AgentLifecycle::Failed => "Failed",
        AgentLifecycle::Stopped => "Stopped",
        AgentLifecycle::Stale => "Stale",
    }
}

fn runtime_name(runtime: AgentRuntimeHint) -> &'static str {
    match runtime {
        AgentRuntimeHint::NormalPty => "terminal",
        AgentRuntimeHint::Tmux => "tmux",
        AgentRuntimeHint::Herdr => "Herdr",
        AgentRuntimeHint::Unknown => "runtime unknown",
    }
}

#[cfg(test)]
fn source_name(authority: SourceAuthority, health: SourceHealth) -> &'static str {
    match (authority, health) {
        (_, SourceHealth::Unavailable) => "source unavailable",
        (_, SourceHealth::Degraded) => "source degraded",
        (SourceAuthority::StructuredEvent, SourceHealth::Healthy) => "event live",
        (SourceAuthority::ProviderSnapshot, SourceHealth::Healthy) => "snapshot live",
        (SourceAuthority::ProcessInference, SourceHealth::Healthy) => "process inferred",
    }
}

#[cfg(test)]
fn route_confidence_name(route: Option<AgentRouteProjection>) -> &'static str {
    match route.map(|route| route.confidence) {
        Some(RouteConfidence::Exact) => "exact route",
        Some(RouteConfidence::Inferred) => "inferred route",
        Some(RouteConfidence::Stale) => "stale route",
        None => "no route",
    }
}

fn action_label(
    provider: AgentProvider,
    source_health: SourceHealth,
    worktree_available: bool,
    current_pane: bool,
    route: Option<AgentRouteProjection>,
    outcome: Option<HeaderActionOutcome>,
) -> &'static str {
    if current_pane {
        return "Current pane";
    }
    if let Some(outcome) = outcome {
        return match outcome {
            HeaderActionOutcome::Activated => "Switched",
            HeaderActionOutcome::AlreadyFocused => "Current pane",
            HeaderActionOutcome::Stale => "Route expired",
            HeaderActionOutcome::ObserveOnly => "Observe only",
        };
    }
    if !worktree_available {
        return "Worktree unavailable";
    }
    if source_health == SourceHealth::Unavailable {
        return "Provider unavailable";
    }
    let Some(route) = route else {
        return match provider {
            AgentProvider::ClaudeCode | AgentProvider::Codex => "Resume unavailable",
            AgentProvider::Herdr => "No exact Herdr route",
            AgentProvider::Cli(_) | AgentProvider::Unknown => "Resume unsupported",
        };
    };
    match route.confidence {
        RouteConfidence::Stale => return "Route expired",
        RouteConfidence::Inferred => return "Observe only",
        RouteConfidence::Exact => {}
    }
    match route.kind {
        AgentActivationKind::FocusPane => "Focus attached pane",
        AgentActivationKind::AttachTmuxPane | AgentActivationKind::AttachHerdrAgent => {
            "Switch here"
        }
        AgentActivationKind::ResumeAgent => "Resume here",
        AgentActivationKind::ObserveOnly => "Observe only",
    }
}

#[cfg(test)]
#[path = "session_row_tests.rs"]
mod tests;
