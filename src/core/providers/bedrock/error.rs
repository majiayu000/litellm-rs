//! Bedrock Provider Error Handling
//!
//! Comprehensive error types and mapping for AWS Bedrock provider

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use serde_json::Value;

/// Error mapper for Bedrock provider
#[derive(Debug, Clone)]
pub struct BedrockErrorMapper;

impl BedrockErrorMapper {
    pub(crate) fn map_service_error(
        error_code: &str,
        error_message: &str,
    ) -> Option<ProviderError> {
        let details = format!("{error_code}: {error_message}");
        match error_code.to_ascii_lowercase().as_str() {
            "validationexception" => Some(ProviderError::invalid_request("bedrock", details)),
            "unauthorizedexception" => Some(ProviderError::authentication("bedrock", details)),
            "accessdeniedexception" => Some(ProviderError::api_error("bedrock", 403, details)),
            "throttlingexception" | "servicequotaexceededexception" => {
                Some(ProviderError::rate_limit("bedrock", None))
            }
            "modelnotreadyexception" => {
                Some(ProviderError::retryable_api_error("bedrock", 424, details))
            }
            "resourcenotfoundexception" => Some(ProviderError::api_error("bedrock", 404, details)),
            "badgatewayexception" => Some(ProviderError::network("bedrock", details)),
            "conflictexception" => Some(ProviderError::api_error("bedrock", 409, details)),
            "dependencyfailedexception" => {
                Some(ProviderError::retryable_api_error("bedrock", 424, details))
            }
            "internalserverexception" => Some(ProviderError::api_error("bedrock", 500, details)),
            _ => None,
        }
    }
}

impl ErrorMapper<ProviderError> for BedrockErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> ProviderError {
        match status_code {
            400 => {
                ProviderError::invalid_request("bedrock", format!("Bad request: {}", response_body))
            }
            401 => ProviderError::authentication(
                "bedrock",
                "Invalid AWS credentials or insufficient permissions".to_string(),
            ),
            403 => ProviderError::api_error(
                "bedrock",
                403,
                format!("Access forbidden: {}", response_body),
            ),
            404 => ProviderError::model_not_found(
                "bedrock",
                "Model not found or not available in region".to_string(),
            ),
            429 => ProviderError::rate_limit("bedrock", None),
            500 => ProviderError::api_error("bedrock", 500, "Internal server error".to_string()),
            502 => ProviderError::network("bedrock", "Bad gateway".to_string()),
            503 => ProviderError::api_error("bedrock", 503, "Service unavailable".to_string()),
            _ => HttpErrorMapper::map_status_code(
                "bedrock",
                status_code,
                &format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }

    fn map_json_error(&self, error_response: &Value) -> ProviderError {
        if let Some(error) = error_response.get("error") {
            let error_code = error
                .get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("UNKNOWN_ERROR");
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");

            Self::map_service_error(error_code, error_message).unwrap_or_else(|| {
                ProviderError::api_error(
                    "bedrock",
                    400,
                    format!("{}: {}", error_code, error_message),
                )
            })
        } else {
            ProviderError::response_parsing("bedrock", "Unknown error response format".to_string())
        }
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::network("bedrock", format!("Network error: {}", error))
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> ProviderError {
        ProviderError::response_parsing("bedrock", format!("Parsing error: {}", error))
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> ProviderError {
        ProviderError::timeout(
            "bedrock",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_error_mapping() {
        let mapper = BedrockErrorMapper;

        let error = mapper.map_http_error(400, "Bad request");
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));

        let error = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(error, ProviderError::Authentication { .. }));

        let error = mapper.map_http_error(403, "Forbidden");
        assert!(matches!(error, ProviderError::ApiError { status: 403, .. }));
        assert_eq!(
            crate::core::providers::unified_provider::provider_http_error_facts(&error).status,
            403
        );
        assert!(!error.is_retryable());

        let error = mapper.map_http_error(429, "Rate limited");
        assert!(matches!(error, ProviderError::RateLimit { .. }));

        let error = mapper.map_http_error(
            424,
            "ModelNotReadyException: misleading ordinary HTTP message",
        );
        assert!(matches!(error, ProviderError::ApiError { status: 424, .. }));
        assert!(!error.is_retryable());
        assert_eq!(error.retry_delay(), None);
    }

    #[test]
    fn test_json_error_mapping() {
        let mapper = BedrockErrorMapper;

        let error_json = json!({
            "error": {
                "code": "ValidationException",
                "message": "Invalid input"
            }
        });

        let error = mapper.map_json_error(&error_json);
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
    }

    // Note: Specific error helper tests removed after unifying on ProviderError.
}
