use std::collections::HashMap;
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::worktree_agents::observation::AgentLifecycle;

const MAX_ROLLOUT_READ_BYTES: u64 = 1024 * 1024;
const MAX_PARTIAL_LINE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CodexRolloutSummary {
    pub(super) lifecycle: Option<AgentLifecycle>,
    pub(super) title: Option<String>,
    pub(super) model_name: Option<String>,
    pub(super) token_usage: Option<u64>,
    pub(super) context_remaining_percent: Option<u8>,
    pub(super) activity_unix_ms: Option<u64>,
}

#[derive(Default)]
pub(super) struct CodexRolloutCache {
    entries: HashMap<PathBuf, CachedRollout>,
    #[cfg(test)]
    bytes_read: u64,
}

#[derive(Default)]
struct CachedRollout {
    file_len: u64,
    initialized: bool,
    summary: CodexRolloutSummary,
    pending: Vec<u8>,
    discarding_partial_line: bool,
}

impl CodexRolloutCache {
    pub(super) fn summarize(&mut self, path: &Path) -> Option<CodexRolloutSummary> {
        let length = path.metadata().ok()?.len();
        if let Some(entry) = self.entries.get(path)
            && entry.initialized
            && entry.file_len == length
        {
            return Some(entry.summary.clone());
        }
        let (start, reset, begins_mid_line) = self.entries.get(path).map_or_else(
            || {
                let start = length.saturating_sub(MAX_ROLLOUT_READ_BYTES);
                (start, true, start > 0)
            },
            |entry| {
                if length < entry.file_len {
                    let start = length.saturating_sub(MAX_ROLLOUT_READ_BYTES);
                    (start, true, start > 0)
                } else {
                    let start = entry
                        .file_len
                        .max(length.saturating_sub(MAX_ROLLOUT_READ_BYTES));
                    (start, false, start > entry.file_len)
                }
            },
        );
        let bytes = read_range(path, start, length)?;
        #[cfg(test)]
        {
            self.bytes_read = self.bytes_read.saturating_add(bytes.len() as u64);
        }
        let entry = self.entries.entry(path.to_path_buf()).or_default();
        if reset {
            *entry = CachedRollout::default();
        }
        entry.apply_chunk(&bytes, begins_mid_line);
        entry.file_len = length;
        entry.initialized = true;
        Some(entry.summary.clone())
    }

