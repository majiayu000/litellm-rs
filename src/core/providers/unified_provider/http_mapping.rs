use super::ProviderError;

/// Default HTTP status-code → `ProviderError` mapping shared by most providers.
///
/// Providers with custom handling (e.g. Gemini, LangGraph, Databricks) should
/// implement `map_http_error` manually and call this for the status codes they
/// don't need to override.
pub fn default_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => {
            let message = parse_error_message_from_body(response_body)
                .unwrap_or_else(|| response_body.to_string());
            ProviderError::invalid_request(provider, message)
        }
        401 => ProviderError::authentication(provider, "Invalid API key"),
        403 => ProviderError::authentication(provider, "Permission denied"),
        404 => ProviderError::model_not_found(provider, "Model not found"),
        429 => {
            let retry_after =
                crate::core::providers::shared::parse_retry_after_from_body(response_body);
            ProviderError::rate_limit(provider, retry_after)
        }
        500..=599 => ProviderError::api_error(provider, status_code, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}

/// Try to extract an error message from a JSON response body.
///
/// Checks `error.message` and top-level `message` fields.
pub fn parse_error_message_from_body(response_body: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(response_body).ok()?;
    json.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .or_else(|| json.get("message").and_then(|m| m.as_str()))
        .map(|s| s.to_string())
}

/// Extended HTTP status-code → `ProviderError` mapping with additional codes.
///
/// Handles 402 (quota), 413 (context length), 408/504 (timeout), 502/503
/// (provider unavailable) in addition to the standard codes.
pub fn extended_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => ProviderError::invalid_request(provider, response_body),
        401 | 403 => ProviderError::authentication(provider, response_body),
        402 => ProviderError::quota_exceeded(provider, response_body),
        404 => ProviderError::model_not_found(provider, response_body),
        408 | 504 => ProviderError::timeout(provider, response_body),
        413 => ProviderError::context_length_exceeded(provider, 0, 0),
        429 => ProviderError::rate_limit(provider, None),
        500 => ProviderError::api_error(provider, status_code, response_body),
        502 | 503 => ProviderError::provider_unavailable(provider, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}
