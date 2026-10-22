use super::*;

#[test]
fn claude_resume_always_names_the_exact_session() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::ClaudeCode,
        session_id: "session id".to_string(),
        worktree: PathBuf::from("/repo"),
    });

    assert_eq!(
        launch.command(ShellType::Zsh).unwrap(),
        "cd '/repo' && command -v claude >/dev/null 2>&1 && exec 'claude' '--resume' 'session id'; printf '%s\\n' 'Warpeer: claude is unavailable; exact session was not resumed.' >&2; false"
    );
    let WorktreeAgentLaunch::NativeResume(target) = launch else {
        unreachable!();
    };
    assert_eq!(target.argv().unwrap(), ["claude", "--resume", "session id"]);
}

#[test]
fn native_resume_uses_the_target_terminal_shell_quoting() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::ClaudeCode,
        session_id: "session 'quoted'".to_string(),
        worktree: PathBuf::from("/repo"),
    });

    assert_eq!(
        launch.command(ShellType::Fish).unwrap(),
        r"cd '/repo' && command -v claude >/dev/null 2>&1 && exec 'claude' '--resume' 'session \'quoted\''; printf '%s\n' 'Warpeer: claude is unavailable; exact session was not resumed.' >&2; false"
    );
}

#[test]
fn codex_resume_never_uses_last() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::Codex,
        session_id: "thread-1".to_string(),
        worktree: PathBuf::from("/repo"),
    });
    let command = launch.command(ShellType::Zsh).unwrap();

    assert_eq!(
        command,
        "cd '/repo' && command -v codex >/dev/null 2>&1 && exec 'codex' 'resume' 'thread-1'; printf '%s\\n' 'Warpeer: codex is unavailable; exact session was not resumed.' >&2; false"
    );
    assert!(!command.contains("--last"));
    let WorktreeAgentLaunch::NativeResume(target) = launch else {
        unreachable!();
    };
    assert_eq!(target.argv().unwrap(), ["codex", "resume", "thread-1"]);
}

#[test]
fn unsupported_provider_has_no_resume_command() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::Unknown,
        session_id: "one".to_string(),
        worktree: PathBuf::from("/repo"),
    });

    assert_eq!(launch.command(ShellType::Zsh), None);
}

#[test]
fn exact_native_resume_prefers_an_idle_current_pane() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::Codex,
        session_id: "thread-1".to_string(),
        worktree: PathBuf::from("/repo/worktree"),
    });

    assert!(launch.prefers_current_pane());
}

#[test]
fn exact_native_resume_changes_to_the_agent_worktree() {
    let launch = WorktreeAgentLaunch::NativeResume(NativeResumeTarget {
        provider: AgentProvider::Codex,
        session_id: "thread-1".to_string(),
        worktree: PathBuf::from("/repo/agent worktree"),
    });

    assert_eq!(
        launch.command(ShellType::Zsh).unwrap(),
        "cd '/repo/agent worktree' && command -v codex >/dev/null 2>&1 && exec 'codex' 'resume' 'thread-1'; printf '%s\\n' 'Warpeer: codex is unavailable; exact session was not resumed.' >&2; false"
    );
}
