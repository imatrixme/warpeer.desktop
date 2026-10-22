use super::*;

fn history_entry(command: &str) -> Arc<HistoryEntry> {
    Arc::new(HistoryEntry::command_only(command))
}

#[test]
fn recognizes_direct_ssh_commands() {
    assert_eq!(
        normalized_ssh_command("ssh host"),
        Some("ssh host".to_string())
    );
    assert_eq!(
        normalized_ssh_command("KEY=value /usr/bin/ssh -p 2222 user@host"),
        Some("KEY=value /usr/bin/ssh -p 2222 user@host".to_string())
    );
    assert_eq!(
        normalized_ssh_command("command ssh 'work host'"),
        Some("command ssh 'work host'".to_string())
    );
}

#[test]
fn rejects_commands_that_do_not_start_ssh() {
    assert_eq!(normalized_ssh_command("ssh-keygen -t ed25519"), None);
    assert_eq!(normalized_ssh_command("echo ssh host"), None);
    assert_eq!(normalized_ssh_command("printf 'ssh host'"), None);
}

#[test]
fn recent_ssh_commands_are_deduplicated_newest_first() {
    let entries = vec![
        history_entry("ssh oldest"),
        history_entry("ssh duplicate"),
        history_entry("ls"),
        history_entry("  ssh duplicate  "),
        history_entry("ssh newest"),
    ];

    let commands = recent_ssh_commands(&entries)
        .into_iter()
        .map(|item| item.command)
        .collect::<Vec<_>>();

    assert_eq!(
        commands,
        vec![
            "ssh newest".to_string(),
            "ssh duplicate".to_string(),
            "ssh oldest".to_string(),
        ]
    );
}
