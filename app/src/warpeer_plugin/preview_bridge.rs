use std::path::Path;
use std::sync::Weak;

use url::Url;
use warpeer_plugin_protocol::{
    BridgeError, BridgePayload, CapabilityRequest, TerminalHoverContext,
};
use warpeer_plugin_runtime::{FetchCancellation, ImagePreviewConfig, recognize_target};

use super::host::HostCore;

pub(super) fn load_preview(
    core: &Weak<HostCore>,
    request: &CapabilityRequest,
    context: &TerminalHoverContext,
    cancellation: &FetchCancellation,
) -> Result<BridgePayload, BridgeError> {
    let core = core.upgrade().ok_or(BridgeError::Cancelled)?;
    let target = request
        .payload
        .get("target")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BridgeError::InvalidRequest("target is required".into()))?;
    let expected = context
        .resolved_resource
        .as_deref()
        .unwrap_or(&context.resource_text);
    if target != expected {
        return Err(BridgeError::StaleContext);
    }
    let installed = core
        .manager
        .installed(&request.plugin_id)
        .ok()
        .flatten()
        .ok_or(BridgeError::NotFound)?;
    let mut config = serde_json::to_value(&installed.settings)
        .ok()
        .and_then(|settings| serde_json::from_value(settings).ok())
        .unwrap_or_else(ImagePreviewConfig::default);
    config.cache_bytes = config
        .cache_bytes
        .min(installed.manifest.budgets.cache_bytes)
        .min(installed.manifest.budgets.memory_bytes);
    recognize_target(context, &config)
        .ok_or_else(|| BridgeError::InvalidRequest("unsupported image target".into()))?;
    let (url, source_url) = if request.method.starts_with("resource.read") {
        local_urls(&core, request, context, target, &config)?
    } else {
        remote_urls(target)?
    };
    if cancellation.is_cancelled() {
        return Err(BridgeError::Cancelled);
    }
    Ok(BridgePayload::ImageResource {
        url,
        source_url,
        max_bytes: config.max_resource_bytes,
        max_width: u16::try_from(config.max_width.min(u16::MAX.into()))
            .map_err(|_| BridgeError::ResourceLimit("preview width exceeds u16".into()))?,
        max_height: u16::try_from(config.max_height.min(u16::MAX.into()))
            .map_err(|_| BridgeError::ResourceLimit("preview height exceeds u16".into()))?,
    })
}

fn local_urls(
    core: &HostCore,
    request: &CapabilityRequest,
    context: &TerminalHoverContext,
    target: &str,
    config: &ImagePreviewConfig,
) -> Result<(String, String), BridgeError> {
    let path = resolve_local_path(target, context)?;
    let handle = core.capabilities.issue_exact_read_handle(
        &request.plugin_id,
        &path,
        context.context_id,
        context.revision,
    )?;
    core.containers
        .attach_capability_handle(context.context_id, handle);
    let path = core.capabilities.resolve_exact_path(
        &request.plugin_id,
        handle,
        context.context_id,
        context.revision,
        config.max_resource_bytes,
    );
    core.capabilities.release(&request.plugin_id, handle);
    core.containers
        .release_capability_handle(&request.plugin_id, handle);
    let path = path?;
    let source_url = Url::from_file_path(path)
        .map_err(|_| BridgeError::InvalidRequest("local image path is not a file URL".into()))?;
    Ok((
        format!("warpeer-resource://preview/{}", request.request_id),
        source_url.into(),
    ))
}

fn remote_urls(target: &str) -> Result<(String, String), BridgeError> {
    let url = Url::parse(target).map_err(|_| BridgeError::InvalidRequest("invalid URL".into()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(BridgeError::InvalidRequest(
            "invalid HTTPS image URL".into(),
        ));
    }
    let url = url.to_string();
    Ok((url.clone(), url))
}

fn resolve_local_path(
    target: &str,
    context: &TerminalHoverContext,
) -> Result<std::path::PathBuf, BridgeError> {
    let path = Path::new(target);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    context
        .working_directory
        .as_deref()
        .map(Path::new)
        .map(|directory| directory.join(path))
        .ok_or_else(|| BridgeError::InvalidRequest("relative path has no working directory".into()))
}
