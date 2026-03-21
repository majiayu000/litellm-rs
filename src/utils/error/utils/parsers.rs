use crate::core::providers::unified_provider::ProviderError;

use super::types::ErrorUtils;

impl ErrorUtils {
    pub fn parse_openai_error(response_body: &str) -> ProviderError {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body)
            && let Some(error) = json.get("error")
        {
            let error_type = error.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return match error_type {
                "invalid_request_error" => ProviderError::InvalidRequest {
                    provider: "openai",
                    message,
                },
                "authentication_error" => ProviderError::Authentication {
                    provider: "openai",
                    message,
                },
                "permission_error" => ProviderError::Authentication {
                    provider: "openai",
                    message,
                },
                "rate_limit_error" => {
                    ProviderError::rate_limit_with_retry("openai", message, Some(60))
                }
                "model_not_found_error" => ProviderError::ModelNotFound {
                    provider: "openai",
                    model: message,
                },
                "context_length_exceeded" => ProviderError::InvalidRequest {
                    provider: "openai",
                    message: format!("Context length exceeded: {}", message),
                },
                "timeout_error" => ProviderError::Timeout {
                    provider: "openai",
                    message,
                },
                "server_error" => ProviderError::ProviderUnavailable {
                    provider: "openai",
                    message,
                },
                _ => ProviderError::Other {
                    provider: "openai",
                    message,
                },
            };
        }

