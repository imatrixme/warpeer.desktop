use std::sync::OnceLock;
use std::time::Duration;

use instant::Instant;
use warpui::{AppContext, SingletonEntity};

use crate::product_profile;
use crate::settings::{AppEditorSettings, CursorBlink, CursorDisplayType};
use crate::terminal::model::ansi::{CursorShape, CursorStyle};

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);
const CURSOR_BLINK_IDLE_DELAY: Duration = Duration::from_millis(500);
static CURSOR_BLINK_EPOCH: OnceLock<Instant> = OnceLock::new();

#[derive(Debug, Default)]
pub(crate) struct CursorBlinkActivity {
    last_position: Option<(usize, usize)>,
    last_activity_at: Duration,
}

impl CursorBlinkActivity {
    pub(crate) fn mark_active(&mut self, now: Duration) {
        self.last_activity_at = now;
    }

    pub(crate) fn idle_elapsed(
        &mut self,
        position: (usize, usize),
        active: bool,
        now: Duration,
    ) -> Duration {
        if !active || self.last_position != Some(position) {
            self.last_position = Some(position);
            self.mark_active(now);
        }
        now.saturating_sub(self.last_activity_at)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TerminalCursorPresentation {
    pub(crate) style: CursorStyle,
    pub(crate) visible: bool,
    pub(crate) repaint_after: Option<Duration>,
}

pub(crate) fn cursor_blink_restart_delay(terminal_only: bool) -> Duration {
    if terminal_only {
        CURSOR_BLINK_IDLE_DELAY + CURSOR_BLINK_INTERVAL
    } else {
        CURSOR_BLINK_INTERVAL
    }
}

pub(crate) fn cursor_clock_elapsed() -> Duration {
    CURSOR_BLINK_EPOCH.get_or_init(Instant::now).elapsed()
}

pub(crate) fn resolve_cursor_presentation(
    terminal_style: CursorStyle,
    terminal_only: bool,
    display_type: CursorDisplayType,
    cursor_blink: CursorBlink,
    focused: bool,
    elapsed: Duration,
) -> TerminalCursorPresentation {
    if !terminal_only {
        return TerminalCursorPresentation {
            style: terminal_style,
            visible: true,
            repaint_after: None,
        };
    }

    let blinking = cursor_blink == CursorBlink::Enabled && focused;
    let style = CursorStyle {
        shape: match display_type {
            CursorDisplayType::Bar => CursorShape::Beam,
            CursorDisplayType::Block => CursorShape::Block,
            CursorDisplayType::Underline => CursorShape::Underline,
        },
        blinking,
    };
    if !blinking {
        return TerminalCursorPresentation {
            style,
            visible: true,
            repaint_after: None,
        };
    }

    let first_hidden_after = cursor_blink_restart_delay(true);
    if elapsed < first_hidden_after {
        return TerminalCursorPresentation {
            style,
            visible: true,
            repaint_after: Some(first_hidden_after - elapsed),
        };
    }

    let interval_nanos = CURSOR_BLINK_INTERVAL.as_nanos();
    let blinking_nanos = (elapsed - first_hidden_after).as_nanos();
    let phase = blinking_nanos / interval_nanos;
    let remaining_nanos = interval_nanos - blinking_nanos % interval_nanos;
    TerminalCursorPresentation {
        style,
        visible: !phase.is_multiple_of(2),
        repaint_after: Some(Duration::from_nanos(remaining_nanos as u64)),
    }
}

pub(crate) fn cursor_presentation_for_app(
    terminal_style: CursorStyle,
    focused: bool,
    idle_elapsed: Duration,
    app: &AppContext,
) -> TerminalCursorPresentation {
    if !product_profile::is_terminal_only() {
        return resolve_cursor_presentation(
            terminal_style,
            false,
            CursorDisplayType::default(),
            CursorBlink::Disabled,
            focused,
            Duration::ZERO,
        );
    }

    let settings = AppEditorSettings::as_ref(app);
    resolve_cursor_presentation(
        terminal_style,
        true,
        *settings.cursor_display_type,
        *settings.cursor_blink,
        focused,
        idle_elapsed,
    )
}

#[cfg(test)]
#[path = "cursor_presentation_tests.rs"]
mod tests;
