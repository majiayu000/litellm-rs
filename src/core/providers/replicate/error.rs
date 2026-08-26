//! Replicate Error Handling
//!
//! Error handling for the Replicate provider

use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

// Standard error helper methods
crate::impl_provider_error_helpers!("replicate", replicate);

/// Replicate-specific error constructors (non-standard)
impl ProviderError {
    /// Create Replicate prediction failed error
    pub fn replicate_prediction_failed(message: impl Into<String>) -> Self {
        Self::api_error("replicate", 422, message.into())
    }

    /// Create Replicate prediction timeout error
    pub fn replicate_prediction_timeout(message: impl Into<String>) -> Self {
        Self::timeout("replicate", message)
    }

    /// Create Replicate prediction canceled error
    pub fn replicate_prediction_canceled(message: impl Into<String>) -> Self {
        Self::cancelled("replicate", "prediction", Some(message.into()))
    }
}

/// Error mapper for Replicate API responses
#[derive(Debug)]
pub struct ReplicateErrorMapper;

impl ErrorMapper<ProviderError> for ReplicateErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            401 => ProviderError::replicate_authentication("Invalid API token"),
            // Upstream 403 is a permission failure; keep the status.
            403 => ProviderError::api_error("replicate", status_code, "Permission denied"),
            404 => {
                // Try to extract model name from error
                if response_body.contains("model") || response_body.contains("version") {
                    ProviderError::replicate_model_not_found(response_body)
                } else {
                    ProviderError::replicate_api_error(404, response_body)
                }
            }
            422 => {
                // Unprocessable entity - usually a prediction failed
                ProviderError::replicate_prediction_failed(response_body)
            }
            429 => {
                // Rate limit - try to parse retry-after
                let retry_after = parse_retry_after_from_body(response_body);
                ProviderError::replicate_rate_limit(retry_after)
            }
            500..=599 => ProviderError::provider_unavailable("replicate", response_body),
            _ => ProviderError::replicate_api_error(status_code, response_body),
        }
    }

    fn map_json_error(&self, error_response: &serde_json::Value) -> ProviderError {
        // Replicate error format:
        // {"detail": "error message"} or
        // {"error": {"message": "...", "code": "..."}}
        let error_msg = error_response
            .get("detail")
            .and_then(|d| d.as_str())
            .or_else(|| {
                error_response
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
            })
            .unwrap_or("Unknown error");

        let error_type = error_response
            .get("type")
            .or_else(|| error_response.get("error").and_then(|e| e.get("type")))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        match error_type {
            "authentication_error" | "invalid_token" => {
                ProviderError::replicate_authentication(error_msg)
            }
            "rate_limit_exceeded" => ProviderError::replicate_rate_limit(None),
            "model_not_found" | "version_not_found" => {
                ProviderError::replicate_model_not_found(error_msg)
            }
            "prediction_failed" => ProviderError::replicate_prediction_failed(error_msg),
            _ => ProviderError::other("replicate", error_msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replicate_authentication_error() {
        let err = ProviderError::replicate_authentication("Invalid API token");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_replicate_rate_limit_error() {
        let err = ProviderError::replicate_rate_limit(Some(60));
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_replicate_model_not_found_error() {
        let err = ProviderError::replicate_model_not_found("unknown/model");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_replicate_prediction_failed_error() {
        let err = ProviderError::replicate_prediction_failed("Prediction failed");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::ApiError { status: 422, .. }));
    }

    #[test]
    fn test_replicate_prediction_timeout_error() {
        let err = ProviderError::replicate_prediction_timeout("Timeout waiting for result");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::Timeout { .. }));
    }

    #[test]
    fn test_error_mapper_401() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_error_mapper_403() {
        let mapper = ReplicateErrorMapper;
        // 403 keeps its upstream status as a permission failure.
        let err = mapper.map_http_error(403, "Forbidden");
        assert!(matches!(err, ProviderError::ApiError { status: 403, .. }));
    }

    #[test]
    fn test_error_mapper_404_model() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(404, "model not found");
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_error_mapper_404_other() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(404, "resource not found");
        assert!(matches!(err, ProviderError::ApiError { status: 404, .. }));
    }

    #[test]
    fn test_error_mapper_422() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(422, "Prediction failed");
        assert!(matches!(err, ProviderError::ApiError { status: 422, .. }));
    }

    #[test]
    fn test_error_mapper_429() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(429, "rate limit exceeded");
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_error_mapper_500() {
        let mapper = ReplicateErrorMapper;
        let err = mapper.map_http_error(500, "Internal server error");
        assert!(matches!(err, ProviderError::ProviderUnavailable { .. }));
    }

    #[test]
    fn test_parse_retry_after_with_rate_limit() {
        let result = parse_retry_after_from_body("rate limit exceeded");
        assert_eq!(result, Some(60));
    }

    #[test]
    fn test_parse_retry_after_with_too_many_requests() {
        let result = parse_retry_after_from_body("too many requests");
        assert_eq!(result, Some(60));
    }

    #[test]
    fn test_parse_retry_after_no_match() {
        let result = parse_retry_after_from_body("some other error");
        assert_eq!(result, None);
    }

    #[test]
    fn test_error_mapper_json_authentication() {
        let mapper = ReplicateErrorMapper;
        let json = serde_json::json!({
            "type": "authentication_error",
            "detail": "Invalid token"
        });
        let err = mapper.map_json_error(&json);
        assert!(matches!(err, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_error_mapper_json_model_not_found() {
        let mapper = ReplicateErrorMapper;
        let json = serde_json::json!({
            "type": "model_not_found",
            "detail": "Model not found"
        });
        let err = mapper.map_json_error(&json);
        assert!(matches!(err, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_error_mapper_json_rate_limit() {
        let mapper = ReplicateErrorMapper;
        let json = serde_json::json!({
            "type": "rate_limit_exceeded",
            "detail": "Too many requests"
        });
        let err = mapper.map_json_error(&json);
        assert!(matches!(err, ProviderError::RateLimit { .. }));
    }

    #[test]
    fn test_error_mapper_json_unknown() {
        let mapper = ReplicateErrorMapper;
        let json = serde_json::json!({
            "type": "unknown_error",
            "detail": "Something went wrong"
        });
        let err = mapper.map_json_error(&json);
        assert!(matches!(err, ProviderError::Other { .. }));
    }

    #[test]
    fn test_replicate_prediction_canceled() {
        let err = ProviderError::replicate_prediction_canceled("User cancelled");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::Cancelled { .. }));
    }

    #[test]
    fn test_replicate_configuration_error() {
        let err = ProviderError::replicate_configuration("Missing API token");
        assert!(err.is_replicate_error());
        assert!(matches!(err, ProviderError::Configuration { .. }));
    }
}