    #[cfg(test)]
    fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl CachedRollout {
    fn apply_chunk(&mut self, bytes: &[u8], begins_mid_line: bool) {
        if begins_mid_line {
            self.pending.clear();
            self.discarding_partial_line = true;
        }
        let bytes = if self.discarding_partial_line {
            let Some(index) = bytes.iter().position(|byte| *byte == b'\n') else {
                return;
            };
            self.discarding_partial_line = false;
            &bytes[index + 1..]
        } else {
            bytes
        };
        let mut combined = std::mem::take(&mut self.pending);
        combined.extend_from_slice(bytes);
        let Some(last_newline) = combined.iter().rposition(|byte| *byte == b'\n') else {
            self.retain_partial(combined);
            return;
        };
        apply_lines(&combined[..=last_newline], &mut self.summary);
        self.retain_partial(combined[last_newline + 1..].to_vec());
    }

    fn retain_partial(&mut self, partial: Vec<u8>) {
        if partial.len() <= MAX_PARTIAL_LINE_BYTES {
            self.pending = partial;
        } else {
            self.pending.clear();
            self.discarding_partial_line = true;
        }
    }
}

fn read_range(path: &Path, start: u64, length: u64) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((length - start) as usize);
    file.take(length - start).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn parse_tail(bytes: &[u8], begins_mid_file: bool) -> CodexRolloutSummary {
    let bytes = if begins_mid_file {
        bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(&[][..], |index| &bytes[index + 1..])
    } else {
        bytes
    };
    let mut summary = CodexRolloutSummary::default();
    apply_lines(bytes, &mut summary);
    summary
}

fn apply_lines(bytes: &[u8], summary: &mut CodexRolloutSummary) {
    for line in bytes.split(|byte| *byte == b'\n') {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp);
        if timestamp.is_some() {
            summary.activity_unix_ms = timestamp;
        }
        match value.get("type").and_then(Value::as_str) {
            Some("turn_context") => {
                summary.model_name = value
                    .pointer("/payload/model")
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            Some("event_msg") => apply_event(value.get("payload"), summary),
            Some(_) | None => {}
        }
    }
}

fn apply_event(payload: Option<&Value>, summary: &mut CodexRolloutSummary) {
    let Some(payload) = payload else {
        return;
    };
    match payload.get("type").and_then(Value::as_str) {
        Some("task_started") => summary.lifecycle = Some(AgentLifecycle::Working),
        Some("task_complete") => summary.lifecycle = Some(AgentLifecycle::ReadyForReview),
        Some("turn_aborted") => summary.lifecycle = Some(AgentLifecycle::Stopped),
        Some("stream_error") => summary.lifecycle = Some(AgentLifecycle::Failed),
        Some("item_started" | "item_completed") => {
            summary.lifecycle = Some(AgentLifecycle::Working)
        }
        Some("user_message") => {
            if summary.title.is_none() {
                summary.title = payload
                    .get("message")
                    .and_then(Value::as_str)
                    .and_then(bounded_title);
            }
        }
        Some("token_count") => apply_token_count(payload, summary),
        Some(_) | None => {}
    }
}

fn apply_token_count(payload: &Value, summary: &mut CodexRolloutSummary) {
    let usage = payload.pointer("/info/last_token_usage/total_tokens");
    summary.token_usage = usage.and_then(Value::as_u64);
    let context_window = payload
        .pointer("/info/model_context_window")
        .and_then(Value::as_u64);
    summary.context_remaining_percent = match (summary.token_usage, context_window) {
        (Some(used), Some(window)) if window > 0 => {
            let remaining = window.saturating_sub(used).saturating_mul(100) / window;
            Some(remaining.min(100) as u8)
        }
        _ => None,
    };
}

fn bounded_title(value: &str) -> Option<String> {
    let value: String = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect();
    (!value.is_empty()).then_some(value)
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let timestamp = DateTime::parse_from_rfc3339(value)
        .ok()?
        .with_timezone(&Utc)
        .timestamp_millis();
    u64::try_from(timestamp).ok()
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn completed_turn_is_waiting_for_user_and_keeps_only_bounded_metadata() {
        let bytes = br#"{"timestamp":"2026-08-29T01:00:00Z","type":"turn_context","payload":{"model":"gpt-5.6-sol"}}
{"timestamp":"2026-08-29T01:00:01Z","type":"event_msg","payload":{"type":"user_message","message":"Fix   the agent status"}}
{"timestamp":"2026-08-29T01:00:02Z","type":"event_msg","payload":{"type":"task_started"}}
{"timestamp":"2026-08-29T01:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":60000},"model_context_window":200000}}}
{"timestamp":"2026-08-29T01:00:04Z","type":"event_msg","payload":{"type":"task_complete"}}
"#;

        let summary = parse_tail(bytes, false);

        assert_eq!(summary.lifecycle, Some(AgentLifecycle::ReadyForReview));
        assert_eq!(summary.title.as_deref(), Some("Fix the agent status"));
        assert_eq!(summary.model_name.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(summary.token_usage, Some(60_000));
        assert_eq!(summary.context_remaining_percent, Some(70));
    }

    #[test]
    fn partial_first_line_is_discarded() {
        let bytes = br#"secret tail}
{"timestamp":"2026-08-29T01:00:02Z","type":"event_msg","payload":{"type":"task_started"}}
"#;

        assert_eq!(
            parse_tail(bytes, true).lifecycle,
            Some(AgentLifecycle::Working)
        );
    }

    #[test]
    fn progress_after_a_clipped_task_start_remains_working() {
        let bytes = br#"partial line}
{"timestamp":"2026-08-29T01:00:03Z","type":"event_msg","payload":{"type":"item_completed"}}
"#;

        assert_eq!(
            parse_tail(bytes, true).lifecycle,
            Some(AgentLifecycle::Working)
        );
    }

    #[test]
    fn task_complete_overrides_progress_events() {
        let bytes = br#"{"timestamp":"2026-08-29T01:00:03Z","type":"event_msg","payload":{"type":"item_completed"}}
{"timestamp":"2026-08-29T01:00:04Z","type":"event_msg","payload":{"type":"task_complete"}}
"#;

        assert_eq!(
            parse_tail(bytes, false).lifecycle,
            Some(AgentLifecycle::ReadyForReview)
        );
    }

    #[test]
    fn later_user_messages_do_not_replace_the_session_title() {
        let bytes =
            br#"{"type":"event_msg","payload":{"type":"user_message","message":"Original task"}}
{"type":"event_msg","payload":{"type":"user_message","message":"Follow-up instruction"}}
"#;

        assert_eq!(
            parse_tail(bytes, false).title.as_deref(),
            Some("Original task")
        );
    }

    #[test]
    fn incremental_cache_preserves_metadata_after_the_initial_tail_rolls_over() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-08-29T01:00:00Z","type":"turn_context","payload":{{"model":"gpt-5.6-sol"}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"timestamp":"2026-08-29T01:00:01Z","type":"event_msg","payload":{{"type":"user_message","message":"Stable title"}}}}"#
        )
        .unwrap();
        file.flush().unwrap();
        let mut cache = CodexRolloutCache::default();

        let first = cache.summarize(file.path()).unwrap();
        let first_read = cache.bytes_read();
        assert_eq!(first.title.as_deref(), Some("Stable title"));
        cache.summarize(file.path()).unwrap();
        assert_eq!(cache.bytes_read(), first_read);

        let padding = "x".repeat(220);
        for _ in 0..6_000 {
            writeln!(
                file,
                r#"{{"timestamp":"2026-08-29T01:00:02Z","type":"event_msg","payload":{{"type":"item_completed","padding":"{padding}"}}}}"#
            )
            .unwrap();
        }
        writeln!(
            file,
            r#"{{"timestamp":"2026-08-29T01:00:03Z","type":"event_msg","payload":{{"type":"token_count","info":{{"last_token_usage":{{"total_tokens":90000}},"model_context_window":200000}}}}}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let updated = cache.summarize(file.path()).unwrap();

        assert_eq!(updated.title.as_deref(), Some("Stable title"));
        assert_eq!(updated.model_name.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(updated.token_usage, Some(90_000));
        assert_eq!(updated.context_remaining_percent, Some(55));
        assert!(cache.bytes_read().saturating_sub(first_read) <= MAX_ROLLOUT_READ_BYTES);
    }
}
