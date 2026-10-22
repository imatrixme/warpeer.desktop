#[path = "subscriptions/get_warp_drive_updates.rs"]
pub mod get_warp_drive_updates;

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use async_channel::Sender;
use cynic::{QueryFragment, QueryVariables, StreamingOperation as CynicStreamingOperation};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub async fn start_graphql_streaming_operation<Q, V, F, T>(
    _server_url: &str,
    _init_payload: HashMap<&str, String>,
    _operation: CynicStreamingOperation<Q, V>,
    _transform_stream_message: F,
    _message_sender: Sender<T>,
    _stream_ready_sender: Sender<()>,
    _handshake_headers: Vec<(&str, String)>,
) -> Result<()>
where
    Q: QueryFragment + DeserializeOwned + Unpin + Send + 'static,
    F: Fn(Option<Q>) -> Result<T> + Unpin + Send,
    V: QueryVariables + Serialize + Unpin + Send + 'static,
{
    Err(anyhow!(
        "GraphQL subscriptions are unavailable in this product profile"
    ))
}
