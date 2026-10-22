use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD_NO_PAD;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore as _;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq as _;
use warpui::{AppContext, Entity, ModelContext, SingletonEntity};
use warpui_extras::secure_storage::{self, AppContextExt as _};

const LEGACY_CREDENTIAL_KEY: &str = "warpeer.tab_privacy.password.v1";
const RECORD_FILE_NAME: &str = "tab-privacy-credentials.json";
const RECORD_VERSION: u8 = 2;
const LEGACY_RECORD_VERSION: u8 = 1;
const PBKDF2_ROUNDS: u32 = 30_000;
const MIN_SUPPORTED_ROUNDS: u32 = 20_000;
const MAX_SUPPORTED_ROUNDS: u32 = 1_000_000;
const SALT_LEN: usize = 16;
const HASH_LEN: usize = 32;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SecretRecord {
    rounds: u32,
    salt: String,
    hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RecoveryRecord {
    question: String,
    answer: SecretRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CredentialRecord {
    version: u8,
    password: SecretRecord,
    #[serde(default)]
    recovery: Option<RecoveryRecord>,
}

#[derive(Debug, Deserialize)]
struct LegacyCredentialRecord {
    version: u8,
    rounds: u32,
    salt: String,
    hash: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("privacy password is not configured")]
    Missing,
    #[error("privacy password must not be empty")]
    EmptyPassword,
    #[error("recovery question and answer must not be empty")]
    EmptyRecovery,
    #[error("password recovery is not configured")]
    MissingRecovery,
    #[error("stored privacy credential is invalid")]
    InvalidRecord,
    #[error("privacy credential storage failed: {0}")]
    Storage(String),
}

impl From<io::Error> for CredentialError {
    fn from(error: io::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPrivacyCredentialEvent {
    Changed,
}

pub struct TabPrivacyCredentialModel {
    record: Option<CredentialRecord>,
    load_error: Option<String>,
    path: PathBuf,
}

impl TabPrivacyCredentialModel {
    pub fn load(ctx: &AppContext) -> Self {
        let path = credential_path();
        match read_record(&path) {
            Ok(Some(record)) => {
                remove_legacy_credential(ctx);
                Self {
                    record: Some(record),
                    load_error: None,
                    path,
                }
            }
            Ok(None) => Self::migrate_legacy(path, ctx),
            Err(error) => Self {
                record: None,
                load_error: Some(error.to_string()),
                path,
            },
        }
    }

    fn migrate_legacy(path: PathBuf, ctx: &AppContext) -> Self {
        let value = match ctx.secure_storage().read_value(LEGACY_CREDENTIAL_KEY) {
            Ok(value) => value,
            Err(secure_storage::Error::NotFound) => {
                return Self {
                    record: None,
                    load_error: None,
                    path,
                };
            }
            Err(error) => {
                return Self {
                    record: None,
                    load_error: Some(error.to_string()),
                    path,
                };
            }
        };
        let result = serde_json::from_str::<LegacyCredentialRecord>(&value)
            .map_err(|_| CredentialError::InvalidRecord)
            .and_then(CredentialRecord::from_legacy)
            .and_then(|record| {
                write_record(&path, &record)?;
                Ok(record)
            });
        match result {
            Ok(record) => {
                remove_legacy_credential(ctx);
                Self {
                    record: Some(record),
                    load_error: None,
                    path,
                }
            }
            Err(error) => Self {
                record: None,
                load_error: Some(error.to_string()),
                path,
            },
        }
    }

    pub fn has_credential(&self) -> bool {
        self.record.is_some() && self.load_error.is_none()
    }

    pub fn has_recovery(&self) -> bool {
        self.record
            .as_ref()
            .is_some_and(|record| record.recovery.is_some())
            && self.load_error.is_none()
    }

    pub fn recovery_question(&self) -> Option<&str> {
        self.record
            .as_ref()?
            .recovery
            .as_ref()
            .map(|recovery| recovery.question.as_str())
    }

    pub fn set_password(
        &mut self,
        password: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), CredentialError> {
        if password.is_empty() {
            return Err(CredentialError::EmptyPassword);
        }
        let password = SecretRecord::new(password);
        let recovery = self
            .record
            .as_ref()
            .and_then(|record| record.recovery.clone());
        self.replace_record(
            CredentialRecord {
                version: RECORD_VERSION,
                password,
                recovery,
            },
            ctx,
        )
    }

    pub fn set_recovery(
        &mut self,
        question: &str,
        answer: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), CredentialError> {
        let question = question.trim();
        let answer = normalize_recovery_answer(answer);
        if question.is_empty() || answer.is_empty() {
            return Err(CredentialError::EmptyRecovery);
        }
        let mut record = self.record.clone().ok_or(CredentialError::Missing)?;
        record.recovery = Some(RecoveryRecord {
            question: question.to_string(),
            answer: SecretRecord::new(&answer),
        });
        self.replace_record(record, ctx)
    }

    pub fn verify(&self, password: &str) -> Result<bool, CredentialError> {
        self.ensure_loaded()?;
        let record = self.record.as_ref().ok_or(CredentialError::Missing)?;
        record.validate()?;
        record.password.verify(password)
    }

    pub fn verify_and_upgrade(
        &mut self,
        password: &str,
        ctx: &mut ModelContext<Self>,
    ) -> Result<bool, CredentialError> {
        let verified = self.verify(password)?;
        if verified
            && self
                .record
                .as_ref()
                .is_some_and(|record| record.password.rounds != PBKDF2_ROUNDS)
        {
            self.set_password(password, ctx)?;
        }
        Ok(verified)
    }

    pub fn verify_recovery(&self, answer: &str) -> Result<bool, CredentialError> {
        self.ensure_loaded()?;
        let recovery = self
            .record
            .as_ref()
            .ok_or(CredentialError::Missing)?
            .recovery
            .as_ref()
            .ok_or(CredentialError::MissingRecovery)?;
        self.record
            .as_ref()
            .ok_or(CredentialError::Missing)?
            .validate()?;
        recovery.answer.verify(&normalize_recovery_answer(answer))
    }

    pub fn clear(&mut self, ctx: &mut ModelContext<Self>) -> Result<(), CredentialError> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        remove_legacy_credential(ctx);
        self.record = None;
        self.load_error = None;
        ctx.emit(TabPrivacyCredentialEvent::Changed);
        Ok(())
    }

    fn ensure_loaded(&self) -> Result<(), CredentialError> {
        if let Some(error) = &self.load_error {
            return Err(CredentialError::Storage(error.clone()));
        }
        Ok(())
    }

    fn replace_record(
        &mut self,
        record: CredentialRecord,
        ctx: &mut ModelContext<Self>,
    ) -> Result<(), CredentialError> {
        write_record(&self.path, &record)?;
        self.record = Some(record);
        self.load_error = None;
        remove_legacy_credential(ctx);
        ctx.emit(TabPrivacyCredentialEvent::Changed);
        Ok(())
    }

    #[cfg(feature = "integration_tests")]
    pub(crate) fn set_password_in_memory_for_integration(
        &mut self,
        password: &str,
        ctx: &mut ModelContext<Self>,
    ) {
        self.record = Some(CredentialRecord {
            version: RECORD_VERSION,
            password: SecretRecord::new_with_salt(password, [7; SALT_LEN]),
            recovery: None,
        });
        self.load_error = None;
        ctx.emit(TabPrivacyCredentialEvent::Changed);
    }
}

impl CredentialRecord {
    fn from_legacy(record: LegacyCredentialRecord) -> Result<Self, CredentialError> {
        if record.version != LEGACY_RECORD_VERSION {
            return Err(CredentialError::InvalidRecord);
        }
        let password = SecretRecord {
            rounds: record.rounds,
            salt: record.salt,
            hash: record.hash,
        };
        password.validate()?;
        Ok(Self {
            version: RECORD_VERSION,
            password,
            recovery: None,
        })
    }

    fn validate(&self) -> Result<(), CredentialError> {
        if self.version != RECORD_VERSION {
            return Err(CredentialError::InvalidRecord);
        }
        self.password.validate()?;
        if let Some(recovery) = &self.recovery {
            if recovery.question.trim().is_empty() {
                return Err(CredentialError::InvalidRecord);
            }
            recovery.answer.validate()?;
        }
        Ok(())
    }
}

impl SecretRecord {
    fn new(value: &str) -> Self {
        let mut salt = [0; SALT_LEN];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        Self::new_with_salt(value, salt)
    }

    fn new_with_salt(value: &str, salt: [u8; SALT_LEN]) -> Self {
        Self {
            rounds: PBKDF2_ROUNDS,
            salt: STANDARD_NO_PAD.encode(salt),
            hash: STANDARD_NO_PAD.encode(derive(value, &salt, PBKDF2_ROUNDS)),
        }
    }

    fn validate(&self) -> Result<(), CredentialError> {
        if !(MIN_SUPPORTED_ROUNDS..=MAX_SUPPORTED_ROUNDS).contains(&self.rounds) {
            return Err(CredentialError::InvalidRecord);
        }
        let salt = STANDARD_NO_PAD
            .decode(&self.salt)
            .map_err(|_| CredentialError::InvalidRecord)?;
        let hash = STANDARD_NO_PAD
            .decode(&self.hash)
            .map_err(|_| CredentialError::InvalidRecord)?;
        if salt.len() != SALT_LEN || hash.len() != HASH_LEN {
            return Err(CredentialError::InvalidRecord);
        }
        Ok(())
    }

    fn verify(&self, value: &str) -> Result<bool, CredentialError> {
        self.validate()?;
        let salt = STANDARD_NO_PAD
            .decode(&self.salt)
            .map_err(|_| CredentialError::InvalidRecord)?;
        let expected = STANDARD_NO_PAD
            .decode(&self.hash)
            .map_err(|_| CredentialError::InvalidRecord)?;
        let actual = derive(value, &salt, self.rounds);
        Ok(actual.ct_eq(expected.as_slice()).into())
    }
}

fn credential_path() -> PathBuf {
    warp_core::paths::data_dir().join(RECORD_FILE_NAME)
}

fn read_record(path: &Path) -> Result<Option<CredentialRecord>, CredentialError> {
    let value = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let record = serde_json::from_str::<CredentialRecord>(&value)
        .map_err(|_| CredentialError::InvalidRecord)?;
    record.validate()?;
    Ok(Some(record))
}

fn write_record(path: &Path, record: &CredentialRecord) -> Result<(), CredentialError> {
    record.validate()?;
    let parent = path.parent().ok_or(CredentialError::InvalidRecord)?;
    fs::create_dir_all(parent)?;
    set_owner_only_directory_permissions(parent)?;
    let temporary_path = path.with_extension("json.tmp");
    let serialized = serde_json::to_vec(record).map_err(|_| CredentialError::InvalidRecord)?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary_path)?;
    file.write_all(&serialized)?;
    file.sync_all()?;
    set_owner_only_file_permissions(&temporary_path)?;
    fs::rename(&temporary_path, path)?;
    set_owner_only_file_permissions(path)?;
    Ok(())
}

fn set_owner_only_directory_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_owner_only_file_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn remove_legacy_credential(ctx: &AppContext) {
    match ctx.secure_storage().remove_value(LEGACY_CREDENTIAL_KEY) {
        Ok(()) | Err(secure_storage::Error::NotFound) => {}
        Err(error) => log::warn!("failed to remove legacy tab privacy keychain item: {error}"),
    }
}

fn normalize_recovery_answer(answer: &str) -> String {
    answer.trim().to_lowercase()
}

fn derive(value: &str, salt: &[u8], rounds: u32) -> [u8; HASH_LEN] {
    let mut output = [0; HASH_LEN];
    pbkdf2_hmac::<Sha256>(value.as_bytes(), salt, rounds, &mut output);
    output
}

impl Entity for TabPrivacyCredentialModel {
    type Event = TabPrivacyCredentialEvent;
}

impl SingletonEntity for TabPrivacyCredentialModel {}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
