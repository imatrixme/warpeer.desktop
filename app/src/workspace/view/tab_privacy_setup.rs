use settings::Setting as _;
use warp_errors::report_if_error;
use warpui::{SingletonEntity as _, ViewContext};

use super::Workspace;
use crate::tab_privacy::{
    BiometricAuthenticationResult, TabPrivacyCredentialModel, TabPrivacyPromptPurpose,
    TabPrivacyRuntime, TabPrivacySettings, TabPrivacySetupCompletion, authenticate_biometrically,
};

impl Workspace {
    pub(super) fn continue_initial_privacy_setup(
        &mut self,
        completion: TabPrivacySetupCompletion,
        ctx: &mut ViewContext<Self>,
    ) {
        if crate::tab_privacy::biometrics_available() {
            self.show_tab_privacy_prompt(
                TabPrivacyPromptPurpose::OfferBiometrics(completion),
                false,
                ctx,
            );
        } else {
            self.show_recovery_setup(Some(completion), ctx);
        }
    }

    pub(super) fn complete_biometric_setup(
        &mut self,
        completion: TabPrivacySetupCompletion,
        enable: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        if !enable {
            self.show_recovery_setup(Some(completion), ctx);
            return;
        }
        ctx.spawn(
            authenticate_biometrically(),
            move |workspace, result, ctx| match result {
                BiometricAuthenticationResult::Success => {
                    TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
                        report_if_error!(settings.biometric_quick_unlock.set_value(true, ctx));
                    });
                    workspace.show_recovery_setup(Some(completion), ctx);
                }
                BiometricAuthenticationResult::Cancelled => {
                    workspace.show_privacy_error("Touch ID setup was cancelled", ctx)
                }
                BiometricAuthenticationResult::Unavailable => {
                    workspace.show_privacy_error("Touch ID is unavailable", ctx)
                }
                BiometricAuthenticationResult::Failed(error) => {
                    workspace.show_privacy_error(error, ctx)
                }
            },
        );
    }

    pub(in crate::workspace::view) fn show_recovery_setup(
        &mut self,
        completion: Option<TabPrivacySetupCompletion>,
        ctx: &mut ViewContext<Self>,
    ) {
        self.tab_privacy_overlay.update(ctx, |overlay, ctx| {
            overlay.show_recovery_setup(completion, ctx)
        });
        ctx.focus(&self.tab_privacy_overlay);
        ctx.notify();
    }

    pub(super) fn complete_recovery_setup(
        &mut self,
        completion: Option<TabPrivacySetupCompletion>,
        saved: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        match completion {
            Some(completion) => self.complete_initial_privacy_setup(completion, ctx),
            None => self.finish_privacy_prompt(
                if saved {
                    "Recovery question saved"
                } else {
                    "Recovery question unchanged"
                },
                ctx,
            ),
        }
    }

    fn complete_initial_privacy_setup(
        &mut self,
        completion: TabPrivacySetupCompletion,
        ctx: &mut ViewContext<Self>,
    ) {
        match completion {
            TabPrivacySetupCompletion::PasswordOnly => {
                self.finish_privacy_prompt("Privacy password created", ctx)
            }
            TabPrivacySetupCompletion::ProtectTab(tab_id) => {
                let Some(index) = self
                    .tabs
                    .iter()
                    .position(|tab| tab.pane_group.id() == tab_id)
                else {
                    self.finish_privacy_prompt("Privacy password created", ctx);
                    return;
                };
                self.set_tab_protected(index, true, ctx);
                self.remember_tab_focus(index, ctx);
                TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                    runtime.protect(tab_id, ctx);
                    runtime.lock(tab_id, ctx);
                });
                ctx.dispatch_global_action("workspace:save_app", ());
                self.show_tab_privacy_prompt(
                    TabPrivacyPromptPurpose::UnlockTab(tab_id),
                    false,
                    ctx,
                );
            }
        }
    }

    pub(in crate::workspace::view) fn show_tab_privacy_recovery_challenge(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(question) = TabPrivacyCredentialModel::as_ref(ctx)
            .recovery_question()
            .map(str::to_string)
        else {
            self.show_privacy_error("Password recovery is not configured", ctx);
            return;
        };
        self.tab_privacy_overlay.update(ctx, |overlay, ctx| {
            overlay.show_recovery_challenge(question, ctx)
        });
        ctx.focus(&self.tab_privacy_overlay);
        ctx.notify();
    }
}
