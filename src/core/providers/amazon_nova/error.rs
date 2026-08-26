//! Amazon Nova Error Handling
//!
//! Error mapper and error handling for Amazon Nova provider

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use serde_json::Value;
use std::time::Duration;

/// Amazon Nova error mapper
#[derive(Debug, Clone, Default)]
pub struct AmazonNovaErrorMapper;

impl ErrorMapper<ProviderError> for AmazonNovaErrorMapper {
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            400 => ProviderError::invalid_request("amazon_nova", format!("Bad request: {}", body)),
            401 => {
                ProviderError::authentication("amazon_nova", format!("Invalid API key: {}", body))
            }
            // Upstream 403 is a permission failure; keep the status.
            403 => ProviderError::api_error(
                "amazon_nova",
                status,
                format!("Access forbidden: {}", body),
            ),
            404 => ProviderError::not_implemented(
                "amazon_nova",
                format!("Model or endpoint not found: {}", body),
            ),
            422 => {
                ProviderError::invalid_request("amazon_nova", format!("Validation error: {}", body))
            }
            429 => ProviderError::rate_limit_simple(
                "amazon_nova",
                format!("Rate limit exceeded: {}", body),
            ),
            500..=599 => {
                ProviderError::api_error("amazon_nova", status, format!("Server error: {}", body))
            }
            _ => HttpErrorMapper::map_status_code(
                "amazon_nova",
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

        ProviderError::response_parsing("amazon_nova", format!("JSON error: {}", error_msg))
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("amazon_nova", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: Duration) -> ProviderError {
        ProviderError::timeout(
            "amazon_nova",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_error_400() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(400, "Invalid model");
        assert!(
            format!("{:?}", error).contains("validation")
                || format!("{:?}", error).contains("Bad request")
        );
    }

    #[test]
    fn test_map_http_error_401() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");
        let debug = format!("{:?}", error).to_lowercase();
        assert!(debug.contains("authentication") || debug.contains("api key"));
    }

    #[test]
    fn test_map_http_error_403() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(403, "Forbidden");
        assert!(
            format!("{:?}", error).contains("authentication")
                || format!("{:?}", error).contains("Forbidden")
        );
    }

    #[test]
    fn test_map_http_error_404() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(404, "Not found");
        assert!(format!("{:?}", error).contains("not") || format!("{:?}", error).contains("found"));
    }

    #[test]
    fn test_map_http_error_429() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(429, "Too many requests");
        assert!(format!("{:?}", error).contains("rate") || format!("{:?}", error).contains("Rate"));
    }

    #[test]
    fn test_map_http_error_500() {
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_http_error(500, "Internal server error");
        assert!(format!("{:?}", error).contains("500"));
    }

    #[test]
    fn test_map_json_error() {
        let mapper = AmazonNovaErrorMapper;
        let error_response = serde_json::json!({
            "error": {
                "message": "Model not available"
            }
        });
        let error = mapper.map_json_error(&error_response);
        assert!(format!("{:?}", error).contains("Model not available"));
    }

    #[test]
    fn test_map_network_error() {
        let mapper = AmazonNovaErrorMapper;
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
        let mapper = AmazonNovaErrorMapper;
        let error = mapper.map_timeout_error(Duration::from_secs(60));
        assert!(
            format!("{:?}", error).contains("timeout") || format!("{:?}", error).contains("60")
        );
    }
}
