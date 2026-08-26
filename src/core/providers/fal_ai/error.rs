//! Fal AI Error Handling
//!
//! Error mapper and error handling for Fal AI provider

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use serde_json::Value;
use std::time::Duration;

/// Fal AI error mapper
#[derive(Debug, Clone, Default)]
pub struct FalAIErrorMapper;

impl ErrorMapper<ProviderError> for FalAIErrorMapper {
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 => ProviderError::authentication("fal_ai", format!("Invalid API key: {}", body)),
            403 => {
                ProviderError::api_error("fal_ai", status, format!("Access forbidden: {}", body))
            }
            404 => ProviderError::not_implemented(
                "fal_ai",
                format!("Model or endpoint not found: {}", body),
            ),
            422 => ProviderError::invalid_request("fal_ai", format!("Validation error: {}", body)),
            429 => {
                ProviderError::rate_limit_simple("fal_ai", format!("Rate limit exceeded: {}", body))
            }
            500..=599 => {
                ProviderError::api_error("fal_ai", status, format!("Server error: {}", body))
            }
            _ => HttpErrorMapper::map_status_code(
                "fal_ai",
                status,
                &format!("HTTP error {}: {}", status, body),
            ),
        }
    }

    fn map_json_error(&self, error_response: &Value) -> ProviderError {
        let error_msg = error_response
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| error_response.get("message").and_then(|m| m.as_str()))
            .unwrap_or("Unknown error");

        ProviderError::response_parsing("fal_ai", format!("JSON error: {}", error_msg))
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("fal_ai", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: Duration) -> ProviderError {
        ProviderError::timeout(
            "fal_ai",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_error_401() {
        let mapper = FalAIErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");
        let debug = format!("{:?}", error).to_lowercase();
        assert!(debug.contains("authentication") || debug.contains("api key"));
    }

    #[test]
    fn test_map_http_error_403() {
        let error = FalAIErrorMapper.map_http_error(403, "model access denied");
        assert!(matches!(
            error,
            ProviderError::ApiError {
                status: 403,
                ref message,
                ..
            } if message.contains("model access denied")
        ));
    }

    #[test]
    fn test_map_http_error_429() {
        let mapper = FalAIErrorMapper;
        let error = mapper.map_http_error(429, "Too many requests");
        assert!(
            format!("{:?}", error).contains("rate_limit")
                || format!("{:?}", error).contains("RateLimit")
        );
    }

    #[test]
    fn test_map_http_error_500() {
        let mapper = FalAIErrorMapper;
        let error = mapper.map_http_error(500, "Internal server error");
        assert!(format!("{:?}", error).contains("500"));
    }

    #[test]
    fn test_map_json_error() {
        let mapper = FalAIErrorMapper;
        let error_response = serde_json::json!({
            "error": {
                "message": "Invalid model"
            }
        });
        let error = mapper.map_json_error(&error_response);
        assert!(format!("{:?}", error).contains("Invalid model"));
    }

    #[test]
    fn test_map_network_error() {
        let mapper = FalAIErrorMapper;
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let error = mapper.map_network_error(&io_error);
        assert!(
            format!("{:?}", error).contains("network")
                || format!("{:?}", error).contains("Connection")
        );
    }

    #[test]
    fn test_map_timeout_error() {
        let mapper = FalAIErrorMapper;
        let error = mapper.map_timeout_error(Duration::from_secs(30));
        assert!(
            format!("{:?}", error).contains("timeout") || format!("{:?}", error).contains("30")
        );
    }
}
