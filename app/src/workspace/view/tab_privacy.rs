use settings::Setting as _;
use warp_errors::report_if_error;
use warpui::elements::{ChildView, Element};
use warpui::{AppContext, EntityId, SingletonEntity as _, ViewContext};

use super::Workspace;
use crate::system::SystemStatsEvent;
use crate::tab_privacy::{
    TabPrivacyCredentialModel, TabPrivacyPromptPurpose, TabPrivacyRuntime, TabPrivacyRuntimeEvent,
    TabPrivacySettings, TabPrivacyState, biometrics_available,
};
use crate::view_components::DismissibleToast;
use crate::workspace::header_toolbar_item::HeaderToolbarItemKind;
use crate::workspace::tab_settings::{HeaderToolbarChipSelection, TabSettings};

#[path = "tab_privacy_button.rs"]
mod button;

impl Workspace {
    pub(super) fn active_tab_privacy_locked(&self, ctx: &AppContext) -> bool {
        self.tabs
            .get(self.active_tab_index)
            .is_some_and(|tab| tab.privacy_locked(ctx))
    }

    pub(super) fn handle_tab_privacy_runtime_event(
        &mut self,
        event: &TabPrivacyRuntimeEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        let TabPrivacyRuntimeEvent::Changed(changed) = event;
        let active_tab_id = self.active_tab_pane_group().id();
        if changed.contains(&active_tab_id) {
            self.sync_active_tab_privacy_overlay(ctx);
        }
        ctx.notify();
    }

    pub(super) fn handle_tab_privacy_system_event(
        &mut self,
        event: &SystemStatsEvent,
        ctx: &mut ViewContext<Self>,
    ) {
        if !matches!(event, SystemStatsEvent::CpuWillSleep)
            || !*TabPrivacySettings::as_ref(ctx).lock_on_system_inactivity
        {
            return;
        }
        self.remember_tab_focus(self.active_tab_index, ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| runtime.lock_all(ctx));
        self.sync_active_tab_privacy_overlay(ctx);
    }

    pub(super) fn ensure_tab_privacy_toolbar_item(&mut self, ctx: &mut ViewContext<Self>) {
        let presence_migrated = *TabPrivacySettings::as_ref(ctx).toolbar_item_migrated;
        let rightmost_migrated = *TabPrivacySettings::as_ref(ctx).toolbar_item_rightmost_migrated;
        if presence_migrated && rightmost_migrated {
            return;
        }

        let config = TabSettings::as_ref(ctx)
            .header_toolbar_chip_selection
            .clone();
        let current_left = config.left_items();
        let current_right = config.right_items();
        let mut left = current_left.clone();
        let mut right = current_right.clone();
        if !rightmost_migrated {
            left.retain(|item| *item != HeaderToolbarItemKind::TabPrivacy);
            right.retain(|item| *item != HeaderToolbarItemKind::TabPrivacy);
            right.push(HeaderToolbarItemKind::TabPrivacy);
        } else if !left.contains(&HeaderToolbarItemKind::TabPrivacy)
            && !right.contains(&HeaderToolbarItemKind::TabPrivacy)
        {
            right.push(HeaderToolbarItemKind::TabPrivacy);
        }
        if left != current_left || right != current_right {
            TabSettings::handle(ctx).update(ctx, |settings, ctx| {
                report_if_error!(
                    settings
                        .header_toolbar_chip_selection
                        .set_value(HeaderToolbarChipSelection::Custom { left, right }, ctx,)
                );
            });
        }
        TabPrivacySettings::handle(ctx).update(ctx, |settings, ctx| {
            if !presence_migrated {
                report_if_error!(settings.toolbar_item_migrated.set_value(true, ctx));
            }
            if !rightmost_migrated {
                report_if_error!(
                    settings
                        .toolbar_item_rightmost_migrated
                        .set_value(true, ctx)
                );
            }
        });
    }

