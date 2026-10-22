use crate::ManagedSecretValue;

pub fn init() {}

#[derive(thiserror::Error, Debug)]
pub enum EnvelopeError {
    #[error("managed secret encryption is unavailable in this product profile")]
    Unavailable,
}

pub struct UploadKey;

impl UploadKey {
    pub fn import_public_keyset(_public_key: &str) -> Result<Self, EnvelopeError> {
        Err(EnvelopeError::Unavailable)
    }

    pub fn encrypt_secret(
        &self,
        _actor_uid: &str,
        _secret_name: &str,
        _secret: &ManagedSecretValue,
    ) -> Result<String, EnvelopeError> {
        Err(EnvelopeError::Unavailable)
    }

    pub fn seal_with_context(
        &self,
        _context_info: &[u8],
        _plaintext: &[u8],
    ) -> Result<String, EnvelopeError> {
        Err(EnvelopeError::Unavailable)
    }
}
