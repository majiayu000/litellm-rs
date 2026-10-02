//! Native Responses HTTP transport. JSON and SSE bodies remain provider-native.

use std::time::Duration;

use serde_json::Value;

use super::ProviderError;
use super::base::{GlobalPoolManager, HeaderPair, HttpMethod, read_streaming_error_body};
use super::unified_provider::default_http_error_mapper;

pub(crate) async fn send(
    pool: &GlobalPoolManager,
    api_base: &str,
    headers: Vec<HeaderPair>,
    timeout_seconds: u64,
    body: Value,
    provider: &'static str,
) -> Result<reqwest::Response, ProviderError> {
    let url = format!("{}/responses", api_base.trim_end_matches('/'));
    let response = if body.get("stream").and_then(Value::as_bool) == Some(true) {
        tokio::time::timeout(
            Duration::from_secs(timeout_seconds),
            pool.execute_streaming_request_preserving_endpoint_policy(
                &url, headers, body, provider,
            ),
        )
        .await
        .map_err(|_| ProviderError::timeout(provider, "Responses upstream timed out"))??
    } else {
        pool.execute_request_preserving_endpoint_policy(&url, HttpMethod::POST, headers, Some(body))
            .await?
    };
    check_status(response, provider).await
}

pub(crate) async fn lifecycle(
    pool: &GlobalPoolManager,
    api_base: &str,
    headers: Vec<HeaderPair>,
    response_id: &str,
    method: HttpMethod,
    suffix: Option<&str>,
    query: &str,
) -> Result<reqwest::Response, ProviderError> {
    let mut url = url::Url::parse(api_base)
        .map_err(|_| ProviderError::invalid_request("responses", "Invalid upstream base URL"))?;
    {
        let mut path = url.path_segments_mut().map_err(|_| {
            ProviderError::invalid_request("responses", "Invalid upstream base URL")
        })?;
        path.pop_if_empty().push("responses").push(response_id);
        if let Some(suffix) = suffix {
            path.push(suffix);
        }
    }
    url.set_query((!query.is_empty()).then_some(query));
    let response = pool
        .execute_request_preserving_endpoint_policy(url.as_str(), method, headers, None)
        .await?;
    check_status(response, "responses").await
}

async fn check_status(
    response: reqwest::Response,
    provider: &'static str,
) -> Result<reqwest::Response, ProviderError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok());
    let body = read_streaming_error_body(response)
        .await
        .unwrap_or_else(|_| "failed to read Responses error body".to_string());
    if status.as_u16() == 429 && retry_after.is_some() {
        return Err(ProviderError::rate_limit_with_retry(
            provider,
            body,
            retry_after,
        ));
    }
    Err(default_http_error_mapper(provider, status.as_u16(), &body))
}
