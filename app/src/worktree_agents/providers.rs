use std::cmp::Ordering;

use crate::terminal::CLIAgent;

#[path = "providers/claude.rs"]
pub(crate) mod claude;
#[path = "providers/codex.rs"]
pub(crate) mod codex;
#[path = "providers/codex_rollout.rs"]
mod codex_rollout;
#[path = "providers/herdr.rs"]
pub(crate) mod herdr;
#[path = "providers/tmux.rs"]
pub(crate) mod tmux;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum AgentProvider {
    ClaudeCode,
    Codex,
    Herdr,
    Cli(CLIAgent),
    Unknown,
}

impl AgentProvider {
    pub(crate) fn from_cli_agent(agent: CLIAgent) -> Self {
        match agent {
            CLIAgent::Claude => Self::ClaudeCode,
            CLIAgent::Codex => Self::Codex,
            CLIAgent::Unknown => Self::Unknown,
            agent => Self::Cli(agent),
        }
    }

    pub(crate) fn from_name(name: &str) -> Self {
        let normalized = name.to_ascii_lowercase();
        if normalized.contains("herdr") {
            return Self::Herdr;
        }
        enum_iterator::all::<CLIAgent>()
            .filter(|agent| *agent != CLIAgent::Unknown)
            .find(|agent| {
                normalized.contains(&agent.display_name().to_ascii_lowercase())
                    || agent.command_prefixes().iter().any(|prefix| {
                        normalized
                            .split(|character: char| !character.is_ascii_alphanumeric())
                            .any(|token| token == *prefix)
                    })
            })
            .map(Self::from_cli_agent)
            .unwrap_or(Self::Unknown)
    }

    pub(crate) fn from_command(command: &str) -> Option<Self> {
        command.split_whitespace().find_map(|argument| {
            let executable = argument
                .rsplit(['/', '\\'])
                .next()?
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric() && !matches!(character, '-' | '.')
                })
                .to_ascii_lowercase();
            match executable.as_str() {
                "claude-code" => return Some(Self::ClaudeCode),
                "codex.js" => return Some(Self::Codex),
                _ => {}
            }
            enum_iterator::all::<CLIAgent>()
                .filter(|agent| *agent != CLIAgent::Unknown)
                .find(|agent| agent.command_prefixes().contains(&executable.as_str()))
                .map(Self::from_cli_agent)
        })
    }

    pub(crate) fn cli_agent(self) -> Option<CLIAgent> {
        match self {
            Self::ClaudeCode => Some(CLIAgent::Claude),
            Self::Codex => Some(CLIAgent::Codex),
            Self::Cli(agent) => Some(agent),
            Self::Herdr | Self::Unknown => None,
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::ClaudeCode => CLIAgent::Claude.display_name(),
            Self::Codex => CLIAgent::Codex.display_name(),
            Self::Herdr => "Herdr",
            Self::Cli(agent) => agent.display_name(),
            Self::Unknown => "Agent",
        }
    }

    fn sort_key(self) -> (u8, &'static str) {
        match self {
            Self::ClaudeCode => (0, self.display_name()),
            Self::Codex => (1, self.display_name()),
            Self::Herdr => (2, self.display_name()),
            Self::Cli(_) => (3, self.display_name()),
            Self::Unknown => (4, self.display_name()),
        }
    }
}

impl Ord for AgentProvider {
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for AgentProvider {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
