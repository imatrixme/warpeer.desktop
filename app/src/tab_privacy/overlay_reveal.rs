use std::time::{Duration, Instant};

use warpui::ViewContext;
use warpui::r#async::Timer;

use super::{TAB_PRIVACY_REVEAL_DURATION, TabPrivacyOverlayView};

impl TabPrivacyOverlayView {
    pub fn start_reveal(&mut self, ctx: &mut ViewContext<Self>) {
        self.purpose = None;
        self.error = None;
        self.reset_password_verification();
        self.clear_fields(ctx);
        self.reveal_started_at = Some(Instant::now());
        self.reveal_epoch = self.reveal_epoch.wrapping_add(1);
        self.schedule_reveal_frame(self.reveal_epoch, ctx);
        ctx.notify();
    }

    fn schedule_reveal_frame(&self, epoch: u64, ctx: &mut ViewContext<Self>) {
        ctx.spawn(
            async move { Timer::after(Duration::from_millis(16)).await },
            move |overlay, _, ctx| {
                if overlay.reveal_epoch != epoch || overlay.reveal_started_at.is_none() {
                    return;
                }
                if overlay
                    .reveal_started_at
                    .is_some_and(|started| started.elapsed() >= TAB_PRIVACY_REVEAL_DURATION)
                {
                    overlay.reveal_started_at = None;
                } else {
                    overlay.schedule_reveal_frame(epoch, ctx);
                }
                ctx.notify();
            },
        );
    }
}
