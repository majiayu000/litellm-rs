use serde_json::Value;

use crate::ProviderError;
use crate::core::providers::vertex_ai::error::VertexAIError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// VertexAI-specific error mapper implementation
#[derive(Debug)]
pub struct VertexAIErrorMapper;

impl ErrorMapper<VertexAIError> for VertexAIErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> VertexAIError {
        match status_code {
            400 => ProviderError::response_parsing(
                "vertex_ai",
                format!("Bad request: {}", response_body),
            ),
            401 => ProviderError::authentication("vertex_ai", "Invalid credentials or API key"),
            403 => ProviderError::configuration(
                "vertex_ai",
                "Access forbidden: insufficient permissions",
            ),
            404 => ProviderError::model_not_found("vertex_ai", "Model not found"),
            429 => ProviderError::rate_limit("vertex_ai", None),
            500 => ProviderError::network("vertex_ai", "Internal server error"),
            502 => ProviderError::network("vertex_ai", "Bad gateway"),
            503 => ProviderError::network("vertex_ai", "Service unavailable"),
            _ => ProviderError::network(
                "vertex_ai",
                format!("HTTP error {}: {}", status_code, response_body),
            ),
        }
    }

    fn map_json_error(&self, error_response: &Value) -> VertexAIError {
        if let Some(error) = error_response.get("error") {
            let error_code = error.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let status = error
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN");

            match status {
                "INVALID_ARGUMENT" => ProviderError::response_parsing("vertex_ai", error_message),
                "UNAUTHENTICATED" => {
                    ProviderError::authentication("vertex_ai", "Authentication failed")
                }
                "PERMISSION_DENIED" => {
                    ProviderError::configuration("vertex_ai", "Permission denied")
                }
                "NOT_FOUND" => ProviderError::model_not_found("vertex_ai", error_message),
                "RESOURCE_EXHAUSTED" => ProviderError::rate_limit("vertex_ai", None),
                "INTERNAL" | "UNAVAILABLE" => ProviderError::network("vertex_ai", error_message),
                _ => ProviderError::network(
                    "vertex_ai",
                    format!("API Error ({}): {}", error_code, error_message),
                ),
            }
        } else {
            ProviderError::response_parsing("vertex_ai", "Unknown error response format")
        }
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> VertexAIError {
        ProviderError::network("vertex_ai", format!("Network error: {}", error))
    }
}
