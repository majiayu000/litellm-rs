use bytes::Bytes;
use serde_json::json;

use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;

/// Format a provider error into SSE error type and code for OpenAI-compatible responses.
pub(in crate::server::routes::ai) fn sse_error_classification(
    error: &ProviderError,
) -> (&'static str, &'static str) {
    match error {
        ProviderError::Authentication { .. } => ("invalid_request_error", "authentication_error"),
        ProviderError::RateLimit { .. } => ("rate_limit_error", "rate_limit_exceeded"),
        ProviderError::InvalidRequest { .. } => ("invalid_request_error", "invalid_request"),
        ProviderError::ModelNotFound { .. } => ("invalid_request_error", "model_not_found"),
        ProviderError::Timeout { .. } => ("server_error", "timeout"),
        ProviderError::ContentFiltered { .. } => ("invalid_request_error", "content_filter"),
        ProviderError::ContextLengthExceeded { .. } => {
            ("invalid_request_error", "context_length_exceeded")
        }
        ProviderError::TokenLimitExceeded { .. } => {
            ("invalid_request_error", "token_limit_exceeded")
        }
        _ => ("server_error", "internal_error"),
    }
}

/// Format an error as an SSE event matching OpenAI's streaming error format.
pub(super) fn format_sse_error(message: &str, error_type: &str, code: &str) -> Bytes {
    let error_json = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    let error_event = Event::default().data(&error_json.to_string());
    let done_event = Event::default().data("[DONE]");
    let mut combined = error_event.to_bytes().to_vec();
    combined.extend_from_slice(&done_event.to_bytes());
    Bytes::from(combined)
}
