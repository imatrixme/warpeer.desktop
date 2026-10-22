use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use warp_cli::agent::Harness;
use warp_cli::{CliCommand, GlobalOptions};
use warpui::AppContext;

use crate::ai::agent::conversation::AIConversation;
use crate::ai::agent_events::AgentMessageEventMetadata;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::ambient_agents::task::HarnessModelConfig;
use crate::server::server_api::ServerApi;
use crate::server::server_api::ai::{AIClient, FileArtifactRecord, InitialSnapshotToken};
use crate::terminal::CLIAgent;

const HOSTED_AGENT_UNAVAILABLE: &str = "Hosted agent support is not compiled into this product.";

pub fn run(
    _ctx: &mut AppContext,
    _command: CliCommand,
    _global_options: GlobalOptions,
) -> anyhow::Result<()> {
    anyhow::bail!(HOSTED_AGENT_UNAVAILABLE)
}

pub fn is_running_in_warp() -> bool {
    false
}

pub(crate) use driver::harness::{ClaudeHarness, task_env_vars, validate_cli_installed};

pub(crate) mod artifact_upload {
    use super::*;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub(crate) struct FileArtifactUploadRequest {
        pub(crate) path: PathBuf,
        pub(crate) run_id: Option<AmbientAgentTaskId>,
        pub(crate) conversation_id: Option<crate::ai::agent::api::ServerConversationToken>,
        pub(crate) title: Option<String>,
        pub(crate) description: Option<String>,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct CompletedFileArtifactUpload {
        pub(crate) artifact: FileArtifactRecord,
        pub(crate) size_bytes: i64,
    }

    pub(crate) struct ResolvedUploadAssociation;

    pub(crate) struct FileArtifactUploader;

    impl FileArtifactUploader {
        pub(crate) fn new(_ai_client: Arc<dyn AIClient>, _server_api: Arc<ServerApi>) -> Self {
            Self
        }

        pub(crate) async fn resolve_upload_association(
            &self,
            _request: &FileArtifactUploadRequest,
        ) -> anyhow::Result<ResolvedUploadAssociation> {
            anyhow::bail!(HOSTED_AGENT_UNAVAILABLE)
        }

        pub(crate) async fn upload_with_association(
            &self,
            _request: FileArtifactUploadRequest,
            _association: ResolvedUploadAssociation,
        ) -> anyhow::Result<CompletedFileArtifactUpload> {
            anyhow::bail!(HOSTED_AGENT_UNAVAILABLE)
        }
    }
}

pub(crate) mod retry {
    pub(crate) use crate::server::retry_strategies::with_bounded_retry;
}

pub(crate) mod driver {
    use super::*;

    pub(crate) const AUTO_RESUME_TIMEOUT: Duration = Duration::ZERO;
    pub(crate) const OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV: &str =
        "OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY";

    #[derive(Debug, thiserror::Error)]
    pub enum AgentDriverError {
        #[error("{HOSTED_AGENT_UNAVAILABLE}")]
        Unavailable,
    }

    pub struct AgentDriver;

    pub(crate) async fn upload_snapshot_for_handoff(
        _repo_paths: Vec<PathBuf>,
        _orphan_file_paths: Vec<PathBuf>,
        _client: Arc<dyn AIClient>,
        _http: &http_client::Client,
    ) -> anyhow::Result<Option<InitialSnapshotToken>> {
        anyhow::bail!(HOSTED_AGENT_UNAVAILABLE)
    }

    pub(crate) mod harness {
        use super::*;

        const WARP_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV: &str =
            "WARP_MESSAGE_LISTENER_MANAGED_EXTERNALLY";
        const LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV: &str =
            "OZ_PARENT_LISTENER_MANAGED_EXTERNALLY";

        pub(crate) struct DisabledThirdPartyHarness {
            harness: Harness,
        }

        impl DisabledThirdPartyHarness {
            pub(crate) fn validate(&self) -> Result<(), AgentDriverError> {
                Err(AgentDriverError::Unavailable)
            }

