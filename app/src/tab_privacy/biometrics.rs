#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BiometricAuthenticationResult {
    Success,
    Cancelled,
    Unavailable,
    Failed(String),
}

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::{Arc, Mutex};

    use block2::RcBlock;
    use futures::channel::oneshot;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAError, LAPolicy};

    use super::BiometricAuthenticationResult;

    const POLICY: LAPolicy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;

    pub fn available() -> bool {
        autoreleasepool(|_| unsafe { LAContext::new().canEvaluatePolicy_error(POLICY).is_ok() })
    }

    pub async fn authenticate() -> BiometricAuthenticationResult {
        let (sender, receiver) = oneshot::channel();
        let sender = Arc::new(Mutex::new(Some(sender)));

        let started = autoreleasepool(|_| unsafe {
            let context = LAContext::new();
            if context.canEvaluatePolicy_error(POLICY).is_err() {
                return false;
            }

            let context_keep_alive = context.clone();
            let sender = sender.clone();
            let reply = RcBlock::new(move |success: Bool, error: *mut NSError| {
                let _context_keep_alive = &context_keep_alive;
                let result = if success.as_bool() {
                    BiometricAuthenticationResult::Success
                } else if error.is_null() {
                    BiometricAuthenticationResult::Failed("Touch ID failed".into())
                } else {
                    let error = &*error;
                    match LAError(error.code()) {
                        LAError::UserCancel
                        | LAError::UserFallback
                        | LAError::SystemCancel
                        | LAError::AppCancel => BiometricAuthenticationResult::Cancelled,
                        LAError::BiometryNotAvailable
                        | LAError::BiometryNotEnrolled
                        | LAError::BiometryLockout => BiometricAuthenticationResult::Unavailable,
                        LAError::AuthenticationFailed
                        | LAError::PasscodeNotSet
                        | LAError::InvalidContext
                        | LAError::NotInteractive
                        | LAError::CompanionNotAvailable
                        | LAError::BiometryNotPaired
                        | LAError::BiometryDisconnected
                        | LAError::InvalidDimensions
                        | LAError(_) => BiometricAuthenticationResult::Failed(
                            error.localizedDescription().to_string(),
                        ),
                    }
                };
                if let Some(sender) = sender.lock().expect("biometric sender poisoned").take() {
                    let _ = sender.send(result);
                }
            });
            let reason = NSString::from_str("unlock protected terminal tabs");
            context.evaluatePolicy_localizedReason_reply(POLICY, &reason, &reply);
            true
        });

        if !started {
            return BiometricAuthenticationResult::Unavailable;
        }
        receiver
            .await
            .unwrap_or(BiometricAuthenticationResult::Cancelled)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::BiometricAuthenticationResult;

    pub fn available() -> bool {
        false
    }

    pub async fn authenticate() -> BiometricAuthenticationResult {
        BiometricAuthenticationResult::Unavailable
    }
}

pub fn biometrics_available() -> bool {
    platform::available()
}

pub async fn authenticate_biometrically() -> BiometricAuthenticationResult {
    platform::authenticate().await
}
