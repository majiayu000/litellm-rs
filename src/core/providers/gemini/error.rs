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
            // Upstream 403 is a permission failure; keep the status.
            403 => ProviderError::api_error(
                "gemini",
                status,
                crate::core::providers::unified_provider::parse_error_message_from_body(body)
                    .unwrap_or_else(|| body.to_string()),
            ),
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
        crate::core::providers::google_error::map_google_error_envelope("gemini", response)
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
    fn status_only_permission_denied_defaults_to_403() {
        let response = json!({
            "error": {
                "message": "caller lacks permission",
                "status": "PERMISSION_DENIED"
            }
        });

        assert!(matches!(
            GeminiErrorMapper::from_api_response(&response),
            ProviderError::ApiError {
                status: 403,
                ref message,
                ..
            } if message == "caller lacks permission"
        ));
    }

    #[test]
    fn http_403_preserves_google_error_envelope_message() {
        let body = r#"{"error":{"code":403,"message":"service account lacks aiplatform.endpoints.predict","status":"PERMISSION_DENIED"}}"#;
        assert!(matches!(
            GeminiErrorMapper::from_http_status(403, body),
            ProviderError::ApiError {
                status: 403,
                ref message,
                ..
            } if message == "service account lacks aiplatform.endpoints.predict"
        ));
    }

    #[test]
    fn http_403_transport_status_overrides_contradictory_envelope_code() {
        let body = r#"{"error":{"code":401,"message":"accepted credential lacks model permission","status":"UNAUTHENTICATED"}}"#;
        assert!(matches!(
            GeminiErrorMapper::from_http_status(403, body),
            ProviderError::ApiError {
                status: 403,
                ref message,
                ..
            } if message == "accepted credential lacks model permission"
        ));
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
