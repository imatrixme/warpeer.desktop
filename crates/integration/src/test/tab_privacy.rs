use warp::integration_testing::settings::open_settings_page;
use warp::integration_testing::tab_privacy::{
    assert_active_tab_privacy_state, assert_biometric_switch_result,
    assert_privacy_password_created, assert_settings_tab_privacy_state, assert_system_lock_setting,
    assert_tab_privacy_password_retry_ready, click_security_biometric_switch,
    click_security_lock_all_button, click_security_password_button, click_security_privacy_switch,
    click_security_system_lock_switch, complete_biometric_unlock_after_transient_focus_loss,
    lock_active_private_tab, protect_active_tab_with_password, submit_tab_privacy_password,
};
use warp::integration_testing::terminal::util::ExpectedExitStatus;
use warp::integration_testing::terminal::{
    execute_command_for_single_terminal_in_tab, wait_until_bootstrapped_single_pane_for_tab,
};
use warp::settings_view::SettingsSection;
use warpui_core::integration::TestStep;

use super::{Builder, new_builder};

const PASSWORD: &str = "warpeer-integration-password";

pub fn test_warpeer_tab_privacy_lock_unlock() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(execute_command_for_single_terminal_in_tab(
            0,
            "printf 'PRIVATE-CONTENT-1234567890\\n'".to_string(),
            ExpectedExitStatus::Success,
            "PRIVATE-CONTENT-1234567890".to_string(),
        ))
        .with_step(protect_active_tab_with_password(PASSWORD))
        .with_step(assert_active_tab_privacy_state(true, true, false))
        .with_step(submit_tab_privacy_password("incorrect-password"))
        .with_step(assert_tab_privacy_password_retry_ready())
        .with_step(submit_tab_privacy_password(PASSWORD))
        .with_step(assert_active_tab_privacy_state(false, false, true))
        .with_step(lock_active_private_tab())
        .with_step(assert_active_tab_privacy_state(true, true, false))
        .with_step(complete_biometric_unlock_after_transient_focus_loss())
        .with_step(assert_active_tab_privacy_state(false, false, true))
}

pub fn test_warpeer_settings_tab_cannot_be_locked() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(open_settings_page(SettingsSection::Security))
        .with_step(click_security_privacy_switch())
        .with_step(assert_settings_tab_privacy_state(true, true, false))
        .with_step(TestStep::new("Tab to password confirmation").with_keystrokes(&["tab"]))
        .with_step(assert_settings_tab_privacy_state(true, false, true))
        .with_step(TestStep::new("Shift-Tab back to password").with_keystrokes(&["shift-tab"]))
        .with_step(assert_settings_tab_privacy_state(true, true, false))
        .with_step(
            TestStep::new("Create privacy password with the default Enter action")
                .with_typed_characters(&[PASSWORD])
                .with_keystrokes(&["tab"])
                .with_typed_characters(&[PASSWORD])
                .with_keystrokes(&["enter"]),
        )
        .with_step(assert_privacy_password_created())
        .with_step(click_security_password_button())
        .with_step(assert_settings_tab_privacy_state(true, true, false))
        .with_step(TestStep::new("Cancel password change with Escape").with_keystrokes(&["escape"]))
        .with_step(assert_privacy_password_created())
        .with_step(click_security_biometric_switch())
        .with_step(assert_biometric_switch_result())
        .with_step(click_security_system_lock_switch())
        .with_step(assert_system_lock_setting(false))
        .with_step(click_security_lock_all_button())
        .with_step(assert_privacy_password_created())
        .with_step(lock_active_private_tab())
        .with_step(assert_privacy_password_created())
}
