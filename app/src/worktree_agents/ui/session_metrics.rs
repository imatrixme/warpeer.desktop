use warp_core::ui::Icon;
use warp_core::ui::theme::Fill as ThemeFill;
use warpui::Element;
use warpui::elements::{
    ConstrainedBox, Container, CrossAxisAlignment, Empty, Expanded, Flex, MainAxisSize,
    ParentElement, Text,
};
use warpui::text_layout::ClipConfig;

use crate::appearance::Appearance;
use crate::worktree_agents::projections::AgentProjection;

const CONTEXT_ICON_SIZE: f32 = 13.;

pub(super) fn render(
    agent: &AgentProjection,
    now_ms: u64,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let mut details = Vec::new();
    if let Some(model) = agent.metrics.model_name.as_ref() {
        details.push(model.as_str().to_string());
    }
    if let Some(tokens) = agent.metrics.token_usage {
        details.push(format!("{} tokens", format_tokens(tokens)));
    }
    if let Some(activity_at_ms) = agent.metrics.activity_at_ms {
        details.push(format!(
            "Active {}",
            format_age(now_ms.saturating_sub(activity_at_ms))
        ));
    }
    if details.is_empty() {
        details.push("Provider details unavailable".to_string());
    }
    let details = Text::new_inline(details.join(" · "), appearance.ui_font_family(), 9.)
        .with_clip(ClipConfig::ellipsis())
        .with_color(theme.sub_text_color(theme.surface_2()).into())
        .finish();
    Flex::row()
        .with_main_axis_size(MainAxisSize::Max)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(Expanded::new(1., details).finish())
        .with_child(render_context_remaining(
            agent.metrics.context_remaining_percent,
            appearance,
        ))
        .finish()
}

fn render_context_remaining(
    remaining_percent: Option<u8>,
    appearance: &Appearance,
) -> Box<dyn Element> {
    let theme = appearance.theme();
    let Some(remaining) = remaining_percent else {
        return Empty::new().finish();
    };
    let remaining = remaining.min(100);
    let fill = if remaining <= 20 {
        ThemeFill::Solid(theme.ansi_fg_red())
    } else {
        theme.main_text_color(theme.surface_2())
    };
    let icon = context_icon(remaining);
    let label = format!("{remaining}%");
    Flex::row()
        .with_main_axis_size(MainAxisSize::Min)
        .with_cross_axis_alignment(CrossAxisAlignment::Center)
        .with_child(
            ConstrainedBox::new(icon.to_warpui_icon(fill).finish())
                .with_width(CONTEXT_ICON_SIZE)
                .with_height(CONTEXT_ICON_SIZE)
                .finish(),
        )
        .with_child(
            Container::new(
                Text::new_inline(label, appearance.ui_font_family(), 9.)
                    .with_color(theme.sub_text_color(theme.surface_2()).into())
                    .finish(),
            )
            .with_margin_left(3.)
            .finish(),
        )
        .finish()
}

fn context_icon(remaining: u8) -> Icon {
    match remaining {
        95..=100 => Icon::ContextRemaining100,
        85..=94 => Icon::ContextRemaining90,
        75..=84 => Icon::ContextRemaining80,
        65..=74 => Icon::ContextRemaining70,
        55..=64 => Icon::ContextRemaining60,
        45..=54 => Icon::ContextRemaining50,
        35..=44 => Icon::ContextRemaining40,
        25..=34 => Icon::ContextRemaining30,
        15..=24 => Icon::ContextRemaining20,
        5..=14 => Icon::ContextRemaining10,
        0..=4 => Icon::ContextRemaining0,
        101..=u8::MAX => Icon::ContextRemaining100,
    }
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.)
    } else {
        tokens.to_string()
    }
}

fn format_age(age_ms: u64) -> String {
    let seconds = age_ms / 1_000;
    if seconds < 5 {
        "now".to_string()
    } else if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else {
        format!("{}h ago", seconds / 3_600)
    }
}

#[cfg(test)]
#[path = "session_metrics_tests.rs"]
mod tests;
