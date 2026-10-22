use std::time::{Duration, Instant};

use pathfinder_geometry::vector::Vector2F;

use super::Renderer;
use crate::platform::ios::TerminalCursorMotion;

const MIN_CURSOR_DURATION: Duration = Duration::from_millis(60);
const MAX_CURSOR_DURATION: Duration = Duration::from_millis(1200);

#[derive(Clone, Copy)]
pub(super) struct CursorMotion {
    from: Vector2F,
    to: Vector2F,
    started_at: Instant,
    duration: Duration,
    effect: TerminalCursorMotion,
}

#[derive(Clone, Copy)]
pub(super) struct CursorSample {
    pub position: Vector2F,
    pub scale: Vector2F,
    pub trail: Option<(Vector2F, f32)>,
}

impl CursorMotion {
    fn new(
        from: Vector2F,
        to: Vector2F,
        started_at: Instant,
        duration_ms: f32,
        duration_scale: f32,
        effect: TerminalCursorMotion,
    ) -> Self {
        Self {
            from,
            to,
            started_at,
            duration: Duration::from_secs_f32(
                duration_ms * duration_scale.clamp(0.05, 1.0) / 1_000.0,
            )
            .clamp(MIN_CURSOR_DURATION, MAX_CURSOR_DURATION),
            effect,
        }
    }

    pub(super) fn sample(self, now: Instant) -> CursorSample {
        let progress = (now.duration_since(self.started_at).as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0.0, 1.0);
        let eased = ease_out(progress);
        let spring = terminal_spring(progress);
        let delta = self.to - self.from;
        let mut scale = Vector2F::new(1.0, 1.0);
        let mut trail = None;
        let motion_progress = match self.effect {
            TerminalCursorMotion::Snap => 1.0,
            TerminalCursorMotion::Ease => eased,
            TerminalCursorMotion::Spring => spring,
            TerminalCursorMotion::Smear => {
                scale.set_x(1.0 + delta.x().abs() * (1.0 - eased) * 0.32);
                eased
            }
            TerminalCursorMotion::Squash => {
                let squash = (progress * std::f32::consts::PI).sin() * 0.22;
                scale = Vector2F::new(1.0 + squash, 1.0 - squash * 0.55);
                eased
            }
            TerminalCursorMotion::Phosphor => {
                trail = Some((self.from + delta * (eased * 0.72), (1.0 - progress) * 0.34));
                eased
            }
            TerminalCursorMotion::Arc => eased,
            TerminalCursorMotion::Warp => smoothstep(0.08, 0.92, progress),
            TerminalCursorMotion::Beam => {
                scale.set_x(0.78 + eased * 0.22);
                eased
            }
        };
        let mut position = self.from + delta * motion_progress;
        if matches!(self.effect, TerminalCursorMotion::Arc) {
            position.set_y(position.y() - (progress * std::f32::consts::PI).sin() * 0.42);
        }
        CursorSample {
            position,
            scale,
            trail,
        }
    }

    pub(super) fn is_active(self, now: Instant) -> bool {
        now.duration_since(self.started_at) < self.duration
    }
}

impl Renderer {
    pub(super) fn retarget_cursor(&mut self, target: Option<(u16, u16)>, now: Instant) {
        if !self.settings.effects_enabled
            || matches!(self.settings.cursor_motion, TerminalCursorMotion::Snap)
            || self.settings.reduced_motion
        {
            self.cursor_motion = None;
            return;
        }
        let Some(target) = target.map(grid_position) else {
            self.cursor_motion = None;
            return;
        };
        let Some(previous) = self.terminal_frame.cursor.map(grid_position) else {
            self.cursor_motion = None;
            return;
        };
        let from = self
            .cursor_motion
            .filter(|motion| motion.is_active(now))
            .map(|motion| motion.sample(now).position)
            .unwrap_or(previous);
        if from == target {
            self.cursor_motion = None;
            return;
        }
        self.cursor_motion = Some(CursorMotion::new(
            from,
            target,
            now,
            self.settings.effect_duration_ms,
            self.pending_cursor_duration_scale.take().unwrap_or(1.0),
            self.settings.cursor_motion,
        ));
    }

    pub(super) fn cursor_sample(&self, authoritative: (u16, u16), now: Instant) -> CursorSample {
        self.cursor_motion
            .filter(|motion| motion.is_active(now))
            .map(|motion| motion.sample(now))
            .unwrap_or(CursorSample {
                position: grid_position(authoritative),
                scale: Vector2F::new(1.0, 1.0),
                trail: None,
            })
    }
}

fn grid_position((row, column): (u16, u16)) -> Vector2F {
    Vector2F::new(f32::from(column), f32::from(row))
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

fn terminal_spring(progress: f32) -> f32 {
    1.0 - (-8.0 * progress).exp() * (12.0 * progress).cos()
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let value = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_reaches_the_destination_without_overshoot() {
        let start = Instant::now();
        let motion = CursorMotion::new(
            Vector2F::new(1.0, 0.0),
            Vector2F::new(5.0, 0.0),
            start,
            180.0,
            1.0,
            TerminalCursorMotion::Ease,
        );
        let midpoint = motion.sample(start + Duration::from_millis(90)).position;
        assert!(midpoint.x() > 1.0 && midpoint.x() < 5.0);
        assert_eq!(
            motion
                .sample(start + Duration::from_millis(180))
                .position
                .x(),
            5.0
        );
    }

    #[test]
    fn alternate_screen_does_not_select_a_different_motion_curve() {
        let start = Instant::now();
        let motion = CursorMotion::new(
            Vector2F::zero(),
            Vector2F::new(4.0, 0.0),
            start,
            220.0,
            1.0,
            TerminalCursorMotion::Warp,
        );
        let sample = motion.sample(start + Duration::from_millis(110));
        assert!(sample.position.x() > 0.0 && sample.position.x() < 4.0);
    }

    #[test]
    fn backward_motion_uses_the_desktop_quarter_duration() {
        let start = Instant::now();
        let motion = CursorMotion::new(
            Vector2F::new(5.0, 0.0),
            Vector2F::new(4.0, 0.0),
            start,
            220.0,
            0.25,
            TerminalCursorMotion::Ease,
        );

        assert!(!motion.is_active(start + Duration::from_millis(60)));
        assert_eq!(
            motion
                .sample(start + Duration::from_millis(60))
                .position
                .x(),
            4.0
        );
    }
}
