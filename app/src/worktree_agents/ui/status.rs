use pathfinder_color::ColorU;
use warp_core::ui::theme::WarpTheme;

use crate::ui_components::icons::Icon;
use crate::worktree_agents::observation::AgentLifecycle;
use crate::worktree_agents::providers::AgentProvider;

pub(super) fn provider_icon(provider: AgentProvider) -> Icon {
    match provider {
        AgentProvider::Herdr => Icon::Herdr,
        AgentProvider::ClaudeCode | AgentProvider::Codex | AgentProvider::Cli(_) => provider
            .cli_agent()
            .and_then(|agent| agent.icon())
            .unwrap_or(Icon::Cognition),
        AgentProvider::Unknown => Icon::Cognition,
    }
}

pub(super) fn current_pane_color(theme: &WarpTheme) -> ColorU {
    theme.ansi_fg_green()
}

pub(super) fn status_color(lifecycle: AgentLifecycle, theme: &WarpTheme) -> ColorU {
    match lifecycle {
        AgentLifecycle::Detected => theme.sub_text_color(theme.background()).into_solid(),
        AgentLifecycle::Starting | AgentLifecycle::Thinking | AgentLifecycle::Working => {
            theme.ansi_fg_cyan()
        }
        AgentLifecycle::NeedsInput => theme.ansi_fg_yellow(),
        AgentLifecycle::ReadyForReview | AgentLifecycle::Completed => theme.ansi_fg_green(),
        AgentLifecycle::Failed => theme.ui_error_color(),
        AgentLifecycle::Stopped | AgentLifecycle::Stale => {
            theme.sub_text_color(theme.background()).into_solid()
        }
    }
}

pub(super) fn lifecycle_priority(lifecycle: AgentLifecycle) -> u8 {
    match lifecycle {
        AgentLifecycle::NeedsInput | AgentLifecycle::Failed => 0,
        AgentLifecycle::ReadyForReview => 1,
        AgentLifecycle::Starting | AgentLifecycle::Thinking | AgentLifecycle::Working => 2,
        AgentLifecycle::Detected => 3,
        AgentLifecycle::Completed => 4,
        AgentLifecycle::Stopped => 5,
        AgentLifecycle::Stale => 6,
    }
}
