//! Gemini Error Handling
//!
//! Error handling

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

// Error
pub type GeminiError = ProviderError;

/// Error
pub struct GeminiErrorMapper;

impl ErrorMapper<ProviderError> for GeminiErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        Self::from_http_status(status_code, response_body)
    }
}

impl GeminiErrorMapper {
    /// Error
    pub fn from_http_status(status: u16, body: &str) -> ProviderError {
        match status {
            400 => ProviderError::invalid_request("gemini", format!("Bad request: {}", body)),
            401 => ProviderError::authentication("gemini", "Invalid or missing API key"),
            403 => ProviderError::authentication("gemini", "Forbidden: insufficient permissions"),
            404 => ProviderError::model_not_found("gemini", "Model or endpoint not found"),
            429 => {
                let retry_after = parse_retry_after_from_body(body);
                ProviderError::rate_limit("gemini", retry_after)
            }
            500..=599 => {
                ProviderError::api_error("gemini", status, format!("Server error: {}", body))
            }
            _ => HttpErrorMapper::map_status_code("gemini", status, body),
        }
    }

    /// Response
    pub fn from_api_response(response: &serde_json::Value) -> ProviderError {
        // Error
        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16;
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let status = error.get("status").and_then(|s| s.as_str()).unwrap_or("");

            return match (code, status) {
                (401, _) | (_, "UNAUTHENTICATED") => {
                    ProviderError::authentication("gemini", message)
                }
                (403, _) | (_, "PERMISSION_DENIED") => {
                    ProviderError::authentication("gemini", message)
                }
                (400, _) | (_, "INVALID_ARGUMENT") => {
                    ProviderError::invalid_request("gemini", message)
                }
                (404, _) | (_, "NOT_FOUND") => ProviderError::model_not_found("gemini", message),
                (429, _) | (_, "RESOURCE_EXHAUSTED") => {
                    let retry_after = Self::extract_retry_after_from_error(error);
                    ProviderError::RateLimit {
                        provider: "gemini",
                        message: message.to_string(),
                        retry_after,
                        rpm_limit: None,
                        tpm_limit: None,
                        current_usage: None,
                    }
                }
                (503, _) | (_, "UNAVAILABLE") => {
                    ProviderError::provider_unavailable("gemini", "Service unavailable")
                }
                (_, "FAILED_PRECONDITION") => ProviderError::invalid_request("gemini", message),
                (_, "UNIMPLEMENTED") => ProviderError::NotSupported {
                    provider: "gemini",
                    feature: message.to_string(),
                },
                _ => ProviderError::api_error("gemini", code, message),
            };
        }

        // Error
        if let Some(message) = response.get("message")
            && let Some(msg_str) = message.as_str()
        {
            return ProviderError::api_error("gemini", 500, msg_str);
        }

        // Error
        if let Some(candidates) = response.get("candidates")
            && let Some(candidate) = candidates.as_array().and_then(|c| c.first())
            && let Some(finish_reason) = candidate.get("finishReason").and_then(|r| r.as_str())
        {
            return match finish_reason {
                "SAFETY" => {
                    ProviderError::invalid_request("gemini", "Content blocked by safety filters")
                }
                "RECITATION" => {
                    ProviderError::invalid_request("gemini", "Content blocked due to recitation")
                }
                "MAX_TOKENS" => {
                    ProviderError::invalid_request("gemini", "Maximum token limit reached")
                }
                "STOP" => ProviderError::api_error("gemini", 200, "Generation completed"),
                _ => ProviderError::api_error(
                    "gemini",
                    500,
                    format!("Unknown finish reason: {}", finish_reason),
                ),
            };
        }

        // Default
        ProviderError::api_error("gemini", 500, "Unknown API error")
    }

    /// Extract retry delay from Gemini error object (handles `details[]` array)
    fn extract_retry_after_from_error(error: &serde_json::Value) -> Option<u64> {
        // Check
        if let Some(retry_after) = error.get("retry_after") {
            return retry_after.as_u64();
        }

        // Check
        if let Some(details) = error.get("details")
            && let Some(details_array) = details.as_array()
        {
            for detail in details_array {
                if let Some(retry_after) = detail.get("retry_after") {
                    return retry_after.as_u64();
                }
            }
        }

        None
    }
}

// Standard error helper functions
crate::define_provider_error_helpers!("gemini", gemini);

/// Create safety filter error (Gemini-specific)
pub fn gemini_safety_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::invalid_request(
        "gemini",
        format!("Content blocked by safety filters: {}", msg.into()),
    )
}

/// Create multimodal error (Gemini-specific)
pub fn gemini_multimodal_error(msg: impl Into<String>) -> ProviderError {
    ProviderError::NotSupported {
        provider: "gemini",
        feature: format!("multimodal: {}", msg.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_error_mapping() {
        let error = GeminiErrorMapper::from_http_status(401, "Unauthorized");
        match error {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "gemini");
            }
            _ => panic!("Expected authentication error"),
        }
    }

    #[test]
    fn test_google_api_error_parsing() {
        let response = json!({
            "error": {
                "code": 401,
                "message": "API key not valid",
                "status": "UNAUTHENTICATED"
            }
        });

        let error = GeminiErrorMapper::from_api_response(&response);
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "gemini");
                assert_eq!(message, "API key not valid");
            }
            _ => panic!("Expected authentication error"),
        }
    }

    #[test]
    fn test_rate_limit_error() {
        let response = json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded",
                "status": "RESOURCE_EXHAUSTED",
                "retry_after": 60
            }
        });

        let error = GeminiErrorMapper::from_api_response(&response);
        match error {
            ProviderError::RateLimit {
                provider,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "gemini");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected rate limit error"),
        }
    }

    #[test]
    fn test_safety_filter_error() {
        let response = json!({
            "candidates": [{
                "finishReason": "SAFETY",
                "safetyRatings": []
            }]
        });

        let error = GeminiErrorMapper::from_api_response(&response);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "gemini");
                assert!(message.contains("safety filters"));
            }
            _ => panic!("Expected invalid request error"),
        }
    }

    #[test]
    fn test_convenience_functions() {
        let config_err = gemini_config_error("Test config error");
        match config_err {
            ProviderError::Configuration { provider, .. } => assert_eq!(provider, "gemini"),
            _ => panic!("Expected configuration error"),
        }

        let auth_err = gemini_auth_error("Test auth error");
        match auth_err {
            ProviderError::Authentication { provider, .. } => assert_eq!(provider, "gemini"),
            _ => panic!("Expected authentication error"),
        }

        let api_err = gemini_api_error(400, "Test API error");
        match api_err {
            ProviderError::ApiError {
                provider, status, ..
            } => {
                assert_eq!(provider, "gemini");
                assert_eq!(status, 400);
            }
            _ => panic!("Expected API error"),
        }

        let safety_err = gemini_safety_error("Harmful content");
        match safety_err {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "gemini");
                assert!(message.contains("safety filters"));
            }
            _ => panic!("Expected invalid request error"),
        }
    }
}
