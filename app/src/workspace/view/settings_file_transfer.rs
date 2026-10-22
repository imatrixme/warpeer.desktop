use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use tempfile::NamedTempFile;
use warpui::modals::{AlertDialogWithCallbacks, AppModalCallback, ModalButton};
use warpui::platform::FilePickerConfiguration;
use warpui::{AppContext, ViewContext};

use super::Workspace;
use crate::view_components::DismissibleToast;

impl Workspace {
    pub(super) fn open_settings_import_picker(&mut self, ctx: &mut ViewContext<Self>) {
        let workspace = ctx.handle();
        ctx.open_file_picker(
            move |result, ctx| match result {
                Ok(paths) => {
                    let Some(path) = paths.first() else {
                        return;
                    };
                    let source = PathBuf::from(path);
                    if let Some(workspace) = workspace.upgrade(ctx) {
                        workspace.update(ctx, |workspace, ctx| {
                            workspace.confirm_settings_import(source, ctx);
                        });
                    }
                }
                Err(error) => {
                    if let Some(workspace) = workspace.upgrade(ctx) {
                        workspace.update(ctx, |workspace, ctx| {
                            workspace.show_settings_transfer_error(
                                format!("Failed to choose settings file: {error}"),
                                ctx,
                            );
                        });
                    }
                }
            },
            FilePickerConfiguration::new(),
        );
    }

    pub(super) fn open_settings_export_picker(&mut self, ctx: &mut ViewContext<Self>) {
        let workspace = ctx.handle();
        ctx.open_file_picker(
            move |result, ctx| match result {
                Ok(paths) => {
                    let Some(path) = paths.first() else {
                        return;
                    };
                    let directory = PathBuf::from(path);
                    if let Some(workspace) = workspace.upgrade(ctx) {
                        workspace.update(ctx, |workspace, ctx| {
                            workspace.confirm_settings_export(directory, ctx);
                        });
                    }
                }
                Err(error) => {
                    if let Some(workspace) = workspace.upgrade(ctx) {
                        workspace.update(ctx, |workspace, ctx| {
                            workspace.show_settings_transfer_error(
                                format!("Failed to choose export folder: {error}"),
                                ctx,
                            );
                        });
                    }
                }
            },
            FilePickerConfiguration::new().folders_only(),
        );
    }

    fn confirm_settings_import(&mut self, source: PathBuf, ctx: &mut ViewContext<Self>) {
        let destination = crate::settings::user_preferences_toml_file_path();
        let workspace = ctx.handle();
        let source_for_action = source.clone();
        let destination_for_action = destination.clone();
        let dialog = AlertDialogWithCallbacks::for_app(
            "Import settings?",
            format!(
                "Source:\n{}\n\nDestination:\n{}\n\nThe current settings file will be replaced.",
                source.display(),
                destination.display()
            ),
            vec![
                ModalButton::for_app("Import", move |app| {
                    if let Some(workspace) = workspace.upgrade(app) {
                        workspace.update(app, |workspace, ctx| {
                            let result =
                                import_settings_file(&source_for_action, &destination_for_action);
                            workspace.show_settings_transfer_result(
                                result,
                                "Settings imported",
                                "Failed to import settings",
                                ctx,
                            );
                        });
                    }
                }),
                ModalButton::for_app("Cancel", |_| {}),
            ],
            |_| {},
        );
        self.show_settings_transfer_dialog(dialog, ctx);
    }

    fn confirm_settings_export(&mut self, directory: PathBuf, ctx: &mut ViewContext<Self>) {
        let source = crate::settings::user_preferences_toml_file_path();
        let destination = directory.join("settings.toml");
        let destination_exists = destination.exists();
        let workspace = ctx.handle();
        let source_for_action = source.clone();
        let destination_for_action = destination.clone();
        let title = if destination_exists {
            "Replace exported settings?"
        } else {
            "Export settings?"
        };
        let overwrite_notice = if destination_exists {
            "\n\nA settings.toml file already exists there and will be replaced."
        } else {
            ""
        };
        let confirm_label = if destination_exists {
            "Replace"
        } else {
            "Export"
        };
        let dialog = AlertDialogWithCallbacks::for_app(
            title,
            format!(
                "Folder:\n{}\n\nFile:\n{}{}",
                directory.display(),
                destination.display(),
                overwrite_notice
            ),
            vec![
                ModalButton::for_app(confirm_label, move |app| {
                    if let Some(workspace) = workspace.upgrade(app) {
                        workspace.update(app, |workspace, ctx| {
                            let result =
                                export_settings_file(&source_for_action, &destination_for_action);
                            workspace.show_settings_transfer_result(
                                result,
                                "Settings exported",
                                "Failed to export settings",
                                ctx,
                            );
                        });
                    }
                }),
                ModalButton::for_app("Cancel", |_| {}),
            ],
            |_| {},
        );
        self.show_settings_transfer_dialog(dialog, ctx);
    }

    fn show_settings_transfer_dialog(
        &mut self,
        dialog: AlertDialogWithCallbacks<AppModalCallback>,
        ctx: &mut ViewContext<Self>,
    ) {
        if cfg!(target_os = "macos") {
            AppContext::show_native_platform_modal(ctx, dialog);
        } else {
            self.show_native_modal(dialog, ctx);
        }
    }

    fn show_settings_transfer_result(
        &mut self,
        result: Result<PathBuf>,
        success_message: &str,
        error_message: &str,
        ctx: &mut ViewContext<Self>,
    ) {
        let toast = match result {
            Ok(path) => DismissibleToast::success(format!("{success_message}: {}", path.display())),
            Err(error) => DismissibleToast::error(format!("{error_message}: {error:#}")),
        };
        self.toast_stack.update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(toast, ctx);
        });
    }

    fn show_settings_transfer_error(&mut self, message: String, ctx: &mut ViewContext<Self>) {
        self.toast_stack.update(ctx, |toast_stack, ctx| {
            toast_stack.add_ephemeral_toast(DismissibleToast::error(message), ctx);
        });
    }
}

fn import_settings_file(source: &Path, destination: &Path) -> Result<PathBuf> {
    ensure_distinct_paths(source, destination)?;
    let contents = fs::read_to_string(source)
        .with_context(|| format!("failed to read {}", source.display()))?;
    toml::from_str::<toml::Value>(&contents)
        .with_context(|| format!("{} is not valid TOML", source.display()))?;
    write_atomically(destination, contents.as_bytes())?;
    Ok(destination.to_path_buf())
}

fn export_settings_file(source: &Path, destination: &Path) -> Result<PathBuf> {
    ensure_distinct_paths(source, destination)?;
    let contents = fs::read(source)
        .with_context(|| format!("failed to read active settings from {}", source.display()))?;
    write_atomically(destination, &contents)?;
    Ok(destination.to_path_buf())
}

fn ensure_distinct_paths(source: &Path, destination: &Path) -> Result<()> {
    let source = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    let destination = destination
        .canonicalize()
        .unwrap_or_else(|_| destination.to_path_buf());
    if source == destination {
        bail!("source and destination are the same settings file");
    }
    Ok(())
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut temporary_file = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create a temporary file in {}", parent.display()))?;
    temporary_file
        .write_all(contents)
        .with_context(|| format!("failed to write temporary settings for {}", path.display()))?;
    temporary_file
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to sync temporary settings for {}", path.display()))?;
    temporary_file
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "settings_file_transfer_tests.rs"]
mod tests;
