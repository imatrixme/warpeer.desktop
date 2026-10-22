use warp_core::ui::theme::Fill as ThemeFill;
use warpui::Element;
use warpui::elements::{
    Border, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, DispatchEventResult,
    EventHandler, Expanded, Flex, Hoverable, MainAxisSize, ParentElement, Radius, SavePosition,
    Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::text_layout::ClipConfig;

use super::{
    HeaderAction, HeaderActionOutcome, HeaderUiSnapshot, detach_button, pane_connector,
    provider_icon, session_row,
};
use crate::appearance::Appearance;
use crate::worktree_agents::projections::{AgentActivationKind, AgentProjection};

const ICON_SIZE: f32 = 14.;
const BASE_INDENT: f32 = 16.;
const DEPTH_INDENT: f32 = 10.;

pub(super) fn render(
    agent: AgentProjection,
    depth: usize,
    ui: &HeaderUiSnapshot,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let position_id = pane_connector::agent_row_position_id(&agent.key);
    let mouse_state = ui.agent_mouse_state(&agent.key);
    let detach_mouse_state = ui.detach_mouse_state(&agent.key);
    let outcome = ui.outcome(&agent.key);
    let clickable = session_row::is_clickable(agent.route);
    let render_agent = agent.clone();
    let hover_key = agent.key.clone();
    let row = Hoverable::new(mouse_state, move |state| {
        render_content(
            &render_agent,
            outcome,
            state.is_hovered(),
            clickable,
            detach_mouse_state.clone(),
            depth,
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
        let generation = agent
            .route
            .map(|route| route.generation)
            .unwrap_or_default();
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
    Container::new(SavePosition::new(row, &position_id).finish())
        .with_margin_left(BASE_INDENT + DEPTH_INDENT * depth.saturating_sub(1) as f32)
        .with_margin_bottom(2.)
        .finish()
}

fn render_content(
    agent: &AgentProjection,
    outcome: Option<HeaderActionOutcome>,
    hovered: bool,
    clickable: bool,
    detach_mouse_state: warpui::elements::MouseStateHandle,
    depth: usize,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let color = super::status::current_pane_color(theme);
    let title = agent
        .relation
        .as_ref()
        .and_then(|relation| relation.role.as_ref())
        .or(agent.metrics.session_title.as_ref())
        .or(agent.label.as_ref())
        .map(|label| label.as_str().to_string())
        .unwrap_or_else(|| format!("{} subagent", agent.provider.display_name()));
    let icon_color = theme.main_text_color(theme.surface_2()).into_solid();
    let icon = ConstrainedBox::new(
        provider_icon(agent.provider)
            .to_warpui_icon(ThemeFill::Solid(icon_color))
            .finish(),
    )
    .with_width(ICON_SIZE)
    .with_height(ICON_SIZE)
    .finish();
    let metadata = format!(
        "{} · {} · level {}",
        agent.provider.display_name(),
        lifecycle_name(agent.lifecycle),
        depth
    );
    let text = Flex::column()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
        .with_child(
            Text::new_inline(title, appearance.ui_font_family(), 10.)
                .with_clip(ClipConfig::ellipsis())
                .with_style(Properties::default().weight(Weight::Semibold))
                .with_color(theme.main_text_color(theme.surface_2()).into())
                .finish(),
        )
        .with_child(
            Text::new_inline(metadata, appearance.ui_font_family(), 8.)
                .with_color(theme.sub_text_color(theme.surface_2()).into())
                .finish(),
        )
        .finish();
    let mut row = Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(icon)
        .with_child(
            Container::new(Expanded::new(1., text).finish())
                .with_margin_left(6.)
                .finish(),
        )
        .with_child(
            Text::new_inline(
                action_label(agent, outcome),
                appearance.ui_font_family(),
                8.,
            )
            .with_color(if agent.current_pane {
                color.into()
            } else if clickable {
                theme.accent().into()
            } else {
                theme.sub_text_color(theme.surface_2()).into()
            })
            .finish(),
        );
    if session_row::is_attached(agent.route) {
        row.add_child(
            Container::new(detach_button::render(
                agent.key.clone(),
                detach_mouse_state,
                appearance,
            ))
            .with_margin_left(4.)
            .finish(),
        );
    }
    Container::new(row.finish())
        .with_background(if hovered {
            theme.accent().with_opacity(10)
        } else {
            theme.surface_2().with_opacity(58)
        })
        .with_border(Border::all(1.).with_border_fill(if agent.current_pane {
            ThemeFill::Solid(color).with_opacity(62)
        } else {
            theme.outline()
        }))
        .with_corner_radius(CornerRadius::with_all(Radius::Pixels(4.)))
        .with_horizontal_padding(6.)
        .with_vertical_padding(4.)
        .finish()
}

fn action_label(agent: &AgentProjection, outcome: Option<HeaderActionOutcome>) -> &'static str {
    if agent.current_pane {
        return "Current";
    }
    if let Some(outcome) = outcome {
        return match outcome {
            HeaderActionOutcome::Activated => "Switched",
            HeaderActionOutcome::AlreadyFocused => "Current",
            HeaderActionOutcome::Stale => "Expired",
            HeaderActionOutcome::ObserveOnly => "Observe",
        };
    }
    match agent.route.map(|route| route.kind) {
        Some(AgentActivationKind::FocusPane) => "Attached",
        Some(
            AgentActivationKind::AttachTmuxPane
            | AgentActivationKind::AttachHerdrAgent
            | AgentActivationKind::ResumeAgent,
        ) => "Switch here",
        Some(AgentActivationKind::ObserveOnly) | None => "Observe",
    }
}

fn lifecycle_name(lifecycle: crate::worktree_agents::observation::AgentLifecycle) -> &'static str {
    use crate::worktree_agents::observation::AgentLifecycle;

    match lifecycle {
        AgentLifecycle::Detected => "unknown",
        AgentLifecycle::Starting => "starting",
        AgentLifecycle::Thinking => "thinking",
        AgentLifecycle::Working => "working",
        AgentLifecycle::NeedsInput => "needs input",
        AgentLifecycle::ReadyForReview => "awaiting input",
        AgentLifecycle::Completed => "completed",
        AgentLifecycle::Failed => "failed",
        AgentLifecycle::Stopped => "stopped",
        AgentLifecycle::Stale => "stale",
    }
}
