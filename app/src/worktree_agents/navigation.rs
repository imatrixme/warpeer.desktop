use std::path::{Path, PathBuf};

use warpui::EntityId;

use super::identity::ExecutionHostKey;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeNavigationDecision {
    ExecuteInCurrent,
    FocusPane(EntityId),
    OpenLocalTerminal(PathBuf),
    BusyRemote,
    Stale,
}

pub(crate) fn decide(
    host: &ExecutionHostKey,
    target_path: &Path,
    existing_pane: Option<EntityId>,
    current_pane_is_safe: bool,
    target_exists: bool,
) -> WorktreeNavigationDecision {
    if !target_exists {
        return WorktreeNavigationDecision::Stale;
    }
    if let Some(terminal_view_id) = existing_pane {
        return WorktreeNavigationDecision::FocusPane(terminal_view_id);
    }
    if current_pane_is_safe {
        return WorktreeNavigationDecision::ExecuteInCurrent;
    }
    match host {
        ExecutionHostKey::Local => {
            WorktreeNavigationDecision::OpenLocalTerminal(target_path.to_path_buf())
        }
        ExecutionHostKey::Remote(_) => WorktreeNavigationDecision::BusyRemote,
    }
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
