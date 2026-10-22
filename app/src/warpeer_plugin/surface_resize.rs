use std::time::{Duration, Instant};

use warpeer_plugin_protocol::{SurfaceContentSize, SurfaceResizeAnimation, SurfaceResizeEasing};

pub(super) struct SurfaceResizeState {
    from: SurfaceContentSize,
    target: SurfaceContentSize,
    started_at: Instant,
    animation: Option<SurfaceResizeAnimation>,
}

impl SurfaceResizeState {
    pub fn new(size: SurfaceContentSize, now: Instant) -> Self {
        Self {
            from: size,
            target: size,
            started_at: now,
            animation: None,
        }
    }

    pub fn request(
        &mut self,
        target: SurfaceContentSize,
        animation: Option<SurfaceResizeAnimation>,
        reduced_motion: bool,
        now: Instant,
    ) {
        self.from = self.current(now);
        self.target = target;
        self.started_at = now;
        self.animation = (!reduced_motion)
            .then_some(animation)
            .flatten()
            .filter(|animation| animation.duration_ms > 0);
    }

    pub fn current(&self, now: Instant) -> SurfaceContentSize {
        let Some(animation) = self.animation else {
            return self.target;
        };
        let duration = Duration::from_millis(u64::from(animation.duration_ms));
        let progress = if duration.is_zero() {
            1.0
        } else {
            now.saturating_duration_since(self.started_at).as_secs_f32() / duration.as_secs_f32()
        }
        .clamp(0.0, 1.0);
        let progress = eased(progress, animation.easing);
        SurfaceContentSize {
            width: lerp(self.from.width, self.target.width, progress),
            height: lerp(self.from.height, self.target.height, progress),
        }
    }

    pub fn retains_previous_frame(&self) -> bool {
        self.animation.is_some()
    }
}

fn eased(progress: f32, easing: SurfaceResizeEasing) -> f32 {
    match easing {
        SurfaceResizeEasing::Linear => progress,
        SurfaceResizeEasing::EaseOut => 1.0 - (1.0 - progress).powi(3),
        SurfaceResizeEasing::EaseInOut => {
            if progress < 0.5 {
                4.0 * progress.powi(3)
            } else {
                1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
            }
        }
        SurfaceResizeEasing::Spring => {
            (1.0 - (-7.0 * progress).exp() * (9.0 * progress).cos()).clamp(0.0, 1.0)
        }
    }
}

fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_interpolates_and_reduced_motion_finishes_immediately() {
        let now = Instant::now();
        let mut state = SurfaceResizeState::new(
            SurfaceContentSize {
                width: 100.0,
                height: 80.0,
            },
            now,
        );
        let animation = SurfaceResizeAnimation {
            duration_ms: 200,
            easing: SurfaceResizeEasing::Linear,
        };
        state.request(
            SurfaceContentSize {
                width: 200.0,
                height: 120.0,
            },
            Some(animation),
            false,
            now,
        );
        assert_eq!(state.current(now + Duration::from_millis(100)).width, 150.0);
        state.request(
            SurfaceContentSize {
                width: 240.0,
                height: 160.0,
            },
            Some(animation),
            true,
            now,
        );
        assert_eq!(state.current(now).width, 240.0);
    }
}
