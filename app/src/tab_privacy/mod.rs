mod biometrics;
mod credential;
mod overlay;
mod reveal;
mod runtime;
#[cfg(target_os = "macos")]
mod screen_lock;
mod settings;

use ::settings::Setting;
pub use biometrics::{
    BiometricAuthenticationResult, authenticate_biometrically, biometrics_available,
};
pub use credential::TabPrivacyCredentialModel;
pub(crate) use overlay::TAB_PRIVACY_REVEAL_DURATION;
pub use overlay::{
    TabPrivacyOverlayEvent, TabPrivacyOverlayView, TabPrivacyPromptPurpose,
    TabPrivacySetupCompletion,
};
pub use runtime::{
    AuthenticationScope, TabPrivacyRuntime, TabPrivacyRuntimeEvent, TabPrivacyState,
};
pub use settings::{TabPrivacySettings, TabPrivacySettingsChangedEvent, TabRelockDelay};
use warpui::{AppContext, SingletonEntity as _};

pub fn init(ctx: &mut AppContext) {
    let credentials = TabPrivacyCredentialModel::load(ctx);
    ctx.add_singleton_model(move |_| credentials);
    ctx.add_singleton_model(|_| TabPrivacyRuntime::new());
    #[cfg(target_os = "macos")]
    ctx.add_singleton_model(screen_lock::ScreenLockMonitor::new);

    ctx.subscribe_to_model(&TabPrivacySettings::handle(ctx), |_, event, ctx| {
        if matches!(
            event,
            TabPrivacySettingsChangedEvent::TabPrivacyRelockDelay { .. }
        ) {
            let delay = *TabPrivacySettings::as_ref(ctx).relock_delay.value();
            TabPrivacyRuntime::handle(ctx).update(ctx, |runtime, ctx| {
                runtime.apply_delay_change(delay, ctx);
            });
        }
    });
}
