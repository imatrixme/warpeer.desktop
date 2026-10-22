use std::sync::Arc;

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};
use crate::server::server_api::{AIApiError, ServerApi};
use crate::server::team_scope::RequestTeamScope;

pub async fn generate_multi_agent_output(
    _server_api: Arc<ServerApi>,
    _params: RequestParams,
    _team_scope: RequestTeamScope,
    _cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    let (tx, rx) = async_channel::unbounded();
    let _ = tx
        .send(Err(Arc::new(AIApiError::Other(anyhow::anyhow!(
            "Hosted AI is unavailable in this product profile"
        )))))
        .await;
    Ok(Box::pin(rx))
}
