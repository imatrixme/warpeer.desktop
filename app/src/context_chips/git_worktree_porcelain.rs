#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct GitWorktreeRecord {
    pub(crate) path: String,
    pub(crate) head: Option<String>,
    pub(crate) branch: Option<String>,
    pub(crate) detached: bool,
    pub(crate) bare: bool,
    pub(crate) locked: bool,
    pub(crate) lock_reason: Option<String>,
    pub(crate) prunable: bool,
    pub(crate) prune_reason: Option<String>,
}

pub(crate) fn parse_git_worktree_porcelain(raw: &str) -> Vec<GitWorktreeRecord> {
    let fields: Vec<&str> = if raw.contains('\0') {
        raw.split('\0').collect()
    } else {
        raw.lines().collect()
    };
    let mut records = Vec::new();
    let mut current: Option<GitWorktreeRecord> = None;

    for field in fields {
        let field = field.trim_end_matches('\r');
        if field.is_empty() {
            if let Some(record) = current.take() {
                records.push(record);
            }
            continue;
        }

        if let Some(path) = field.strip_prefix("worktree ") {
            if let Some(record) = current.take() {
                records.push(record);
            }
            current = Some(GitWorktreeRecord {
                path: decode_git_path(path),
                ..Default::default()
            });
            continue;
        }

        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some(head) = field.strip_prefix("HEAD ") {
            record.head = Some(head.to_string());
        } else if let Some(branch) = field.strip_prefix("branch ") {
            record.branch = Some(branch.to_string());
        } else if field == "detached" {
            record.detached = true;
        } else if field == "bare" {
            record.bare = true;
        } else if let Some(reason) = marker_reason(field, "locked") {
            record.locked = true;
            record.lock_reason = reason;
        } else if let Some(reason) = marker_reason(field, "prunable") {
            record.prunable = true;
            record.prune_reason = reason;
        }
    }

    if let Some(record) = current {
        records.push(record);
    }
    records
}

fn marker_reason(field: &str, marker: &str) -> Option<Option<String>> {
    if field == marker {
        return Some(None);
    }
    field
        .strip_prefix(marker)
        .and_then(|reason| reason.strip_prefix(' '))
        .map(|reason| (!reason.is_empty()).then(|| reason.to_string()))
}

fn decode_git_path(path: &str) -> String {
    let Some(quoted) = path
        .strip_prefix('"')
        .and_then(|path| path.strip_suffix('"'))
    else {
        return path.to_string();
    };
    let mut bytes = Vec::with_capacity(quoted.len());
    let mut characters = quoted.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\\' {
            let mut encoded = [0; 4];
            bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            continue;
        }

        let Some(escaped) = characters.next() else {
            bytes.push(b'\\');
            break;
        };
        match escaped {
            '\\' => bytes.push(b'\\'),
            '"' => bytes.push(b'"'),
            'n' => bytes.push(b'\n'),
            'r' => bytes.push(b'\r'),
            't' => bytes.push(b'\t'),
            'b' => bytes.push(8),
            '0'..='7' => {
                let mut octal = String::from(escaped);
                for _ in 0..2 {
                    if characters
                        .peek()
                        .is_some_and(|value| matches!(value, '0'..='7'))
                    {
                        octal.push(characters.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Ok(value) = u8::from_str_radix(&octal, 8) {
                    bytes.push(value);
                }
            }
            other => {
                let mut encoded = [0; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut encoded).as_bytes());
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
#[path = "git_worktree_porcelain_tests.rs"]
mod tests;
