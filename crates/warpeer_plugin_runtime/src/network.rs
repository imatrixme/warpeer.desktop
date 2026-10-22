use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, LOCATION};
use reqwest::{StatusCode, Url};

const MAX_REDIRECTS: usize = 3;

#[derive(Clone, Copy, Debug)]
pub struct FetchPolicy {
    pub max_bytes: u64,
    pub timeout: Duration,
}

impl Default for FetchPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            timeout: Duration::from_secs(8),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FetchedResource {
    pub final_url: Url,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Default)]
pub struct FetchCancellation(Arc<AtomicBool>);

impl FetchCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub fn fetch_exact_https(target: &Url, policy: FetchPolicy) -> Result<FetchedResource, FetchError> {
    fetch_exact_https_cancellable(target, policy, &FetchCancellation::default())
}

pub fn fetch_exact_https_cancellable(
    target: &Url,
    policy: FetchPolicy,
    cancellation: &FetchCancellation,
) -> Result<FetchedResource, FetchError> {
    if target.scheme() != "https"
        || target.host_str().is_none()
        || target.username() != ""
        || target.password().is_some()
    {
        return Err(FetchError::InvalidTarget);
    }
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(policy.timeout)
        .user_agent("WarpeerPluginBridge/1")
        .build()
        .map_err(FetchError::Client)?;
    fetch_with_transport(target, cancellation, |current| {
        request_once(&client, current, policy, cancellation)
    })
}

enum FetchStep {
    Redirect(String),
    Resource(FetchedResource),
}

fn fetch_with_transport(
    target: &Url,
    cancellation: &FetchCancellation,
    mut request: impl FnMut(&Url) -> Result<FetchStep, FetchError>,
) -> Result<FetchedResource, FetchError> {
    let mut current = target.clone();
    for redirect_count in 0..=MAX_REDIRECTS {
        if cancellation.is_cancelled() {
            return Err(FetchError::Cancelled);
        }
        match request(&current)? {
            FetchStep::Redirect(location) => {
                if redirect_count == MAX_REDIRECTS {
                    return Err(FetchError::TooManyRedirects);
                }
                current = redirect_target(&current, &location)?;
            }
            FetchStep::Resource(resource) => return Ok(resource),
        }
    }
    Err(FetchError::TooManyRedirects)
}

fn request_once(
    client: &Client,
    current: &Url,
    policy: FetchPolicy,
    cancellation: &FetchCancellation,
) -> Result<FetchStep, FetchError> {
    let response = client
        .get(current.clone())
        .header(reqwest::header::ACCEPT, "image/png,image/jpeg")
        .send()
        .map_err(|error| {
            if error.is_timeout() {
                FetchError::Timeout
            } else {
                FetchError::Request(error)
            }
        })?;
    if response.status().is_redirection() {
        let location = response
            .headers()
            .get(LOCATION)
            .ok_or(FetchError::InvalidRedirect)?
            .to_str()
            .map_err(|_| FetchError::InvalidRedirect)?
            .to_owned();
        Ok(FetchStep::Redirect(location))
    } else {
        read_response(current.clone(), response, policy.max_bytes, cancellation)
            .map(FetchStep::Resource)
    }
}

fn redirect_target(base: &Url, location: &str) -> Result<Url, FetchError> {
    let target = base
        .join(location)
        .map_err(|_| FetchError::InvalidRedirect)?;
    if target.scheme() != "https" || target.username() != "" || target.password().is_some() {
        return Err(FetchError::InvalidRedirect);
    }
    Ok(target)
}

