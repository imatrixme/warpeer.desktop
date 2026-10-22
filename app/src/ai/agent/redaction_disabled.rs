use super::super::blocklist::block::secret_redaction::{
    SECRET_REDACTION_REPLACEMENT_CHARACTER, find_secrets_in_text,
};

pub(crate) fn redact_secrets(input: &mut String) {
    let mut secrets: Vec<_> = find_secrets_in_text(input)
        .into_iter()
        .map(|range| range.byte_range)
        .collect();
    secrets.sort_by_key(|range| range.start);
    for range in secrets.into_iter().rev() {
        let replacement =
            SECRET_REDACTION_REPLACEMENT_CHARACTER.repeat(range.end.saturating_sub(range.start));
        input.replace_range(range.start..range.end, &replacement);
    }
}
