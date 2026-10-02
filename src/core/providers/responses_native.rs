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