    pub(super) fn show_tab_privacy_prompt(
        &mut self,
        purpose: TabPrivacyPromptPurpose,
        cancellable: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let biometric_available =
            *TabPrivacySettings::as_ref(ctx).biometric_quick_unlock && biometrics_available();
        let recovery_available = TabPrivacyCredentialModel::as_ref(ctx).has_recovery();
        self.tab_privacy_overlay.update(ctx, |overlay, ctx| {
            overlay.show(
                purpose,
                cancellable,
                biometric_available,
                recovery_available,
                ctx,
            );
        });
        ctx.focus(&self.tab_privacy_overlay);
        ctx.notify();
    }

    pub(super) fn sync_active_tab_privacy_overlay(&mut self, ctx: &mut ViewContext<Self>) {
        let tab_id = self.active_tab_pane_group().id();
        let locked = self.active_tab_privacy_locked(ctx);
        let current_purpose = self.tab_privacy_overlay.as_ref(ctx).purpose();
        let preserves_explicit_prompt = current_purpose
            .is_some_and(|purpose| !matches!(purpose, TabPrivacyPromptPurpose::UnlockTab(_)));
        if preserves_explicit_prompt {
            return;
        }
        if locked {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockTab(tab_id), false, ctx);
        } else if self.tab_privacy_overlay.as_ref(ctx).is_visible() {
            self.tab_privacy_overlay
                .update(ctx, |overlay, ctx| overlay.hide(ctx));
        }
    }

    pub(super) fn transition_tab_privacy_for_activation(
        &mut self,
        previous_index: usize,
        next_index: usize,
        ctx: &mut ViewContext<Self>,
    ) -> bool {
        let delay = *TabPrivacySettings::as_ref(ctx).relock_delay;
        let previous_id = (previous_index != next_index)
            .then(|| self.tabs.get(previous_index))
            .flatten()
            .filter(|tab| tab.supports_privacy(ctx) && tab.privacy_protected)
            .map(|tab| tab.pane_group.id());
        if let Some(previous_id) = previous_id {
            if let Some(purpose) = self.tab_privacy_overlay.as_ref(ctx).purpose() {
                self.cancel_tab_privacy_authentication(purpose, ctx);
            }
            TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                runtime.mark_away(previous_id, delay, ctx);
            });
        }

        let Some(next) = self.tabs.get(next_index) else {
            return true;
        };
        if !next.supports_privacy(ctx) || !next.privacy_protected {
            self.tab_privacy_overlay
                .update(ctx, |overlay, ctx| overlay.hide(ctx));
            return true;
        }
        let next_id = next.pane_group.id();
        if TabPrivacyRuntime::as_ref(ctx).is_locked(next_id) {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockTab(next_id), false, ctx);
            return false;
        }
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
            runtime.mark_viewed(next_id, ctx);
        });
        self.tab_privacy_overlay
            .update(ctx, |overlay, ctx| overlay.hide(ctx));
        true
    }

    pub(super) fn handle_tab_privacy_focus_change(
        &mut self,
        was_viewed: bool,
        is_viewed: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        let Some(active) = self.tabs.get(self.active_tab_index) else {
            return;
        };
        if !active.supports_privacy(ctx) || !active.privacy_protected || was_viewed == is_viewed {
            return;
        }
        let tab_id = active.pane_group.id();
        if matches!(
            TabPrivacyRuntime::as_ref(ctx).state(tab_id),
            Some(TabPrivacyState::Authenticating { .. })
        ) {
            return;
        }
        if is_viewed {
            if TabPrivacyRuntime::as_ref(ctx).is_locked(tab_id) {
                self.show_tab_privacy_prompt(
                    TabPrivacyPromptPurpose::UnlockTab(tab_id),
                    false,
                    ctx,
                );
            } else {
                TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                    runtime.mark_viewed(tab_id, ctx);
                });
            }
        } else {
            if let Some(purpose) = self.tab_privacy_overlay.as_ref(ctx).purpose() {
                self.cancel_tab_privacy_authentication(purpose, ctx);
            }
            let delay = *TabPrivacySettings::as_ref(ctx).relock_delay;
            TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                runtime.mark_away(tab_id, delay, ctx);
            });
        }
    }

    pub(super) fn set_tab_protected(
        &mut self,
        index: usize,
        protected: bool,
        ctx: &AppContext,
    ) -> Option<EntityId> {
        let tab = self.tabs.get_mut(index)?;
        if protected && !tab.supports_privacy(ctx) {
            return None;
        }
        tab.privacy_protected = protected;
        Some(tab.pane_group.id())
    }

    pub(super) fn remember_tab_focus(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self.tabs.get(index) else {
            return;
        };
        let tab_id = tab.pane_group.id();
        let pane_id = tab.pane_group.as_ref(ctx).focused_pane_id(ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, _| {
            runtime.remember_pane(tab_id, pane_id);
        });
    }

    pub(super) fn protect_and_lock_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self.tabs.get(index).filter(|tab| tab.supports_privacy(ctx)) else {
            return;
        };
        let tab_id = tab.pane_group.id();
        if !TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
            self.show_tab_privacy_prompt(
                TabPrivacyPromptPurpose::CreateAndProtect(tab_id),
                true,
                ctx,
            );
            return;
        }
        self.set_tab_protected(index, true, ctx);
        self.remember_tab_focus(index, ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
            runtime.protect(tab_id, ctx);
            runtime.lock(tab_id, ctx);
        });
        ctx.dispatch_global_action("workspace:save_app", ());
        if index == self.active_tab_index {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockTab(tab_id), false, ctx);
        }
        ctx.notify();
    }

    pub(super) fn lock_tab_privacy(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        let Some(tab) = self
            .tabs
            .get(index)
            .filter(|tab| tab.supports_privacy(ctx) && tab.privacy_protected)
        else {
            return;
        };
        let tab_id = tab.pane_group.id();
        self.remember_tab_focus(index, ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| runtime.lock(tab_id, ctx));
        if index == self.active_tab_index {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockTab(tab_id), false, ctx);
        }
    }

    pub(super) fn request_unlock_tab(&mut self, index: usize, ctx: &mut ViewContext<Self>) {
        if let Some(tab_id) = self
            .tabs
            .get(index)
            .filter(|tab| tab.supports_privacy(ctx) && tab.privacy_protected)
            .map(|tab| tab.pane_group.id())
        {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockTab(tab_id), false, ctx);
        }
    }

    pub(super) fn request_remove_tab_protection(
        &mut self,
        index: usize,
        ctx: &mut ViewContext<Self>,
    ) {
        if let Some(tab_id) = self
            .tabs
            .get(index)
            .filter(|tab| tab.supports_privacy(ctx) && tab.privacy_protected)
            .map(|tab| tab.pane_group.id())
        {
            self.show_tab_privacy_prompt(
                TabPrivacyPromptPurpose::RemoveProtection(tab_id),
                true,
                ctx,
            );
        }
    }

    pub(super) fn toggle_active_tab_privacy(&mut self, ctx: &mut ViewContext<Self>) {
        let index = self.active_tab_index;
        if !self
            .tabs
            .get(index)
            .is_some_and(|tab| tab.supports_privacy(ctx))
        {
            return;
        }
        let tab_id = self.active_tab_pane_group().id();
        match TabPrivacyRuntime::as_ref(ctx).state(tab_id) {
            None => self.protect_and_lock_tab(index, ctx),
            Some(TabPrivacyState::Locked) => {
                let purpose = TabPrivacyPromptPurpose::UnlockTab(tab_id);
                self.show_tab_privacy_prompt(purpose, false, ctx);
                if *TabPrivacySettings::as_ref(ctx).biometric_quick_unlock && biometrics_available()
                {
                    self.authenticate_tab_privacy_biometrically(purpose, ctx);
                }
            }
            Some(TabPrivacyState::Authenticating { .. }) => {}
            Some(TabPrivacyState::UnlockedViewed | TabPrivacyState::UnlockedAway { .. }) => {
                self.lock_tab_privacy(index, ctx)
            }
        }
    }

    pub(super) fn toggle_all_tab_privacy(&mut self, ctx: &mut ViewContext<Self>) {
        let runtime = TabPrivacyRuntime::as_ref(ctx);
        if runtime.protected_count() == 0 {
            self.show_privacy_toast("No protected tabs", false, ctx);
            return;
        }
        if runtime.all_protected_locked() {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::UnlockAll, true, ctx);
            return;
        }
        self.lock_all_tab_privacy(ctx);
    }

    pub(super) fn lock_all_tab_privacy(&mut self, ctx: &mut ViewContext<Self>) {
        if TabPrivacyRuntime::as_ref(ctx).protected_count() == 0 {
            self.show_privacy_toast("No protected tabs", false, ctx);
            return;
        }
        self.remember_tab_focus(self.active_tab_index, ctx);
        TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| runtime.lock_all(ctx));
        self.sync_active_tab_privacy_overlay(ctx);
    }

    pub(super) fn request_change_tab_privacy_password(&mut self, ctx: &mut ViewContext<Self>) {
        let purpose = if TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
            TabPrivacyPromptPurpose::ChangePassword
        } else {
            TabPrivacyPromptPurpose::CreatePassword
        };
        self.show_tab_privacy_prompt(purpose, true, ctx);
    }

    pub(super) fn request_configure_tab_privacy_recovery(&mut self, ctx: &mut ViewContext<Self>) {
        let credentials = TabPrivacyCredentialModel::as_ref(ctx);
        if !credentials.has_credential() {
            self.show_privacy_toast("Create a privacy password first", false, ctx);
        } else if credentials.has_recovery() {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::ConfigureRecovery, true, ctx);
        } else {
            self.show_recovery_setup(None, ctx);
        }
    }

    pub(super) fn request_recover_tab_privacy_password(&mut self, ctx: &mut ViewContext<Self>) {
        let credentials = TabPrivacyCredentialModel::as_ref(ctx);
        if !credentials.has_credential() {
            self.show_privacy_toast("Create a privacy password first", false, ctx);
        } else if credentials.has_recovery() {
            self.show_tab_privacy_recovery_challenge(ctx);
        } else {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::ResetPassword, true, ctx);
        }
    }

    pub(super) fn request_disable_tab_privacy(&mut self, ctx: &mut ViewContext<Self>) {
        if TabPrivacyCredentialModel::as_ref(ctx).has_credential() {
            self.show_tab_privacy_prompt(TabPrivacyPromptPurpose::DisableProtection, true, ctx);
        }
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn protect_active_tab_for_privacy_integration(
        &mut self,
        password: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        TabPrivacyCredentialModel::handle(ctx).update(ctx, |model, ctx| {
            model.set_password_in_memory_for_integration(password, ctx);
        });
        self.protect_and_lock_tab(self.active_tab_index, ctx);
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn active_tab_privacy_state_for_integration(
        &self,
        ctx: &AppContext,
    ) -> (bool, bool, bool, String) {
        let tab = &self.tabs[self.active_tab_index];
        let tab_id = tab.pane_group.id();
        (
            tab.privacy_protected,
            TabPrivacyRuntime::as_ref(ctx).is_locked(tab_id),
            self.tab_privacy_overlay.as_ref(ctx).is_visible(),
            tab.display_title(ctx),
        )
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn tab_privacy_editor_focus_for_integration(
        &self,
        ctx: &AppContext,
    ) -> (bool, bool) {
        self.tab_privacy_overlay
            .as_ref(ctx)
            .editor_focus_for_integration(ctx)
    }

    pub(super) fn render_tab_privacy_overlay(&self) -> Box<dyn Element> {
        ChildView::new(&self.tab_privacy_overlay).finish()
    }

    pub(super) fn show_privacy_toast(
        &mut self,
        message: &str,
        success: bool,
        ctx: &mut ViewContext<Self>,
    ) {
        self.toast_stack.update(ctx, |stack, ctx| {
            let toast = if success {
                DismissibleToast::success(message.to_string())
            } else {
                DismissibleToast::default(message.to_string())
            };
            stack.add_ephemeral_toast(toast, ctx);
        });
    }
}
