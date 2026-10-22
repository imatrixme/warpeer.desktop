use warpui::integration::TestStep;
use warpui::{SingletonEntity as _, TypedActionView as _, async_assert};

use super::view_getters::{single_input_view_for_tab, workspace_view};
use crate::settings_view::{
    SECURITY_BIOMETRIC_SWITCH_POSITION_ID, SECURITY_LOCK_ALL_BUTTON_POSITION_ID,
    SECURITY_PASSWORD_BUTTON_POSITION_ID, SECURITY_PRIVACY_SWITCH_POSITION_ID,
    SECURITY_SYSTEM_LOCK_SWITCH_POSITION_ID,
};
use crate::tab_privacy::{TabPrivacyCredentialModel, TabPrivacySettings, biometrics_available};
use crate::workspace::WorkspaceAction;

pub fn protect_active_tab_with_password(password: &'static str) -> TestStep {
    TestStep::new("Protect and lock the active tab").with_action(move |app, window_id, _| {
        workspace_view(app, window_id).update(app, |workspace, ctx| {
            workspace.protect_active_tab_for_privacy_integration(password, ctx);
        });
    })
}

pub fn submit_tab_privacy_password(password: &'static str) -> TestStep {
    TestStep::new("Submit the tab privacy password").with_action(move |app, window_id, _| {
        workspace_view(app, window_id).update(app, |workspace, ctx| {
            workspace.submit_tab_privacy_password_for_integration(password, ctx);
        });
    })
}

pub fn lock_active_private_tab() -> TestStep {
    TestStep::new("Lock the active private tab").with_action(|app, window_id, _| {
        workspace_view(app, window_id).update(app, |workspace, ctx| {
            workspace.handle_action(&WorkspaceAction::ToggleActiveTabPrivacy, ctx);
        });
    })
}

pub fn complete_biometric_unlock_after_transient_focus_loss() -> TestStep {
    TestStep::new("Complete biometric unlock after transient focus loss").with_action(
        |app, window_id, _| {
            workspace_view(app, window_id).update(app, |workspace, ctx| {
                workspace.complete_biometric_unlock_after_transient_focus_loss_for_integration(ctx);
            });
        },
    )
}

pub fn click_security_privacy_switch() -> TestStep {
    TestStep::new("Click the Security privacy switch")
        .with_click_on_saved_position(SECURITY_PRIVACY_SWITCH_POSITION_ID)
}

pub fn click_security_password_button() -> TestStep {
    TestStep::new("Click the Security password button")
        .with_click_on_saved_position(SECURITY_PASSWORD_BUTTON_POSITION_ID)
}

pub fn click_security_biometric_switch() -> TestStep {
    TestStep::new("Click the Security biometric switch")
        .with_click_on_saved_position(SECURITY_BIOMETRIC_SWITCH_POSITION_ID)
}

pub fn click_security_system_lock_switch() -> TestStep {
    TestStep::new("Click the Security system lock switch")
        .with_click_on_saved_position(SECURITY_SYSTEM_LOCK_SWITCH_POSITION_ID)
}

pub fn click_security_lock_all_button() -> TestStep {
    TestStep::new("Click the Security Lock All button")
        .with_click_on_saved_position(SECURITY_LOCK_ALL_BUTTON_POSITION_ID)
}

pub fn assert_settings_tab_privacy_state(
    overlay_visible: bool,
    password_focused: bool,
    confirmation_focused: bool,
) -> TestStep {
    TestStep::new("Assert Settings tab privacy state").add_named_assertion(
        "Settings remains public and password field focus matches",
        move |app, window_id| {
            let ((protected, locked, actual_overlay_visible, _), focus) =
                workspace_view(app, window_id).read(app, |workspace, ctx| {
                    (
                        workspace.active_tab_privacy_state_for_integration(ctx),
                        workspace.tab_privacy_editor_focus_for_integration(ctx),
                    )
                });
            async_assert!(
                !protected
                    && !locked
                    && actual_overlay_visible == overlay_visible
                    && focus == (password_focused, confirmation_focused),
                "Settings privacy state did not match: protected={protected}, locked={locked}, overlay={actual_overlay_visible}, password_focused={}, confirmation_focused={}",
                focus.0,
                focus.1
            )
        },
    )
}