fn read_response(
    final_url: Url,
    mut response: Response,
    max_bytes: u64,
    cancellation: &FetchCancellation,
) -> Result<FetchedResource, FetchError> {
    if response.status() != StatusCode::OK {
        return Err(FetchError::Status(response.status()));
    }
    validate_content_length(
        response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok()),
        max_bytes,
    )?;
    let media_type = validate_media_type(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
    )?;
    let mut bytes = Vec::with_capacity((max_bytes.min(256 * 1024)) as usize);
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        if cancellation.is_cancelled() {
            return Err(FetchError::Cancelled);
        }
        let read = response.read(&mut buffer).map_err(FetchError::Read)?;
        if read == 0 {
            break;
        }
        if bytes.len().saturating_add(read) as u64 > max_bytes {
            return Err(FetchError::TooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Ok(FetchedResource {
        final_url,
        media_type,
        bytes,
    })
}

fn validate_content_length(length: Option<u64>, max_bytes: u64) -> Result<(), FetchError> {
    if length.is_some_and(|length| length > max_bytes) {
        Err(FetchError::TooLarge)
    } else {
        Ok(())
    }
}

fn validate_media_type(value: Option<&str>) -> Result<String, FetchError> {
    value
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| matches!(*value, "image/png" | "image/jpeg"))
        .map(str::to_owned)
        .ok_or(FetchError::UnsupportedMediaType)
}

#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("plugin HTTPS request was cancelled")]
    Cancelled,
    #[error("plugin network targets must be credential-free HTTPS URLs")]
    InvalidTarget,
    #[error("failed to construct isolated plugin HTTP client: {0}")]
    Client(#[source] reqwest::Error),
    #[error("plugin HTTPS request failed: {0}")]
    Request(#[source] reqwest::Error),
    #[error("plugin HTTPS request timed out")]
    Timeout,
    #[error("plugin HTTPS redirect is invalid")]
    InvalidRedirect,
    #[error("plugin HTTPS request exceeded the redirect limit")]
    TooManyRedirects,
    #[error("plugin HTTPS request returned {0}")]
    Status(StatusCode),
    #[error("plugin HTTPS response is not a supported image MIME type")]
    UnsupportedMediaType,
    #[error("plugin HTTPS response exceeds its byte limit")]
    TooLarge,
    #[error("failed to read plugin HTTPS response: {0}")]
    Read(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_and_credentialed_targets_before_network_access() {
        let policy = FetchPolicy::default();
        assert!(matches!(
            fetch_exact_https(&Url::parse("http://example.com/a.png").unwrap(), policy),
            Err(FetchError::InvalidTarget)
        ));
        assert!(matches!(
            fetch_exact_https(
                &Url::parse("https://user@example.com/a.png").unwrap(),
                policy
            ),
            Err(FetchError::InvalidTarget)
        ));
    }

    #[test]
    fn remote_policy_follows_https_redirects_and_returns_success() {
        let target = Url::parse("https://example.com/start.png").unwrap();
        let mut requests = Vec::new();
        let result = fetch_with_transport(&target, &FetchCancellation::default(), |url| {
            requests.push(url.path().to_owned());
            if url.path() == "/start.png" {
                Ok(FetchStep::Redirect("/final.png".into()))
            } else {
                Ok(FetchStep::Resource(FetchedResource {
                    final_url: url.clone(),
                    media_type: "image/png".into(),
                    bytes: vec![1, 2, 3],
                }))
            }
        })
        .unwrap();
        assert_eq!(requests, ["/start.png", "/final.png"]);
        assert_eq!(result.final_url.path(), "/final.png");
    }

    #[test]
    fn remote_policy_rejects_redirect_mime_size_and_timeout_failures() {
        let target = Url::parse("https://example.com/start.png").unwrap();
        assert!(matches!(
            redirect_target(&target, "http://example.com/final.png"),
            Err(FetchError::InvalidRedirect)
        ));
        assert!(matches!(
            validate_media_type(Some("text/html")),
            Err(FetchError::UnsupportedMediaType)
        ));
        assert!(matches!(
            validate_content_length(Some(9), 8),
            Err(FetchError::TooLarge)
        ));
        assert!(matches!(
            fetch_with_transport(&target, &FetchCancellation::default(), |_| {
                Err(FetchError::Timeout)
            }),
            Err(FetchError::Timeout)
        ));
    }
}
