use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context_chips::context_chip::{
    ChipFingerprintInput, ChipRuntimePolicy, ContextChip, RefreshConfig, ShellCommand,
    ShellCommandGenerator,
};
use crate::context_chips::git_worktree_porcelain::{
    GitWorktreeRecord, parse_git_worktree_porcelain,
};
use crate::context_chips::{ChipValue, ContextChipKind};
use crate::terminal::shell::ShellType;

const CURRENT_PREFIX: &str = "warpeer-current ";
const BRANCH_PREFIX: &str = "refs/heads/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WorktreeMenuEntry {
    pub(crate) common_dir: String,
    pub(crate) path: String,
    pub(crate) branch: Option<String>,
    pub(crate) head: Option<String>,
    pub(crate) is_current: bool,
    pub(crate) detached: bool,
    pub(crate) locked: bool,
    pub(crate) prunable: bool,
    #[serde(default)]
    pub(crate) dirty: WorktreeDirtyState,
    #[serde(default)]
    pub(crate) agent_count: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum WorktreeDirtyState {
    #[default]
    Unknown,
    Clean,
    Dirty,
    Unavailable,
}

impl WorktreeMenuEntry {
    pub(crate) fn compact_name(&self) -> String {
        compact_path(&self.path).to_string()
    }

    pub(crate) fn checkout_type_label(&self) -> &'static str {
        if self.is_primary_checkout() {
            "Primary"
        } else {
            "Worktree"
        }
    }

    pub(crate) fn git_state_label(&self) -> String {
        self.branch.clone().unwrap_or_else(|| {
            format!(
                "Detached {}",
                self.head.as_deref().map(short_head).unwrap_or("unknown")
            )
        })
    }

    pub(crate) fn is_primary_checkout(&self) -> bool {
        Path::new(&self.common_dir)
            .parent()
            .is_some_and(|parent| parent == Path::new(&self.path))
    }

    #[cfg(test)]
    pub(crate) fn display_name(&self) -> String {
        let identity = self
            .branch
            .as_deref()
            .or_else(|| self.head.as_deref().map(short_head))
            .unwrap_or("worktree");
        let state = if self.is_current {
            "current"
        } else if self.locked {
            "locked"
        } else if self.prunable {
            "unavailable"
        } else if self.detached {
            "detached"
        } else {
            "linked"
        };
        let dirty = match self.dirty {
            WorktreeDirtyState::Unknown => None,
            WorktreeDirtyState::Clean => Some("clean"),
            WorktreeDirtyState::Dirty => Some("modified"),
            WorktreeDirtyState::Unavailable => Some("status unavailable"),
        };
        let agents = (self.agent_count > 0).then(|| {
            if self.agent_count == 1 {
                "1 agent".to_string()
            } else {
                format!("{} agents", self.agent_count)
            }
        });
        [
            identity.to_string(),
            compact_path(&self.path).to_string(),
            state.to_string(),
        ]
        .into_iter()
        .chain(dirty.map(str::to_string))
        .chain(agents)
        .collect::<Vec<_>>()
        .join("  ·  ")
    }

    pub(crate) fn encode(&self) -> Option<String> {
        serde_json::to_string(self).ok()
    }

    pub(crate) fn decode(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }

    pub(crate) fn can_navigate(&self) -> bool {
        !self.is_current && !self.prunable
    }
}

pub(crate) fn context_chip() -> ContextChip {
    let generator = shell_worktree_list();
    let runtime_policy = ChipRuntimePolicy::new(
        ["git"],
        false,
        None,
        [
            ChipFingerprintInput::SessionId,
            ChipFingerprintInput::SessionIsLocal,
            ChipFingerprintInput::WorkingDirectory,
        ],
    )
    .with_hide_when_unavailable();
    ContextChip::shell_builtin_with_runtime_policy(
        "Worktrees",
        generator.clone(),
        Some(generator),
        RefreshConfig::OnDemandOnly,
        runtime_policy,
    )
}

pub(crate) fn prepend_prompt_chip(chips: &mut Vec<ContextChipKind>) {
    if !crate::product_profile::allows_worktree_agent_control_center()
        || chips.contains(&ContextChipKind::Worktrees)
    {
        return;
    }
    let index = chips
        .iter()
        .position(|chip| *chip == ContextChipKind::Tty)
        .map_or(0, |index| index + 1);
    chips.insert(index, ContextChipKind::Worktrees);
}

