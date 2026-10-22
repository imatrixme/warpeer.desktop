use settings::Setting as _;
use warp_core::ui::theme::Fill;
use warpui::{AppContext, SingletonEntity as _};

use crate::context_chips::runtime_sessions::RuntimeKind;
use crate::terminal::model::session::Sessions;
use crate::terminal::model::terminal_model::TerminalModel;
use crate::window_settings::WindowSettings;

const TMUX_SURFACE_OPACITY_FACTOR: f32 = 0.8;

pub(crate) fn active_runtime(model: &TerminalModel, sessions: &Sessions) -> Option<RuntimeKind> {
    let active_block = model.block_list().active_block();
    if !active_block.is_active_and_long_running() {
        return None;
    }
    active_block
        .top_level_command(sessions)
        .and_then(|command| RuntimeKind::from_executable(&command))
}

pub(crate) fn tmux_surface_background(app: &AppContext) -> Fill {
    let opacity =
        scaled_tmux_surface_opacity(*WindowSettings::as_ref(app).background_opacity.value());
    crate::appearance::Appearance::as_ref(app)
        .theme()
        .background()
        .with_opacity(opacity)
}

fn scaled_tmux_surface_opacity(global_opacity: u8) -> u8 {
    (global_opacity as f32 * TMUX_SURFACE_OPACITY_FACTOR).round() as u8
}

#[cfg(test)]
#[path = "runtime_style_tests.rs"]
mod tests;
