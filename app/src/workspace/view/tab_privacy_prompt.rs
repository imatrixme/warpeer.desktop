use std::time::Duration;

use settings::Setting as _;
use warp_errors::report_if_error;
use warpui::r#async::Timer;
use warpui::{EntityId, SingletonEntity as _, ViewContext};

use super::Workspace;
use crate::tab_privacy::{
    AuthenticationScope, BiometricAuthenticationResult, TAB_PRIVACY_REVEAL_DURATION,
    TabPrivacyCredentialModel, TabPrivacyOverlayEvent, TabPrivacyPromptPurpose, TabPrivacyRuntime,
    TabPrivacySettings, TabPrivacySetupCompletion, authenticate_biometrically,
};

#[path = "tab_privacy_setup.rs"]
mod setup;

impl Workspace {
    pub(super) fn handle_tab_privacy_overlay_event(
        &mut self,
        event: &TabPrivacyOverlayEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        match event {
            TabPrivacyOverlayEvent::Submit {
                purpose,
                password,
                new_password,
                confirmation,
                recovery_question,
                recovery_answer,
            } => self.submit_tab_privacy_prompt(
                *purpose,
                password,
                new_password.as_deref(),
                confirmation.as_deref(),
                recovery_question.as_deref(),
                recovery_answer.as_deref(),
                ctx,
            ),
            TabPrivacyOverlayEvent::Biometric(purpose) => {
                self.authenticate_tab_privacy_biometrically(*purpose, ctx)
            }
            TabPrivacyOverlayEvent::BiometricSetup { completion, enable } => {
                self.complete_biometric_setup(*completion, *enable, ctx)
            }
            TabPrivacyOverlayEvent::SkipRecovery(completion) => {
                self.complete_recovery_setup(*completion, false, ctx)
            }
            TabPrivacyOverlayEvent::ForgotPassword => self.show_tab_privacy_recovery_challenge(ctx),
            TabPrivacyOverlayEvent::PasswordVerificationCompleted(purpose) => {
                self.finish_verified_privacy_purpose(*purpose, ctx)
            }
            TabPrivacyOverlayEvent::Cancel(purpose) => {
                self.cancel_tab_privacy_authentication(*purpose, ctx);
                self.sync_active_tab_privacy_overlay(ctx);
                self.focus_active_tab(ctx);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_tab_privacy_prompt(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        password: &str,
        new_password: Option<&str>,
        confirmation: Option<&str>,
        recovery_question: Option<&str>,
        recovery_answer: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) {
        if purpose.requires_authentication() {
            self.resolve_tab_privacy_password_verification(purpose, password, ctx);
            return;
        }
        match purpose {
            TabPrivacyPromptPurpose::CreatePassword
            | TabPrivacyPromptPurpose::CreateAndProtect(_) => {
                if password.is_empty() || confirmation != Some(password) {
                    self.show_privacy_error("Passwords do not match", ctx);
                    return;
                }
                let result = TabPrivacyCredentialModel::handle(ctx)
                    .update(ctx, |model, ctx| model.set_password(password, ctx));
                if let Err(error) = result {
                    self.show_privacy_error(error.to_string(), ctx);
                    return;
                }
                let completion = match purpose {
                    TabPrivacyPromptPurpose::CreatePassword => {
                        TabPrivacySetupCompletion::PasswordOnly
                    }
                    TabPrivacyPromptPurpose::CreateAndProtect(tab_id) => {
                        TabPrivacySetupCompletion::ProtectTab(tab_id)
                    }
                    _ => unreachable!(),
                };
                self.continue_initial_privacy_setup(completion, ctx);
            }
            TabPrivacyPromptPurpose::SetNewPassword
            | TabPrivacyPromptPurpose::ResetRecoveredPassword => {
                let reset_from_recovery =
                    matches!(purpose, TabPrivacyPromptPurpose::ResetRecoveredPassword);
                if self.replace_privacy_password(new_password, confirmation, ctx)
                    && reset_from_recovery
                {
                    self.sync_active_tab_privacy_overlay(ctx);
                }
            }
            TabPrivacyPromptPurpose::SetupRecovery(completion) => {
                let Some(question) = recovery_question else {
                    self.show_privacy_error("Choose or enter a recovery question", ctx);
                    return;
                };
                let Some(answer) = recovery_answer else {
                    self.show_privacy_error("Enter a recovery answer", ctx);
                    return;
                };
                let result = TabPrivacyCredentialModel::handle(ctx)
                    .update(ctx, |model, ctx| model.set_recovery(question, answer, ctx));
                if let Err(error) = result {
                    self.show_privacy_error(error.to_string(), ctx);
                    return;
                }
                self.complete_recovery_setup(completion, true, ctx);
            }
            TabPrivacyPromptPurpose::RecoveryChallenge => {
                let success = recovery_answer.is_some_and(|answer| {
                    TabPrivacyCredentialModel::as_ref(ctx)
                        .verify_recovery(answer)
                        .unwrap_or(false)
                });
                if success {
                    self.show_tab_privacy_prompt(
                        TabPrivacyPromptPurpose::ResetRecoveredPassword,
                        true,
                        ctx,
                    );
                } else {
                    self.show_privacy_error("Incorrect recovery answer", ctx);
                }
            }
            TabPrivacyPromptPurpose::OfferBiometrics(_) => {}
            TabPrivacyPromptPurpose::UnlockTab(_)
            | TabPrivacyPromptPurpose::UnlockAll
            | TabPrivacyPromptPurpose::ChangePassword
            | TabPrivacyPromptPurpose::ResetPassword
            | TabPrivacyPromptPurpose::RemoveProtection(_)
            | TabPrivacyPromptPurpose::DisableProtection
            | TabPrivacyPromptPurpose::ConfigureRecovery => unreachable!(),
        }
    }

    fn resolve_tab_privacy_password_verification(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        password: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        let result = TabPrivacyCredentialModel::handle(ctx)
            .update(ctx, |model, ctx| model.verify_and_upgrade(password, ctx));
        let result = match result {
            Ok(true) => Ok(()),
            Ok(false) => Err("Incorrect password. Try again.".to_string()),
            Err(error) => Err(error.to_string()),
        };
        self.tab_privacy_overlay.update(ctx, |overlay, ctx| {
            overlay.resolve_password_verification(purpose, result, ctx);
        });
    }

    fn replace_privacy_password(
        &mut self,
        new_password: Option<&str>,
        confirmation: Option<&str>,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let Some(new_password) = new_password else {
            self.show_privacy_error("Enter a new password", ctx);
            return false;
        };
        if new_password.is_empty() || confirmation != Some(new_password) {
            self.show_privacy_error("New passwords do not match", ctx);
            return false;
        }
        let result = TabPrivacyCredentialModel::handle(ctx)
            .update(ctx, |model, ctx| model.set_password(new_password, ctx));
        if let Err(error) = result {
            self.show_privacy_error(error.to_string(), ctx);
            return false;
        }
        self.finish_privacy_prompt("Privacy password changed", ctx);
        true
    }

    fn finish_verified_tab_privacy_authentication(
        &mut self,
        scope: AuthenticationScope,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(revision) = TabPrivacyRuntime::handle(ctx)
            .update(ctx, |runtime, ctx| runtime.begin_authentication(scope, ctx))
        else {
            return;
        };
        self.finish_tab_privacy_authentication(scope, revision, true, ctx);
    }

    pub(super) fn authenticate_tab_privacy_biometrically(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        ctx: &mut ViewContext<Self>,
    ) {
        let authentication = match purpose {
            TabPrivacyPromptPurpose::UnlockTab(tab_id) => {
                TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                    runtime
                        .begin_authentication(AuthenticationScope::Tab(tab_id), ctx)
                        .map(|revision| (AuthenticationScope::Tab(tab_id), revision))
                })
            }
            TabPrivacyPromptPurpose::UnlockAll => {
                TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                    runtime
                        .begin_authentication(AuthenticationScope::All, ctx)
                        .map(|revision| (AuthenticationScope::All, revision))
                })
            }
            _ if purpose.requires_authentication() => None,
            _ => return,
        };
        ctx.spawn(
            authenticate_biometrically(),
            move |workspace, result, ctx| match result {
                BiometricAuthenticationResult::Success => {
                    if let Some((scope, revision)) = authentication {
                        workspace.finish_tab_privacy_authentication(scope, revision, true, ctx);
                    } else {
                        workspace.finish_verified_privacy_purpose(purpose, ctx);
                    }
                }
                BiometricAuthenticationResult::Cancelled => {
                    if let Some((scope, revision)) = authentication {
                        workspace.finish_tab_privacy_authentication(scope, revision, false, ctx);
                    } else {
                        workspace.tab_privacy_overlay.update(ctx, |overlay, ctx| {
                            overlay.focus_password(ctx);
                        });
                    }
                }
                BiometricAuthenticationResult::Unavailable => workspace.finish_failed_biometric(
                    authentication,
                    "Touch ID is unavailable",
                    ctx,
                ),
                BiometricAuthenticationResult::Failed(error) => {
                    workspace.finish_failed_biometric(authentication, &error, ctx)
                }
            },
        );
    }

    fn finish_failed_biometric(
        &mut self,
        authentication: Option<(AuthenticationScope, u64)>,
        error: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some((scope, revision)) = authentication {
            self.finish_tab_privacy_authentication(scope, revision, false, ctx);
        }
        self.show_privacy_error(error, ctx);
    }

    fn finish_verified_privacy_purpose(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        ctx: &mut ViewContext<Self>,
    ) {
        match purpose {
            TabPrivacyPromptPurpose::UnlockTab(tab_id) => self
                .finish_verified_tab_privacy_authentication(AuthenticationScope::Tab(tab_id), ctx),
            TabPrivacyPromptPurpose::UnlockAll => {
                self.finish_verified_tab_privacy_authentication(AuthenticationScope::All, ctx)
            }
            TabPrivacyPromptPurpose::ChangePassword => {
                self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::SetNewPassword, true, ctx)
            }
            TabPrivacyPromptPurpose::ResetPassword => {
                self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::SetNewPassword, true, ctx)
            }
            TabPrivacyPromptPurpose::RemoveProtection(tab_id) => {
                self.remove_tab_privacy_protection(tab_id, ctx)
            }
            TabPrivacyPromptPurpose::DisableProtection => self.disable_tab_privacy(ctx),
            TabPrivacyPromptPurpose::ConfigureRecovery => self.show_recovery_setup(None, ctx),
            TabPrivacyPromptPurpose::CreatePassword
            | TabPrivacyPromptPurpose::CreateAndProtect(_)
            | TabPrivacyPromptPurpose::SetNewPassword
            | TabPrivacyPromptPurpose::OfferBiometrics(_)
            | TabPrivacyPromptPurpose::SetupRecovery(_)
            | TabPrivacyPromptPurpose::RecoveryChallenge
            | TabPrivacyPromptPurpose::ResetRecoveredPassword => {}
        }
    }

    fn remove_tab_privacy_protection(&mut self, tab_id: EntityId, ctx: &mut ViewContext<Self>) {
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.pane_group.id() == tab_id)
        {
            tab.privacy_protected = false;
        }
        TabPrivacyRuntime::handle(ctx)
            .update(ctx, |runtime, ctx| runtime.remove_protection(tab_id, ctx));
        ctx.dispatch_global_action("workspace:save_app", ());
        self.finish_privacy_prompt("Tab protection removed", ctx);
    }

    fn disable_tab_privacy(&mut self, ctx: &mut ViewContext<Self>) {
        let result =
            TabPrivacyCredentialModel::handle(ctx).update(ctx, |model, ctx| model.clear(ctx));
        if let Err(error) = result {
            self.show_privacy_error(error.to_string(), ctx);
            return;
        }
        self.clear_tab_privacy_from_all_workspaces(ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| runtime.clear(ctx));
        TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
            report_if_error!(settings.biometric_quick_unlock.set_value(false, ctx));
        });
        ctx.dispatch_global_action("workspace:save_app", ());
        self.finish_privacy_prompt("Tab privacy turned off", ctx);
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn submit_tab_privacy_password_for_integration(
        &mut self,
        password: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        self.tab_privacy_overlay.update(ctx, |overlay, ctx| {
            overlay.submit_password_for_integration(password, ctx);
        });
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn complete_biometric_unlock_after_transient_focus_loss_for_integration(
        &mut self,
        ctx: &mut ViewContext<Self>,
    ) {
        let tab_id = self.active_tab_pane_group().id();
        let scope = AuthenticationScope::Tab(tab_id);
        let revision = TabPrivacyRuntime::handle(ctx)
            .update(ctx, |runtime, ctx| runtime.begin_authentication(scope, ctx))
            .expect("protected tab should begin biometric authentication");
        self.handle_tab_privacy_focus_change(true, false, ctx);
        self.finish_tab_privacy_authentication(scope, revision, true, ctx);
    }

    fn finish_tab_privacy_authentication(
        &mut self,
        scope: AuthenticationScope,
        revision: u64,
        success: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let window_state = ctx.windows().state();
        let is_viewed = window_state.active_window == Some(self.window_id)
            && window_state.stage == warpui::windowing::state::ApplicationStage::Active;
        let viewed_tab = is_viewed
            .then(|| {
                self.tabs
                    .get(self.active_tab_index)
                    .filter(|tab| tab.supports_privacy(ctx) && tab.privacy_protected)
                    .map(|tab| tab.pane_group.id())
            })
            .flatten();
        let delay = *TabPrivacySettings::as_ref(ctx).relock_delay;
        let accepted = TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
            runtime.finish_authentication(scope, revision, success, viewed_tab, delay, ctx)
        });
        if !accepted {
            return;
        }
        if !success {
            self.tab_privacy_overlay
                .update(ctx, |overlay, ctx| overlay.focus_password(ctx));
            return;
        }
        self.tab_privacy_overlay
            .update(ctx, |overlay, ctx| overlay.start_reveal(ctx));
        self.schedule_tab_privacy_focus_restore(viewed_tab, ctx);
        ctx.notify();
    }

    pub(super) fn cancel_tab_privacy_authentication(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        ctx: &mut ViewContext<Self>,
    ) {
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| match purpose {
            TabPrivacyPromptPurpose::UnlockTab(tab_id) => runtime.lock(tab_id, ctx),
            TabPrivacyPromptPurpose::UnlockAll => runtime.lock_all(ctx),
            TabPrivacyPromptPurpose::CreatePassword
            | TabPrivacyPromptPurpose::CreateAndProtect(_)
            | TabPrivacyPromptPurpose::ChangePassword
            | TabPrivacyPromptPurpose::ResetPassword
            | TabPrivacyPromptPurpose::SetNewPassword
            | TabPrivacyPromptPurpose::RemoveProtection(_)
            | TabPrivacyPromptPurpose::DisableProtection
            | TabPrivacyPromptPurpose::ConfigureRecovery
            | TabPrivacyPromptPurpose::OfferBiometrics(_)
            | TabPrivacyPromptPurpose::SetupRecovery(_)
            | TabPrivacyPromptPurpose::RecoveryChallenge
            | TabPrivacyPromptPurpose::ResetRecoveredPassword => {}
        });
    }

    fn restore_tab_privacy_focus(&mut self, tab_id: EntityId, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.pane_group.id() == tab_id) else {
            return;
        };
        if let Some(pane_id) = TabPrivacyRuntime::as_ref(ctx).remembered_pane(tab_id) {
            tab.pane_group.update(ctx, |pane_group, ctx| {
                pane_group.reveal_and_focus_pane(pane_id, ctx);
                pane_group.focus(ctx);
            });
        } else {
            self.focus_active_tab(ctx);
        }
    }

    fn schedule_tab_privacy_focus_restore(
        &mut self,
        tab_id: Option<EntityId>,
        ctx: &mut ViewContext<Self>,
    ) {
        ctx.spawn(
            async move {
                Timer::after(TAB_PRIVACY_REVEAL_DURATION + Duration::from_millis(16)).await
            },
            move |workspace, _, ctx| {
                if let Some(tab_id) = tab_id {
                    workspace.restore_tab_privacy_focus(tab_id, ctx);
                } else {
                    workspace.focus_active_tab(ctx);
                }
            },
        );
    }

    fn show_privacy_error(&mut self, error: impl Into<String>, ctx: &mut ViewContext<Self>) {
        self.tab_privacy_overlay
            .update(ctx, |overlay, ctx| overlay.set_error(error, ctx));
    }

    fn finish_privacy_prompt(&mut self, message: &str, ctx: &mut ViewContext<Self>) {
        self.tab_privacy_overlay
            .update(ctx, |overlay, ctx| overlay.hide(ctx));
        self.show_privacy_toast(message, true, ctx);
        self.focus_active_tab(ctx);
        ctx.notify();
    }

    fn clear_tab_privacy_from_all_workspaces(&mut self, ctx: &mut ViewContext<Self>) {
        for tab in &mut self.tabs {
            tab.privacy_protected = false;
        }
        let current_workspace_id = ctx.view_id();
        let workspaces = crate::workspace::WorkspaceRegistry::as_ref(ctx).all_workspaces(ctx);
        for (_, workspace) in workspaces {
            if workspace.id() == current_workspace_id {
                continue;
            }
            workspace.update(ctx, |workspace, ctx| {
                for tab in &mut workspace.tabs {
                    tab.privacy_protected = false;
                }
                workspace
                    .tab_privacy_overlay
                    .update(ctx, |overlay, ctx| overlay.hide(ctx));
                ctx.notify();
            });
        }
    }
}
