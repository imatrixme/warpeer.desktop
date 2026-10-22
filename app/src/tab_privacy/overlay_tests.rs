use std::time::{Duration, Instant};

use warpui::EntityId;

use super::verification::{
    PasswordVerificationAnimation, VERIFICATION_FORWARD_DURATION, VERIFICATION_RETURN_DURATION,
    VerificationAdvance,
};
use super::{PasswordField, TabPrivacyPromptPurpose};

#[test]
fn password_creation_confirms_the_primary_password_without_unlocking() {
    let purpose = TabPrivacyPromptPurpose::CreatePassword;

    assert!(purpose.needs_confirmation());
    assert!(!purpose.needs_new_password());
    assert!(!purpose.is_unlock());
}

#[test]
fn password_change_verifies_current_password_before_showing_new_fields() {
    let purpose = TabPrivacyPromptPurpose::ChangePassword;

    assert_eq!(
        purpose.password_fields(false),
        vec![PasswordField::Password]
    );
    assert!(!purpose.needs_new_password());
    assert!(!purpose.needs_confirmation());
    assert!(!purpose.is_unlock());
}

#[test]
fn authentication_prompts_support_biometrics() {
    let authentication_prompts = [
        TabPrivacyPromptPurpose::UnlockTab(EntityId::from_usize(1)),
        TabPrivacyPromptPurpose::UnlockAll,
        TabPrivacyPromptPurpose::ChangePassword,
        TabPrivacyPromptPurpose::ResetPassword,
        TabPrivacyPromptPurpose::RemoveProtection(EntityId::from_usize(2)),
        TabPrivacyPromptPurpose::DisableProtection,
        TabPrivacyPromptPurpose::ConfigureRecovery,
    ];

    assert!(
        authentication_prompts
            .into_iter()
            .all(TabPrivacyPromptPurpose::requires_authentication)
    );
    assert!(TabPrivacyPromptPurpose::UnlockTab(EntityId::from_usize(3)).is_unlock());
    assert!(TabPrivacyPromptPurpose::UnlockAll.is_unlock());
    assert!(
        !TabPrivacyPromptPurpose::CreateAndProtect(EntityId::from_usize(4))
            .requires_authentication()
    );
}

#[test]
fn password_field_order_matches_each_prompt() {
    assert_eq!(
        TabPrivacyPromptPurpose::CreatePassword.password_fields(false),
        vec![PasswordField::Password, PasswordField::Confirmation]
    );
    assert_eq!(
        TabPrivacyPromptPurpose::ChangePassword.password_fields(false),
        vec![PasswordField::Password]
    );
    assert_eq!(
        TabPrivacyPromptPurpose::UnlockAll.password_fields(false),
        vec![PasswordField::Password]
    );
    assert_eq!(
        TabPrivacyPromptPurpose::SetNewPassword.password_fields(false),
        vec![PasswordField::NewPassword, PasswordField::Confirmation]
    );
}

#[test]
fn successful_verification_waits_for_the_slider_endpoint() {
    let started_at = Instant::now();
    let purpose = TabPrivacyPromptPurpose::UnlockAll;
    let mut animation = PasswordVerificationAnimation::new(purpose, started_at);
    animation.resolve(Ok(()));

    assert_eq!(animation.progress_at(started_at), 0.);
    assert_eq!(
        animation.advance(started_at + VERIFICATION_FORWARD_DURATION - Duration::from_millis(1)),
        VerificationAdvance::Continue
    );
    assert_eq!(
        animation.advance(started_at + VERIFICATION_FORWARD_DURATION),
        VerificationAdvance::Succeeded(purpose)
    );
}

#[test]
fn failed_verification_returns_before_restoring_the_password_field() {
    let started_at = Instant::now();
    let mut animation =
        PasswordVerificationAnimation::new(TabPrivacyPromptPurpose::UnlockAll, started_at);
    animation.resolve(Err("Incorrect password. Try again.".to_string()));
    let endpoint = started_at + VERIFICATION_FORWARD_DURATION;

    assert_eq!(animation.advance(endpoint), VerificationAdvance::Continue);
    assert_eq!(animation.progress_at(endpoint), 1.);
    assert!(animation.progress_at(endpoint + VERIFICATION_RETURN_DURATION / 2) < 1.);
    assert_eq!(
        animation.advance(endpoint + VERIFICATION_RETURN_DURATION),
        VerificationAdvance::Failed("Incorrect password. Try again.".to_string())
    );
}
