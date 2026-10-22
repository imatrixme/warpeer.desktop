use std::time::{Duration, Instant};

use warpui::ViewContext;
use warpui::r#async::Timer;

use super::{TabPrivacyOverlayEvent, TabPrivacyOverlayView, TabPrivacyPromptPurpose};

pub(super) const VERIFICATION_FORWARD_DURATION: Duration = Duration::from_millis(680);
pub(super) const VERIFICATION_RETURN_DURATION: Duration = Duration::from_millis(420);

#[derive(Clone, Debug, Eq, PartialEq)]
enum VerificationPhase {
    Forward,
    Returning { error: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum VerificationAdvance {
    Continue,
    Succeeded(TabPrivacyPromptPurpose),
    Failed(String),
}

#[derive(Clone, Debug)]
pub(super) struct PasswordVerificationAnimation {
    purpose: TabPrivacyPromptPurpose,
    phase: VerificationPhase,
    started_at: Instant,
    outcome: Option<Result<(), String>>,
}

impl PasswordVerificationAnimation {
    pub(super) fn new(purpose: TabPrivacyPromptPurpose, started_at: Instant) -> Self {
        Self {
            purpose,
            phase: VerificationPhase::Forward,
            started_at,
            outcome: None,
        }
    }

    pub(super) fn purpose(&self) -> TabPrivacyPromptPurpose {
        self.purpose
    }

    pub(super) fn resolve(&mut self, result: Result<(), String>) {
        if matches!(self.phase, VerificationPhase::Forward) && self.outcome.is_none() {
            self.outcome = Some(result);
        }
    }

    pub(super) fn progress_at(&self, now: Instant) -> f32 {
        match self.phase {
            VerificationPhase::Forward => {
                let progress =
                    elapsed_progress(self.started_at, now, VERIFICATION_FORWARD_DURATION);
                1. - (1. - progress).powi(3)
            }
            VerificationPhase::Returning { .. } => {
                let progress = elapsed_progress(self.started_at, now, VERIFICATION_RETURN_DURATION);
                1. - smoothstep(progress)
            }
        }
    }

    pub(super) fn advance(&mut self, now: Instant) -> VerificationAdvance {
        match &self.phase {
            VerificationPhase::Forward
                if now.duration_since(self.started_at) >= VERIFICATION_FORWARD_DURATION =>
            {
                match self.outcome.take() {
                    Some(Ok(())) => VerificationAdvance::Succeeded(self.purpose),
                    Some(Err(error)) => {
                        self.phase = VerificationPhase::Returning { error };
                        self.started_at = now;
                        VerificationAdvance::Continue
                    }
                    None => VerificationAdvance::Continue,
                }
            }
            VerificationPhase::Returning { error }
                if now.duration_since(self.started_at) >= VERIFICATION_RETURN_DURATION =>
            {
                VerificationAdvance::Failed(error.clone())
            }
            VerificationPhase::Forward | VerificationPhase::Returning { .. } => {
                VerificationAdvance::Continue
            }
        }
    }
}

impl TabPrivacyOverlayView {
    pub(super) fn start_password_verification(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        if self.password_verification.is_some() || self.purpose != Some(purpose) {
            return false;
        }
        self.error = None;
        self.password_verification =
            Some(PasswordVerificationAnimation::new(purpose, Instant::now()));
        self.verification_epoch = self.verification_epoch.wrapping_add(1);
        ctx.focus_self();
        self.schedule_verification_frame(self.verification_epoch, ctx);
        ctx.notify();
        true
    }

    pub fn resolve_password_verification(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        result: Result<(), String>,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(animation) = self.password_verification.as_mut() else {
            return;
        };
        if animation.purpose() != purpose {
            return;
        }
        animation.resolve(result);
        ctx.notify();
    }

    pub(super) fn password_verification_progress(&self) -> Option<f32> {
        self.password_verification
            .as_ref()
            .map(|animation| animation.progress_at(Instant::now()))
    }

    pub(super) fn is_verifying_password(&self) -> bool {
        self.password_verification.is_some()
    }

    pub(super) fn reset_password_verification(&mut self) {
        self.password_verification = None;
        self.verification_epoch = self.verification_epoch.wrapping_add(1);
    }

    fn schedule_verification_frame(&self, epoch: u64, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move { Timer::after(Duration::from_millis(16)).await },
            move |overlay, _, ctx| {
                if overlay.verification_epoch != epoch {
                    return;
                }
                let advance = overlay
                    .password_verification
                    .as_mut()
                    .map(|animation| animation.advance(Instant::now()));
                match advance {
                    Some(VerificationAdvance::Succeeded(purpose)) => {
                        overlay.password_verification = None;
                        ctx.emit(TabPrivacyOverlayEvent::PasswordVerificationCompleted(
                            purpose,
                        ));
                    }
                    Some(VerificationAdvance::Failed(error)) => {
                        overlay.password_verification = None;
                        overlay.error = Some(error);
                        overlay.clear_active_secret(ctx);
                        overlay.focus_first_field(ctx);
                    }
                    Some(VerificationAdvance::Continue) => {
                        overlay.schedule_verification_frame(epoch, ctx);
                    }
                    None => return,
                }
                ctx.notify();
            },
        );
    }
}

fn elapsed_progress(started_at: Instant, now: Instant, duration: Duration) -> f32 {
    (now.duration_since(started_at).as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.)
}

fn smoothstep(progress: f32) -> f32 {
    progress * progress * (3. - 2. * progress)
}
