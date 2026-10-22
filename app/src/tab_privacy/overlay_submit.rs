use warpui::ViewContext;

use super::{
    RecoveryQuestionChoice, TabPrivacyOverlayEvent, TabPrivacyOverlayView, TabPrivacyPromptPurpose,
};

impl TabPrivacyOverlayView {
    pub(super) fn submit(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(purpose) = self.purpose else {
            return;
        };
        if self.is_verifying_password() {
            return;
        }
        let password = self.password_editor.as_ref(ctx).buffer_text(ctx);
        if purpose.requires_authentication() && password.is_empty() {
            self.set_error("Enter your password", ctx);
            return;
        }
        let new_password = purpose
            .needs_new_password()
            .then(|| self.new_password_editor.as_ref(ctx).buffer_text(ctx));
        let confirmation = purpose
            .needs_confirmation()
            .then(|| self.confirmation_editor.as_ref(ctx).buffer_text(ctx));
        let recovery_question =
            matches!(purpose, TabPrivacyPromptPurpose::SetupRecovery(_)).then(|| {
                if self.recovery_choice == RecoveryQuestionChoice::Custom {
                    self.recovery_question_editor.as_ref(ctx).buffer_text(ctx)
                } else {
                    self.recovery_choice.label().to_string()
                }
            });
        let recovery_answer = matches!(
            purpose,
            TabPrivacyPromptPurpose::SetupRecovery(_) | TabPrivacyPromptPurpose::RecoveryChallenge
        )
        .then(|| self.recovery_answer_editor.as_ref(ctx).buffer_text(ctx));
        if purpose.requires_authentication() && !self.start_password_verification(purpose, ctx) {
            return;
        }
        ctx.emit(TabPrivacyOverlayEvent::Submit {
            purpose,
            password,
            new_password,
            confirmation,
            recovery_question,
            recovery_answer,
        });
    }

    pub(super) fn cancel(&mut self, ctx: &mut ViewContext<Self>) {
        let Some(purpose) = self.purpose else {
            return;
        };
        self.hide(ctx);
        ctx.emit(TabPrivacyOverlayEvent::Cancel(purpose));
    }

    #[cfg(feature = "integration_tests")]
    pub fn submit_password_for_integration(&mut self, password: &str, ctx: &mut ViewContext<Self>) {
        self.password_editor
            .update(ctx, |editor, ctx| editor.set_buffer_text(password, ctx));
        self.submit(ctx);
    }
}
