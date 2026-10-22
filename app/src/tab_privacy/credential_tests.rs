use tempfile::TempDir;

use super::*;

fn model_for(password: &str) -> (TempDir, TabPrivacyCredentialModel) {
    let directory = tempfile::tempdir().unwrap();
    let password = SecretRecord::new_with_salt(password, [7; SALT_LEN]);
    (
        directory,
        TabPrivacyCredentialModel {
            record: Some(CredentialRecord {
                version: RECORD_VERSION,
                password,
                recovery: None,
            }),
            load_error: None,
            path: PathBuf::new(),
        },
    )
}

#[test]
fn verifies_correct_and_incorrect_passwords() {
    let (_directory, model) = model_for("correct horse battery staple");

    assert!(model.verify("correct horse battery staple").unwrap());
    assert!(!model.verify("incorrect").unwrap());
}

#[test]
fn verifies_normalized_recovery_answers() {
    let (_directory, mut model) = model_for("password");
    model.record.as_mut().unwrap().recovery = Some(RecoveryRecord {
        question: "What was your first project?".to_string(),
        answer: SecretRecord::new_with_salt("terminal", [9; SALT_LEN]),
    });

    assert!(model.verify_recovery("  TERMINAL ").unwrap());
    assert!(!model.verify_recovery("editor").unwrap());
    assert_eq!(
        model.recovery_question(),
        Some("What was your first project?")
    );
}

#[test]
fn rejects_unsupported_and_malformed_records() {
    let (_directory, mut unsupported) = model_for("password");
    unsupported.record.as_mut().unwrap().version += 1;
    assert!(matches!(
        unsupported.verify("password"),
        Err(CredentialError::InvalidRecord)
    ));

    let (_directory, mut malformed) = model_for("password");
    malformed.record.as_mut().unwrap().password.hash = "not-base64".to_string();
    assert!(matches!(
        malformed.verify("password"),
        Err(CredentialError::InvalidRecord)
    ));

    let (_directory, mut unexpected_rounds) = model_for("password");
    unexpected_rounds.record.as_mut().unwrap().password.rounds = MIN_SUPPORTED_ROUNDS - 1;
    assert!(matches!(
        unexpected_rounds.verify("password"),
        Err(CredentialError::InvalidRecord)
    ));
}

#[test]
fn reads_and_writes_owner_only_local_records() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(RECORD_FILE_NAME);
    let record = CredentialRecord {
        version: RECORD_VERSION,
        password: SecretRecord::new_with_salt("password", [7; SALT_LEN]),
        recovery: None,
    };

    write_record(&path, &record).unwrap();
    let loaded = read_record(&path).unwrap().unwrap();
    assert!(loaded.password.verify("password").unwrap());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn reports_missing_and_storage_failures() {
    let missing = TabPrivacyCredentialModel {
        record: None,
        load_error: None,
        path: PathBuf::new(),
    };
    assert!(matches!(
        missing.verify("password"),
        Err(CredentialError::Missing)
    ));

    let unavailable = TabPrivacyCredentialModel {
        record: None,
        load_error: Some("local storage unavailable".to_string()),
        path: PathBuf::new(),
    };
    assert!(matches!(
        unavailable.verify("password"),
        Err(CredentialError::Storage(_))
    ));
}

#[test]
fn replacing_a_record_invalidates_the_previous_password() {
    let (_directory, mut model) = model_for("old password");
    assert!(model.verify("old password").unwrap());

    model.record.as_mut().unwrap().password =
        SecretRecord::new_with_salt("new password", [8; SALT_LEN]);
    assert!(!model.verify("old password").unwrap());
    assert!(model.verify("new password").unwrap());
}