pub fn assert_privacy_password_created() -> TestStep {
    TestStep::new("Assert privacy password was created").add_named_assertion(
        "credential exists and password prompt is closed",
        |app, window_id| {
            let has_credential = TabPrivacyCredentialModel::handle(app)
                .read(app, |model, _| model.has_credential());
            let (protected, locked, overlay_visible, _) =
                workspace_view(app, window_id).read(app, |workspace, ctx| {
                    workspace.active_tab_privacy_state_for_integration(ctx)
                });
            async_assert!(
                has_credential && !protected && !locked && !overlay_visible,
                "privacy password state did not match: credential={has_credential}, protected={protected}, locked={locked}, overlay={overlay_visible}"
            )
        },
    )
}

pub fn assert_biometric_switch_result() -> TestStep {
    TestStep::new("Assert biometric switch result").add_named_assertion(
        "Touch ID enables when available and remains off otherwise",
        |app, _| {
            let enabled = TabPrivacySettings::handle(app)
                .read(app, |settings, _| *settings.biometric_quick_unlock);
            async_assert!(
                enabled == biometrics_available(),
                "Touch ID setting did not match availability: enabled={enabled}, available={}",
                biometrics_available()
            )
        },
    )
}

pub fn assert_system_lock_setting(expected: bool) -> TestStep {
    TestStep::new("Assert system lock setting").add_named_assertion(
        "system lock setting matches the clicked switch",
        move |app, _| {
            let actual = TabPrivacySettings::handle(app)
                .read(app, |settings, _| *settings.lock_on_system_inactivity);
            async_assert!(
                actual == expected,
                "system lock setting should be {expected}, was {actual}"
            )
        },
    )
}

pub fn assert_active_tab_privacy_state(
    locked: bool,
    overlay_visible: bool,
    input_focused: bool,
) -> TestStep {
    TestStep::new("Assert active tab privacy state").add_named_assertion(
        "privacy state, redaction, overlay, and focus match",
        move |app, window_id| {
            let (protected, actual_locked, actual_overlay_visible, title) =
                workspace_view(app, window_id).read(app, |workspace, ctx| {
                    workspace.active_tab_privacy_state_for_integration(ctx)
                });
            let input = single_input_view_for_tab(app, window_id, 0);
            let actual_input_focused =
                input.read(app, |input, ctx| input.editor().is_focused(ctx));

            async_assert!(
                protected
                    && actual_locked == locked
                    && actual_overlay_visible == overlay_visible
                    && actual_input_focused == input_focused
                    && (!locked || title == "Private tab"),
                "privacy state did not match: protected={protected}, locked={actual_locked}, overlay={actual_overlay_visible}, input_focused={actual_input_focused}, title={title:?}"
            )
        },
    )
}

pub fn assert_tab_privacy_password_retry_ready() -> TestStep {
    TestStep::new("Assert tab privacy password retry is ready").add_named_assertion(
        "password verification returned to the editable state",
        |app, window_id| {
            let ((protected, locked, overlay_visible, _), editor_focus) =
                workspace_view(app, window_id).read(app, |workspace, ctx| {
                    (
                        workspace.active_tab_privacy_state_for_integration(ctx),
                        workspace.tab_privacy_editor_focus_for_integration(ctx),
                    )
                });
            async_assert!(
                protected
                    && locked
                    && overlay_visible
                    && editor_focus == (true, false),
                "privacy password retry was not ready: protected={protected}, locked={locked}, overlay={overlay_visible}, password_focused={}, confirmation_focused={}",
                editor_focus.0,
                editor_focus.1
            )
        },
    )
}
