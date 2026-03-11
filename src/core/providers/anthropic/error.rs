//! Anthropic Error Handling
//!
//! Error handling

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;

// Error
pub type AnthropicError = ProviderError;

/// Error
pub use crate::core::traits::error_mapper::implementations::AnthropicErrorMapper;

impl AnthropicErrorMapper {
    /// Error
    pub fn from_http_status(status: u16, body: &str) -> ProviderError {
        match status {
            400 => ProviderError::invalid_request("anthropic", format!("Bad request: {}", body)),
            401 => ProviderError::authentication("anthropic", "Invalid or missing API key"),
            403 => {
                ProviderError::authentication("anthropic", "Forbidden: insufficient permissions")
            }
            404 => ProviderError::model_not_found("anthropic", "Model or endpoint not found"),
            429 => {
                let retry_after = parse_retry_after_from_body(body);
                ProviderError::rate_limit("anthropic", retry_after)
            }
            500..=599 => {
                ProviderError::api_error("anthropic", status, format!("Server error: {}", body))
            }
            _ => HttpErrorMapper::map_status_code("anthropic", status, body),
        }
    }

    /// Response
    pub fn from_api_response(response: &serde_json::Value) -> ProviderError {
        // Error
        if let Some(error) = response.get("error") {
            let error_type = error
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");

            return match error_type {
                "authentication_error" => ProviderError::authentication("anthropic", message),
                "permission_error" => ProviderError::authentication("anthropic", message),
                "invalid_request_error" => ProviderError::invalid_request("anthropic", message),
                "not_found_error" => ProviderError::model_not_found("anthropic", message),
                "rate_limit_error" => {
                    let retry_after = error.get("retry_after").and_then(|r| r.as_u64());
                    ProviderError::RateLimit {
                        provider: "anthropic",
                        message: message.to_string(),
                        retry_after,
                        rpm_limit: None,
                        tpm_limit: None,
                        current_usage: None,
                    }
                }
                "overloaded_error" => {
                    ProviderError::provider_unavailable("anthropic", "Service overloaded")
                }
                "api_error" => ProviderError::api_error("anthropic", 500, message),
                _ => ProviderError::api_error(
                    "anthropic",
                    500,
                    format!("{}: {}", error_type, message),
                ),
            };
        }

        // Try top-level message
        if let Some(message) = response.get("message")
            && let Some(msg_str) = message.as_str()
        {
            return ProviderError::api_error("anthropic", 500, msg_str);
        }

        // Default error
        ProviderError::api_error("anthropic", 500, "Unknown API error")
    }
}

// Standard error helper functions
crate::define_provider_error_helpers!("anthropic", anthropic);

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_error_mapping() {
        let error = AnthropicErrorMapper::from_http_status(401, "Unauthorized");
        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "anthropic");
            }
            _ => panic!("Expected authentication error"),
        }
    }

    #[test]
    fn test_api_error_parsing() {
        let response = json!({
            "error": {
                "type": "authentication_error",
                "message": "Invalid API key"
            }
        });

        let error = AnthropicErrorMapper::from_api_response(&response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Invalid API key");
            }
            _ => panic!("Expected authentication error"),
        }
    }

    #[test]
    fn test_rate_limit_error() {
        let response = json!({
            "error": {
                "type": "rate_limit_error",
                "message": "Rate limit exceeded",
                "retry_after": 60
            }
        });

        let error = AnthropicErrorMapper::from_api_response(&response);
        match error {
            ProviderError::RateLimit {
                provider,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected rate limit error"),
        }
    }

    #[test]
    fn test_convenience_functions() {
        let config_err = anthropic_config_error("Test config error");
        match config_err {
            ProviderError::Configuration { provider, .. } => assert_eq!(provider, "anthropic"),
            _ => panic!("Expected configuration error"),
        }

        let auth_err = anthropic_auth_error("Test auth error");
        match auth_err {
            ProviderError::Authentication { provider, .. } => assert_eq!(provider, "anthropic"),
            _ => panic!("Expected authentication error"),
        }

        let api_err = anthropic_api_error(400, "Test API error");
        match api_err {
            ProviderError::ApiError {
                provider, status, ..
            } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(status, 400);
            }
            _ => panic!("Expected API error"),
        }
    }
}
