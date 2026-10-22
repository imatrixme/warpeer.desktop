use std::time::Duration;

use warpeer_plugin_protocol::DiagnosticLevel;
use warpui::{AppContext, SingletonEntity as _};

use crate::menu::{Menu, MenuItem, MenuItemFields, MenuVariant};
use crate::ui_components::icons::Icon;
use crate::warpeer_plugin::{PluginDiagnosticRecord, WarpeerPluginHost};
use crate::workspace::WorkspaceAction;

pub(super) const POSITION_ID: &str = "workspace:plugin_diagnostics";
const PANEL_WIDTH: f32 = 460.;
const PANEL_HEIGHT: f32 = 420.;
const MAX_VISIBLE_RECORDS: usize = 80;

pub(super) fn menu() -> Menu<WorkspaceAction> {
    let mut menu = Menu::new()
        .with_width(PANEL_WIDTH)
        .with_menu_variant(MenuVariant::scrollable())
        .with_drop_shadow()
        .prevent_interaction_with_other_elements();
    menu.set_height(PANEL_HEIGHT);
    menu
}

pub(super) fn items(app: &AppContext) -> Vec<MenuItem<WorkspaceAction>> {
    let diagnostic_text = plain_text(app);
    let mut items = vec![
        MenuItemFields::new("Plugin Diagnostics")
            .with_icon(Icon::Bug)
            .with_disabled(true)
            .into_item(),
        MenuItemFields::new("Copy diagnostics")
            .with_icon(Icon::Copy)
            .with_on_select_action(WorkspaceAction::CopyTextToClipboard(diagnostic_text))
            .into_item(),
        MenuItemFields::new("Clear diagnostics")
            .with_icon(Icon::Trash)
            .with_on_select_action(WorkspaceAction::ClearPluginDiagnostics)
            .into_item(),
        MenuItemFields::new("Close")
            .with_icon(Icon::X)
            .with_on_select_action(WorkspaceAction::ClosePluginDiagnostics)
            .into_item(),
        MenuItem::Separator,
    ];

    let records = WarpeerPluginHost::as_ref(app).diagnostic_records();
    if records.is_empty() {
        items.push(
            MenuItemFields::new_with_stacked_label(
                "No plugin events yet",
                "Hover or preview an image link to trace the runtime path.",
            )
            .with_disabled(true)
            .into_item(),
        );
        return items;
    }

    items.extend(
        records
            .iter()
            .rev()
            .take(MAX_VISIBLE_RECORDS)
            .map(record_item),
    );
    items
}

pub(super) fn plain_text(app: &AppContext) -> String {
    format_records(&WarpeerPluginHost::as_ref(app).diagnostic_records())
}

fn format_records(records: &[PluginDiagnosticRecord]) -> String {
    let mut lines = vec!["Warpeer Plugin Diagnostics".to_string()];
    if records.is_empty() {
        lines.push("No plugin diagnostic events.".to_string());
        return lines.join("\n");
    }

    lines.push(format!("{} events, oldest first", records.len()));
    lines.push(String::new());
    lines.extend(records.iter().map(|record| {
        format!(
            "[{}] {} · {} · {}",
            level_label(&record.level),
            record.plugin_id,
            elapsed_label(record.timestamp.elapsed()),
            record.message
        )
    }));
    lines.join("\n")
}

fn record_item(record: &PluginDiagnosticRecord) -> MenuItem<WorkspaceAction> {
    let title = format!("{} · {}", level_label(&record.level), record.plugin_id);
    let subtitle = format!(
        "{} · {}",
        elapsed_label(record.timestamp.elapsed()),
        record.message
    );
    MenuItemFields::new_with_stacked_label(title, subtitle)
        .with_disabled(true)
        .into_item()
}

fn level_label(level: &DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "Error",
        DiagnosticLevel::Warning => "Warning",
        DiagnosticLevel::Info => "Info",
        DiagnosticLevel::Debug => "Debug",
    }
}

fn elapsed_label(elapsed: Duration) -> String {
    if elapsed < Duration::from_secs(1) {
        "now".into()
    } else if elapsed < Duration::from_secs(60) {
        format!("{}s ago", elapsed.as_secs())
    } else {
        format!("{}m ago", elapsed.as_secs() / 60)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use warpeer_plugin_protocol::{DiagnosticLevel, PluginId};

    use super::{PluginDiagnosticRecord, format_records};

    #[test]
    fn copied_diagnostics_are_chronological_plain_text() {
        let now = Instant::now();
        let records = vec![
            PluginDiagnosticRecord {
                plugin_id: PluginId::parse("dev.warpeer.first").unwrap(),
                level: DiagnosticLevel::Info,
                message: "worker started".into(),
                timestamp: now - Duration::from_secs(12),
            },
            PluginDiagnosticRecord {
                plugin_id: PluginId::parse("dev.warpeer.second").unwrap(),
                level: DiagnosticLevel::Warning,
                message: "frame rejected".into(),
                timestamp: now - Duration::from_secs(4),
            },
        ];

        let text = format_records(&records);
        assert!(text.starts_with("Warpeer Plugin Diagnostics\n2 events, oldest first"));
        assert!(text.find("worker started").unwrap() < text.find("frame rejected").unwrap());
    }

    #[test]
    fn empty_diagnostics_copy_an_explicit_report() {
        assert_eq!(
            format_records(&[]),
            "Warpeer Plugin Diagnostics\nNo plugin diagnostic events."
        );
    }
}
