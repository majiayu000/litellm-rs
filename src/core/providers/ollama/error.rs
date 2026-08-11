//! Error types for Ollama provider.

use crate::core::providers::base::HttpErrorMapper;
pub use crate::core::providers::unified_provider::ProviderError;

/// Ollama error type (alias to unified ProviderError)
pub type OllamaError = ProviderError;

pub(super) fn parse_http_json_response(
    status: u16,
    body: &[u8],
) -> Result<serde_json::Value, ProviderError> {
    if !(200..300).contains(&status) {
        return Err(HttpErrorMapper::map_status_code(
            "ollama",
            status,
            &String::from_utf8_lossy(body),
        ));
    }
    serde_json::from_slice(body).map_err(|error| {
        ProviderError::api_error("ollama", 500, format!("Failed to parse response: {error}"))
    })
}

pub(super) fn parse_tool_arguments(raw: &str) -> Result<serde_json::Value, ProviderError> {
    let arguments: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
        ProviderError::invalid_request(
            "ollama",
            format!("tool call arguments must be valid JSON: {error}"),
        )
    })?;
    if !arguments.is_object() {
        return Err(ProviderError::invalid_request(
            "ollama",
            "tool call arguments must be a JSON object",
        ));
    }
    Ok(arguments)
}
