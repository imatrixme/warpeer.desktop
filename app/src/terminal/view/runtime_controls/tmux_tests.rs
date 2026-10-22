use super::*;

#[test]
fn extracts_the_attached_session_from_generated_commands() {
    assert_eq!(
        attached_session_from_command(
            "tmux -S /private/tmp/tmux-501/default display-message -p -t %26 '#{pane_id}' >/dev/null && exec tmux -S /private/tmp/tmux-501/default select-window -t @22 ; select-pane -t %26 ; attach-session -t dst-22"
        )
        .as_deref(),
        Some("dst-22")
    );
}

#[test]
fn selects_the_most_recent_client_attached_to_the_source_session() {
    let output = b"/dev/ttys005\x1fdev\x1f100\x1f41\n/dev/ttys006\x1fother\x1f300\x1f42\n/dev/ttys007\x1fdev\x1f200\x1f43\n";

    assert_eq!(
        current_client(output, "dev", &HashMap::new()),
        Some(TmuxClient {
            name: "/dev/ttys007".to_string(),
            session: "dev".to_string(),
            activity: 200,
            process_id: 43,
        })
    );
}

#[test]
fn does_not_select_a_client_from_another_session() {
    let output = b"/dev/ttys005\x1fother\x1f100\x1f41\n";

    assert_eq!(current_client(output, "dev", &HashMap::new()), None);
}

#[test]
fn keeps_identifying_a_client_after_it_switches_sessions() {
    let output = b"/dev/ttys005\x1ftarget\x1f100\x1f41\n";
    let process_sessions = HashMap::from([(41, "source".to_string())]);

    assert_eq!(
        current_client(output, "source", &process_sessions),
        Some(TmuxClient {
            name: "/dev/ttys005".to_string(),
            session: "target".to_string(),
            activity: 100,
            process_id: 41,
        })
    );
}

#[test]
fn maps_tmux_client_processes_to_their_original_attach_session() {
    let output = b" 41 tmux attach-session -t source\n42 tmux -S /tmp/custom attach -ttarget\n";

    assert_eq!(
        parse_process_sessions(output),
        HashMap::from([(41, "source".to_string()), (42, "target".to_string()),])
    );
}