            pub(crate) fn cli_agent(&self) -> CLIAgent {
                CLIAgent::from_harness(self.harness).unwrap_or(CLIAgent::Unknown)
            }
        }

        pub(crate) enum HarnessKind {
            Oz,
            ThirdParty(Box<DisabledThirdPartyHarness>),
            Unsupported(Harness),
        }

        pub(crate) fn harness_kind(harness: Harness) -> Result<HarnessKind, AgentDriverError> {
            match harness {
                Harness::Oz => Ok(HarnessKind::Oz),
                Harness::Claude | Harness::Codex | Harness::Gemini => Ok(HarnessKind::ThirdParty(
                    Box::new(DisabledThirdPartyHarness { harness }),
                )),
                Harness::OpenCode => Ok(HarnessKind::Unsupported(harness)),
                Harness::Unknown => Err(AgentDriverError::Unavailable),
            }
        }

        pub(crate) fn auth_check_command_for(_harness: Harness) -> Option<String> {
            None
        }

        pub(crate) fn validate_cli_installed(
            _cli: &str,
            _install_docs_url: Option<&str>,
        ) -> Result<(), AgentDriverError> {
            Err(AgentDriverError::Unavailable)
        }

        pub(crate) fn task_env_vars(
            _task_id: Option<&AmbientAgentTaskId>,
            _parent_run_id: Option<&str>,
            _selected_harness: Harness,
        ) -> HashMap<OsString, OsString> {
            HashMap::new()
        }

        pub(crate) fn harness_model_env_vars(
            selected_harness: Harness,
            model_config: Option<&HarnessModelConfig>,
        ) -> HashMap<OsString, OsString> {
            let mut env_vars = HashMap::new();
            if let (Harness::Claude, Some(model_config)) = (selected_harness, model_config)
                && !model_config.model_id.is_empty()
            {
                env_vars.insert(
                    OsString::from("ANTHROPIC_MODEL"),
                    OsString::from(&model_config.model_id),
                );
            }
            env_vars
        }

        pub(crate) fn remove_claude_externally_managed_listener_env_vars(
            env_vars: &mut HashMap<OsString, OsString>,
        ) {
            for key in [
                OZ_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV,
                WARP_MESSAGE_LISTENER_MANAGED_EXTERNALLY_ENV,
                LEGACY_OZ_PARENT_LISTENER_MANAGED_EXTERNALLY_ENV,
            ] {
                env_vars.remove(OsStr::new(key));
            }
        }

        pub(crate) struct ClaudeHarness;

        impl ClaudeHarness {
            pub(crate) async fn wake_dormant_session(
                _server_api: Arc<ServerApi>,
                _conversation: AIConversation,
                _parent_conversation: Option<AIConversation>,
                _working_dir: Option<PathBuf>,
                _wake_message: Option<AgentMessageEventMetadata>,
            ) -> anyhow::Result<Option<String>> {
                Ok(None)
            }
        }

        pub(crate) mod claude_code {
            use super::*;

            pub(crate) fn prepare_claude_environment_config(
                _working_dir: &Path,
                _resolved_env_vars: &HashMap<OsString, OsString>,
            ) -> anyhow::Result<()> {
                anyhow::bail!(HOSTED_AGENT_UNAVAILABLE)
            }
        }

        pub(crate) mod claude_transcript {
            pub(crate) struct ClaudeLocalContinuation {
                pub(crate) command: String,
            }

            pub(crate) fn rehydrate_claude_transcript_from_reader(
                _reader: impl std::io::Read,
            ) -> anyhow::Result<ClaudeLocalContinuation> {
                anyhow::bail!(super::HOSTED_AGENT_UNAVAILABLE)
            }
        }

        pub(crate) mod codex_transcript {
            pub(crate) struct CodexLocalContinuation {
                pub(crate) command: String,
            }

            pub(crate) fn rehydrate_codex_transcript_from_reader(
                _reader: impl std::io::Read,
            ) -> anyhow::Result<CodexLocalContinuation> {
                anyhow::bail!(super::HOSTED_AGENT_UNAVAILABLE)
            }
        }
    }
}
