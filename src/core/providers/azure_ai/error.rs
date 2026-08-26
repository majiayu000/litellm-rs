//! Azure AI Error Handling
//!
//! Error handling

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

/// Error
#[derive(Debug, Clone, Default)]
pub struct AzureAIErrorMapper;

impl ErrorMapper<ProviderError> for AzureAIErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            400 => {
                // Error
                if response_body.contains("invalid_request_error") {
                    ProviderError::invalid_request("azure_ai", response_body)
                } else if response_body.contains("model_not_found") {
                    ProviderError::model_not_found("azure_ai", "Requested model not available")
                } else {
                    ProviderError::invalid_request("azure_ai", response_body)
                }
            }
            401 => {
                if response_body.contains("invalid_api_key") {
                    ProviderError::authentication("azure_ai", "Invalid API key")
                } else if response_body.contains("insufficient_quota") {
                    ProviderError::quota_exceeded("azure_ai", "API quota exceeded")
                } else {
                    ProviderError::authentication("azure_ai", "Authentication failed")
                }
            }
            // Upstream 403 is a permission failure; keep the status.
            403 => ProviderError::api_error("azure_ai", status_code, "Access forbidden"),
            404 => {
                if response_body.contains("model") {
                    ProviderError::model_not_found("azure_ai", "Model not found")
                } else {
                    ProviderError::api_error("azure_ai", status_code, "Endpoint not found")
                }
            }
            429 => {
                // Response
                let retry_after = parse_retry_after_from_body(response_body);
                ProviderError::rate_limit("azure_ai", retry_after)
            }
            500 => ProviderError::api_error("azure_ai", 500, "Internal server error"),
            502 => ProviderError::api_error("azure_ai", 502, "Bad gateway"),
            503 => ProviderError::provider_unavailable("azure_ai", "Service unavailable"),
            504 => ProviderError::timeout("azure_ai", "Gateway timeout"),
            _ => HttpErrorMapper::map_status_code("azure_ai", status_code, response_body),
        }
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        let error_str = error.to_string();

        if error_str.contains("timeout") || error_str.contains("timed out") {
            ProviderError::timeout("azure_ai", &error_str)
        } else if error_str.contains("connection") {
            ProviderError::network("azure_ai", &error_str)
        } else if error_str.contains("dns") || error_str.contains("resolve") {
            ProviderError::network("azure_ai", "DNS resolution failed")
        } else {
            ProviderError::network("azure_ai", &error_str)
        }
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::response_parsing("azure_ai", error.to_string())
    }
}

/// Error
pub fn is_unsupported_feature_error(response_body: &str) -> bool {
    let unsupported_indicators = [
        "not supported",
        "not available",
        "feature not enabled",
        "unsupported model",
        "unsupported parameter",
    ];

    let body_lower = response_body.to_lowercase();
    unsupported_indicators
        .iter()
        .any(|&indicator| body_lower.contains(indicator))
}

/// Error
pub fn is_content_filter_error(response_body: &str) -> bool {
    let content_filter_indicators = [
        "content_filter",
        "content filtered",
        "harmful content",
        "inappropriate content",
    ];

    let body_lower = response_body.to_lowercase();
    content_filter_indicators
        .iter()
        .any(|&indicator| body_lower.contains(indicator))
}

/// Response
pub fn extract_error_message(response_body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(response_body) {
        // Error
        let possible_paths: Vec<Vec<&str>> = vec![
            vec!["error", "message"],
            vec!["error", "details"],
            vec!["message"],
            vec!["detail"],
            vec!["error_description"],
        ];

        for path in &possible_paths {
            let mut current = &json;
            for &key in path {
                if let Some(next) = current.get(key) {
                    current = next;
                } else {
                    break;
                }
            }

            if let Some(message) = current.as_str() {
                return message.to_string();
            }
        }

        // If no standard message found, return the entire JSON as string
        return json.to_string();
    }

    // Response
    response_body.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_after_parsing() {
        let json_response = r#"{"error": {"retry_after": 60, "message": "Rate limit exceeded"}}"#;
        let retry_after = parse_retry_after_from_body(json_response);
        assert_eq!(retry_after, Some(60));
    }

    #[test]
    fn test_rate_limit_detection() {
        let response = "Rate limit exceeded. Please try again later.";
        let retry_after = parse_retry_after_from_body(response);
        assert_eq!(retry_after, Some(60));
    }

    #[test]
    fn test_error_message_extraction() {
        let json_response = r#"{"error": {"message": "Invalid request format"}}"#;
        let message = extract_error_message(json_response);
        assert_eq!(message, "Invalid request format");
    }

    #[test]
    fn test_content_filter_detection() {
        let response = "Content filtered due to harmful content detection";
        assert!(is_content_filter_error(response));
    }

    #[test]
    fn test_unsupported_feature_detection() {
        let response = "This feature is not supported for the selected model";
        assert!(is_unsupported_feature_error(response));
    }
}
