use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use async_trait::async_trait;

use crate::ai::agent::ReceivedMessageInput;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::server::server_api::ServerApi;
use crate::server::server_api::ai::{AIClient, AgentRunEvent, ReadAgentMessageResponse};

const UNAVAILABLE: &str = "hosted agent events are unavailable in this product profile";

#[derive(Clone, Debug)]
pub(crate) enum AgentEventFilter {
    RunIds(Vec<String>),
    AncestorRunId {
        ancestor_run_id: String,
        include_self: bool,
    },
}

impl AgentEventFilter {
    pub(crate) fn log_label(&self) -> String {
        match self {
            Self::RunIds(ids) => format!("run_ids={ids:?}"),
            Self::AncestorRunId {
                ancestor_run_id,
                include_self,
            } => format!("ancestor_run_id={ancestor_run_id} include_self={include_self}"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentEventDriverConfig {
    pub filter: AgentEventFilter,
    pub since_sequence: i64,
}

impl AgentEventDriverConfig {
    pub(crate) fn retry_forever(filter: AgentEventFilter, since_sequence: i64) -> Self {
        Self {
            filter,
            since_sequence,
        }
    }

    pub(crate) fn retry_forever_run_ids(run_ids: Vec<String>, since_sequence: i64) -> Self {
        Self::retry_forever(AgentEventFilter::RunIds(run_ids), since_sequence)
    }

    pub(crate) fn bounded_run_ids(run_ids: Vec<String>, since_sequence: i64) -> Self {
        Self::retry_forever_run_ids(run_ids, since_sequence)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentEventConsumerControlFlow {
    Continue,
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentEventDriverState {
    Connected,
    RetryScheduled {
        consecutive_failures: usize,
        backoff: Duration,
        is_initial_connect: bool,
    },
    ProactiveReconnect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentMessageEventMetadata {
    pub sequence: i64,
    pub message_id: String,
    pub occurred_at: String,
}

impl AgentMessageEventMetadata {
    pub(crate) fn from_event(event: &AgentRunEvent) -> Option<Self> {
        if event.event_type != "new_message" {
            return None;
        }

        Some(Self {
            sequence: event.sequence,
            message_id: event.ref_id.clone()?,
            occurred_at: event.occurred_at.clone(),
        })
    }
}

pub(crate) trait AgentEventSource: Send + Sync {}

pub(crate) struct ServerApiAgentEventSource;

impl ServerApiAgentEventSource {
    pub(crate) fn new(server_api: Arc<ServerApi>) -> Self {
        drop(server_api);
        Self
    }
}

impl AgentEventSource for ServerApiAgentEventSource {}

#[cfg_attr(target_family = "wasm", async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
pub(crate) trait AgentEventConsumer: Send {
    async fn on_event(&mut self, event: AgentRunEvent) -> Result<AgentEventConsumerControlFlow>;

    async fn persist_cursor(&mut self, sequence: i64) -> Result<()> {
        let _ = sequence;
        Ok(())
    }

    async fn on_driver_state(&mut self, state: AgentEventDriverState) -> Result<()> {
        drop(state);
        Ok(())
    }
}

pub(crate) async fn run_agent_event_driver<S, C>(
    source: S,
    config: AgentEventDriverConfig,
    consumer: &mut C,
) -> Result<()>
where
    S: AgentEventSource,
    C: AgentEventConsumer,
{
    drop(source);
    drop(config);
    let _ = consumer;
    Err(anyhow!(UNAVAILABLE))
}

#[derive(Clone, Default)]
pub(crate) struct MessageHydrator;

impl MessageHydrator {
    pub(crate) fn new(ai_client: Arc<dyn AIClient>) -> Self {
        drop(ai_client);
        Self
    }

    pub(crate) fn for_task(server_api: Arc<ServerApi>, task_id: AmbientAgentTaskId) -> Self {
        drop(server_api);
        let _ = task_id;
        Self
    }

    pub(crate) async fn hydrate_event_for_recipient(
        &self,
        event: &AgentRunEvent,
        recipient_run_id: &str,
    ) -> Option<ReceivedMessageInput> {
        let _ = event;
        let _ = recipient_run_id;
        None
    }

    pub(crate) async fn read_message_with_timeout(
        &self,
        message_id: &str,
    ) -> Result<ReadAgentMessageResponse> {
        let _ = message_id;
        Err(anyhow!(UNAVAILABLE))
    }

    pub(crate) async fn read_message_from_event_with_timeout(
        &self,
        event: &AgentRunEvent,
    ) -> Result<ReadAgentMessageResponse> {
        let _ = event;
        Err(anyhow!(UNAVAILABLE))
    }

    pub(crate) async fn mark_message_delivered(&self, message_id: &str) -> Result<()> {
        let _ = message_id;
        Err(anyhow!(UNAVAILABLE))
    }

    pub(crate) async fn mark_messages_delivered_best_effort<'a, I>(
        &self,
        message_ids: I,
    ) -> Vec<(String, anyhow::Error)>
    where
        I: IntoIterator<Item = &'a str>,
    {
        message_ids
            .into_iter()
            .map(|message_id| (message_id.to_string(), anyhow!(UNAVAILABLE)))
            .collect()
    }
}