        ProviderError::Other {
            provider: "openai",
            message: response_body.to_string(),
        }
    }

    pub fn parse_anthropic_error(response_body: &str) -> ProviderError {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body)
            && let Some(error) = json.get("error")
        {
            let error_type = error.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return match error_type {
                "invalid_request_error" => ProviderError::InvalidRequest {
                    provider: "anthropic",
                    message,
                },
                "authentication_error" => ProviderError::Authentication {
                    provider: "anthropic",
                    message,
                },
                "permission_error" => ProviderError::Authentication {
                    provider: "anthropic",
                    message,
                },
                "rate_limit_error" => {
                    ProviderError::rate_limit_with_retry("anthropic", message, Some(60))
                }
                "not_found_error" => ProviderError::ModelNotFound {
                    provider: "anthropic",
                    model: message,
                },
                "overloaded_error" => ProviderError::ProviderUnavailable {
                    provider: "anthropic",
                    message,
                },
                _ => ProviderError::Other {
                    provider: "anthropic",
                    message,
                },
            };
        }

        ProviderError::Other {
            provider: "anthropic",
            message: response_body.to_string(),
        }
    }

    pub fn parse_google_error(response_body: &str) -> ProviderError {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body)
            && let Some(error) = json.get("error")
        {
            let status = error.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();

            return match status {
                "INVALID_ARGUMENT" => ProviderError::InvalidRequest {
                    provider: "google",
                    message,
                },
                "UNAUTHENTICATED" => ProviderError::Authentication {
                    provider: "google",
                    message,
                },
                "PERMISSION_DENIED" => ProviderError::Authentication {
                    provider: "google",
                    message,
                },
                "RESOURCE_EXHAUSTED" => ProviderError::rate_limit_simple("google", message),
                "NOT_FOUND" => ProviderError::ModelNotFound {
                    provider: "google",
                    model: message,
                },
                "INTERNAL" => ProviderError::Other {
                    provider: "google",
                    message,
                },
                "UNAVAILABLE" => ProviderError::ProviderUnavailable {
                    provider: "google",
                    message,
                },
                _ => ProviderError::Other {
                    provider: "google",
                    message,
                },
            };
        }

        ProviderError::Other {
            provider: "unknown",
            message: response_body.to_string(),
        }
    }

    pub fn parse_provider_error(
        provider: &'static str,
        status_code: u16,
        response_body: &str,
    ) -> ProviderError {
        match provider.to_lowercase().as_str() {
            "openai" => Self::parse_openai_error(response_body),
            "anthropic" => Self::parse_anthropic_error(response_body),
            "google" => Self::parse_google_error(response_body),
            _ => Self::map_http_status_to_error(
                provider,
                status_code,
                Some(response_body.to_string()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::unified_provider::ProviderError;

    #[test]
    fn test_parse_openai_error_invalid_request() {
        let response =
            r#"{"error": {"type": "invalid_request_error", "message": "Invalid model specified"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Invalid model specified");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_openai_error_authentication() {
        let response =
            r#"{"error": {"type": "authentication_error", "message": "Invalid API key"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Invalid API key");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_openai_error_permission() {
        let response =
            r#"{"error": {"type": "permission_error", "message": "Access denied to this model"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Access denied to this model");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_openai_error_rate_limit() {
        let response =
            r#"{"error": {"type": "rate_limit_error", "message": "Rate limit exceeded"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::RateLimit {
                provider,
                message,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Rate limit exceeded");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_parse_openai_error_model_not_found() {
        let response = r#"{"error": {"type": "model_not_found_error", "message": "gpt-unknown"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "openai");
                assert_eq!(model, "gpt-unknown");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_parse_openai_error_context_length() {
        let response = r#"{"error": {"type": "context_length_exceeded", "message": "Maximum context length is 4096 tokens"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "openai");
                assert!(message.contains("Context length exceeded"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_openai_error_timeout() {
        let response = r#"{"error": {"type": "timeout_error", "message": "Request timed out"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Timeout { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Request timed out");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_parse_openai_error_server_error() {
        let response = r#"{"error": {"type": "server_error", "message": "Internal server error"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Internal server error");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_parse_openai_error_unknown_type() {
        let response =
            r#"{"error": {"type": "unknown_error_type", "message": "Something went wrong"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_openai_error_no_message() {
        let response = r#"{"error": {"type": "invalid_request_error"}}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::InvalidRequest { message, .. } => {
                assert_eq!(message, "Unknown error");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_openai_error_invalid_json() {
        let response = "not valid json";
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "not valid json");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_openai_error_no_error_field() {
        let response = r#"{"status": "failed", "message": "Something went wrong"}"#;
        let error = ErrorUtils::parse_openai_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "openai");
                assert!(message.contains("status"));
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_invalid_request() {
        let response =
            r#"{"error": {"type": "invalid_request_error", "message": "Invalid parameter"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Invalid parameter");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_authentication() {
        let response =
            r#"{"error": {"type": "authentication_error", "message": "Invalid API key"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Invalid API key");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_permission() {
        let response =
            r#"{"error": {"type": "permission_error", "message": "Access not allowed"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Access not allowed");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_rate_limit() {
        let response = r#"{"error": {"type": "rate_limit_error", "message": "Too many requests"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::RateLimit {
                provider,
                message,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Too many requests");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_not_found() {
        let response = r#"{"error": {"type": "not_found_error", "message": "claude-unknown"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(model, "claude-unknown");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_overloaded() {
        let response =
            r#"{"error": {"type": "overloaded_error", "message": "Service is overloaded"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Service is overloaded");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_unknown_type() {
        let response = r#"{"error": {"type": "unknown_error", "message": "Something went wrong"}}"#;
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_anthropic_error_invalid_json() {
        let response = "invalid json";
        let error = ErrorUtils::parse_anthropic_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "anthropic");
                assert_eq!(message, "invalid json");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_google_error_invalid_argument() {
        let response = r#"{"error": {"status": "INVALID_ARGUMENT", "message": "Invalid input"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Invalid input");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_google_error_unauthenticated() {
        let response =
            r#"{"error": {"status": "UNAUTHENTICATED", "message": "Invalid credentials"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Invalid credentials");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_google_error_permission_denied() {
        let response = r#"{"error": {"status": "PERMISSION_DENIED", "message": "Access denied"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Access denied");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_google_error_resource_exhausted() {
        let response =
            r#"{"error": {"status": "RESOURCE_EXHAUSTED", "message": "Quota exceeded"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::RateLimit {
                provider, message, ..
            } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Quota exceeded");
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_parse_google_error_not_found() {
        let response = r#"{"error": {"status": "NOT_FOUND", "message": "Model not found"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "google");
                assert_eq!(model, "Model not found");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_parse_google_error_internal() {
        let response = r#"{"error": {"status": "INTERNAL", "message": "Internal error"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Internal error");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_google_error_unavailable() {
        let response = r#"{"error": {"status": "UNAVAILABLE", "message": "Service unavailable"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Service unavailable");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_parse_google_error_unknown_status() {
        let response =
            r#"{"error": {"status": "UNKNOWN_STATUS", "message": "Something went wrong"}}"#;
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "google");
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_google_error_invalid_json() {
        let response = "not json";
        let error = ErrorUtils::parse_google_error(response);
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "unknown");
                assert_eq!(message, "not json");
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[test]
    fn test_parse_provider_error_openai() {
        let response =
            r#"{"error": {"type": "invalid_request_error", "message": "Invalid model"}}"#;
        let error = ErrorUtils::parse_provider_error("openai", 400, response);
        match error {
            ProviderError::InvalidRequest { provider, .. } => {
                assert_eq!(provider, "openai");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_provider_error_anthropic() {
        let response =
            r#"{"error": {"type": "authentication_error", "message": "Invalid API key"}}"#;
        let error = ErrorUtils::parse_provider_error("anthropic", 401, response);
        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "anthropic");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_parse_provider_error_google() {
        let response = r#"{"error": {"status": "INVALID_ARGUMENT", "message": "Invalid input"}}"#;
        let error = ErrorUtils::parse_provider_error("google", 400, response);
        match error {
            ProviderError::InvalidRequest { provider, .. } => {
                assert_eq!(provider, "google");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_parse_provider_error_unknown_provider() {
        let response = "Error message";
        let error = ErrorUtils::parse_provider_error("unknown-provider", 500, response);
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "unknown-provider");
                assert_eq!(message, "Error message");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_parse_provider_error_case_insensitive() {
        let response = r#"{"error": {"type": "invalid_request_error", "message": "Invalid"}}"#;
        let error = ErrorUtils::parse_provider_error("OpenAI", 400, response);
        match error {
            ProviderError::InvalidRequest { provider, .. } => {
                assert_eq!(provider, "openai");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }
}