pub(crate) fn normalize_chip_value(value: Option<ChipValue>) -> Option<ChipValue> {
    let ChipValue::Text(raw) = value? else {
        return None;
    };
    (parse_menu_entries(&raw).len() > 1).then(|| ChipValue::Text("worktrees".to_string()))
}

pub(crate) fn filter_on_click_values(values: Option<Vec<String>>) -> Option<Vec<String>> {
    let raw = values?.join("\n");
    Some(
        parse_menu_entries(&raw)
            .into_iter()
            .filter_map(|entry| entry.encode())
            .collect(),
    )
}

pub(crate) fn decode_menu_entries(values: &[String]) -> Vec<WorktreeMenuEntry> {
    values
        .iter()
        .filter_map(|value| WorktreeMenuEntry::decode(value))
        .collect()
}

fn shell_worktree_list() -> ShellCommandGenerator {
    const SH_COMMAND: &str = "root=$(git --no-optional-locks rev-parse --show-toplevel 2>/dev/null) || exit 1; \
        common=$(git --no-optional-locks rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || exit 1; \
        printf 'warpeer-current %s\\0warpeer-common %s\\0' \"$root\" \"$common\"; \
        git --no-optional-locks worktree list --porcelain -z";
    const POWERSHELL_COMMAND: &str = "$root = git --no-optional-locks rev-parse --show-toplevel 2>$null; \
        if ($LASTEXITCODE -ne 0 -or -not $root) { exit 1 }; \
        $common = git --no-optional-locks rev-parse --path-format=absolute --git-common-dir 2>$null; \
        if ($LASTEXITCODE -ne 0 -or -not $common) { exit 1 }; \
        Write-Output \"warpeer-current $root\"; \
        Write-Output \"warpeer-common $common\"; \
        git --no-optional-locks worktree list --porcelain";
    ShellCommandGenerator::new(
        ShellCommand::shell_specific([
            (ShellType::PowerShell, POWERSHELL_COMMAND.to_string()),
            (ShellType::Bash, SH_COMMAND.to_string()),
            (ShellType::Zsh, SH_COMMAND.to_string()),
            (ShellType::Fish, SH_COMMAND.to_string()),
        ]),
        Some(vec!["git".to_string()]),
    )
}

fn parse_menu_entries(raw: &str) -> Vec<WorktreeMenuEntry> {
    let (current_path, common_dir, porcelain) = split_headers(raw);
    let mut entries: Vec<_> = parse_git_worktree_porcelain(&porcelain)
        .into_iter()
        .map(|record| menu_entry(record, current_path.as_deref(), common_dir.as_deref()))
        .collect();
    entries.sort_by_key(|entry| !entry.is_current);
    entries
}

fn split_headers(raw: &str) -> (Option<String>, Option<String>, String) {
    let separator = if raw.contains('\0') { '\0' } else { '\n' };
    let mut fields = raw.split(separator);
    let current_path = fields
        .next()
        .and_then(|header| header.strip_prefix(CURRENT_PREFIX))
        .map(str::to_string);
    let common_dir = fields
        .next()
        .and_then(|header| header.strip_prefix("warpeer-common "))
        .map(str::to_string);
    let remainder = fields.collect::<Vec<_>>().join(&separator.to_string());
    (current_path, common_dir, remainder)
}

fn menu_entry(
    record: GitWorktreeRecord,
    current_path: Option<&str>,
    common_dir: Option<&str>,
) -> WorktreeMenuEntry {
    let branch = record
        .branch
        .as_deref()
        .and_then(|branch| branch.strip_prefix(BRANCH_PREFIX))
        .map(str::to_string);
    WorktreeMenuEntry {
        common_dir: common_dir.unwrap_or_default().to_string(),
        is_current: current_path == Some(record.path.as_str()),
        path: record.path,
        branch,
        head: record.head,
        detached: record.detached,
        locked: record.locked,
        prunable: record.prunable,
        dirty: WorktreeDirtyState::Unknown,
        agent_count: 0,
    }
}

fn short_head(head: &str) -> &str {
    head.get(..8).unwrap_or(head)
}

fn compact_path(path: &str) -> &str {
    path.rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
