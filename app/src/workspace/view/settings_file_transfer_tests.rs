use std::fs;

use super::{export_settings_file, import_settings_file};

#[test]
fn imports_valid_toml_and_replaces_the_active_file() {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("import.toml");
    let destination = directory.path().join("active/settings.toml");
    fs::write(&source, "theme = \"Dracula\"\n").expect("write source");
    fs::create_dir_all(destination.parent().unwrap()).expect("create destination directory");
    fs::write(&destination, "theme = \"System\"\n").expect("write destination");

    import_settings_file(&source, &destination).expect("import settings");

    assert_eq!(
        fs::read_to_string(destination).expect("read imported settings"),
        "theme = \"Dracula\"\n"
    );
}

#[test]
fn invalid_toml_does_not_replace_the_active_file() {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("invalid.toml");
    let destination = directory.path().join("settings.toml");
    fs::write(&source, "theme = [\n").expect("write source");
    fs::write(&destination, "theme = \"System\"\n").expect("write destination");

    assert!(import_settings_file(&source, &destination).is_err());
    assert_eq!(
        fs::read_to_string(destination).expect("read unchanged settings"),
        "theme = \"System\"\n"
    );
}

#[test]
fn exports_settings_and_replaces_an_existing_file() {
    let directory = tempfile::tempdir().expect("create temp directory");
    let source = directory.path().join("active.toml");
    let destination = directory.path().join("export/settings.toml");
    fs::write(&source, "theme = \"Dracula\"\n").expect("write source");
    fs::create_dir_all(destination.parent().unwrap()).expect("create export directory");
    fs::write(&destination, "old = true\n").expect("write existing export");

    export_settings_file(&source, &destination).expect("export settings");

    assert_eq!(
        fs::read_to_string(destination).expect("read exported settings"),
        "theme = \"Dracula\"\n"
    );
}
