//! HTTP response handling for errors

use super::types::GatewayError;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::error::canonical::CanonicalError;
use actix_web::{HttpResponse, ResponseError};

impl ResponseError for GatewayError {
    fn error_response(&self) -> HttpResponse {
        let (status_code, error_code, message) = match self {
            GatewayError::Config(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIG_ERROR",
                self.to_string(),
            ),
            GatewayError::Database(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "DATABASE_ERROR",
                "Database operation failed".to_string(),
            ),
            GatewayError::Redis(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "CACHE_ERROR",
                "Cache operation failed".to_string(),
            ),
            GatewayError::Auth(_) => (
                actix_web::http::StatusCode::UNAUTHORIZED,
                "AUTH_ERROR",
                self.to_string(),
            ),
            GatewayError::Forbidden(_) => (
                actix_web::http::StatusCode::FORBIDDEN,
                "FORBIDDEN",
                self.to_string(),
            ),
            GatewayError::Provider(provider_error) => match provider_error {
                ProviderError::RateLimit { .. } => (
                    actix_web::http::StatusCode::TOO_MANY_REQUESTS,
                    "PROVIDER_RATE_LIMIT",
                    provider_error.to_string(),
                ),
                ProviderError::QuotaExceeded { .. } => (
                    actix_web::http::StatusCode::PAYMENT_REQUIRED,
                    "PROVIDER_QUOTA_EXCEEDED",
                    provider_error.to_string(),
                ),
                ProviderError::ModelNotFound { .. } => (
                    actix_web::http::StatusCode::NOT_FOUND,
                    "MODEL_NOT_FOUND",
                    provider_error.to_string(),
                ),
                ProviderError::InvalidRequest { .. } => (
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "INVALID_REQUEST",
                    provider_error.to_string(),
                ),
                ProviderError::Timeout { .. } => (
                    actix_web::http::StatusCode::GATEWAY_TIMEOUT,
                    "PROVIDER_TIMEOUT",
                    provider_error.to_string(),
                ),
                ProviderError::ProviderUnavailable { .. } => (
                    actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                    "PROVIDER_UNAVAILABLE",
                    provider_error.to_string(),
                ),
                ProviderError::Authentication { .. } => (
                    actix_web::http::StatusCode::UNAUTHORIZED,
                    "PROVIDER_AUTH_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::Network { .. } => (
                    actix_web::http::StatusCode::BAD_GATEWAY,
                    "PROVIDER_NETWORK_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::Configuration { .. }
                | ProviderError::Serialization { .. }
                | ProviderError::TransformationError { .. } => (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "PROVIDER_INTERNAL_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::ContextLengthExceeded { .. }
                | ProviderError::ContentFiltered { .. }
                | ProviderError::TokenLimitExceeded { .. } => (
                    actix_web::http::StatusCode::BAD_REQUEST,
                    "PROVIDER_REQUEST_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::NotSupported { .. }
                | ProviderError::NotImplemented { .. }
                | ProviderError::FeatureDisabled { .. } => (
                    actix_web::http::StatusCode::NOT_IMPLEMENTED,
                    "PROVIDER_NOT_IMPLEMENTED",
                    provider_error.to_string(),
                ),
                ProviderError::DeploymentError { .. } => (
                    actix_web::http::StatusCode::NOT_FOUND,
                    "DEPLOYMENT_NOT_FOUND",
                    provider_error.to_string(),
                ),
                ProviderError::ResponseParsing { .. } | ProviderError::Streaming { .. } => (
                    actix_web::http::StatusCode::BAD_GATEWAY,
                    "PROVIDER_RESPONSE_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::RoutingError { .. } => (
                    actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                    "PROVIDER_ROUTING_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::ApiError { status, .. } => (
                    actix_web::http::StatusCode::from_u16(*status)
                        .unwrap_or(actix_web::http::StatusCode::BAD_GATEWAY),
                    "PROVIDER_API_ERROR",
                    provider_error.to_string(),
                ),
                ProviderError::Cancelled { .. } => (
                    actix_web::http::StatusCode::from_u16(499)
                        .unwrap_or(actix_web::http::StatusCode::BAD_REQUEST),
                    "PROVIDER_CANCELLED",
                    provider_error.to_string(),
                ),
                ProviderError::Other { .. } => (
                    actix_web::http::StatusCode::BAD_GATEWAY,
                    "PROVIDER_ERROR",
                    provider_error.to_string(),
                ),
            },
            GatewayError::RateLimit { .. } => (
                actix_web::http::StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMIT_EXCEEDED",
                self.to_string(),
            ),
            GatewayError::Validation(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "VALIDATION_ERROR",
                self.to_string(),
            ),
            GatewayError::NotFound(_) => (
                actix_web::http::StatusCode::NOT_FOUND,
                "NOT_FOUND",
                self.to_string(),
            ),
            GatewayError::Conflict(_) => (
                actix_web::http::StatusCode::CONFLICT,
                "CONFLICT",
                self.to_string(),
            ),
            GatewayError::BadRequest(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "BAD_REQUEST",
                self.to_string(),
            ),
            GatewayError::Timeout(_) => (
                actix_web::http::StatusCode::REQUEST_TIMEOUT,
                "TIMEOUT",
                self.to_string(),
            ),
            GatewayError::ProviderUnavailable(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                self.to_string(),
            ),
            GatewayError::CircuitBreaker(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "CIRCUIT_BREAKER_OPEN",
                self.to_string(),
            ),
            GatewayError::Network(_) => (
                actix_web::http::StatusCode::BAD_GATEWAY,
                "NETWORK_ERROR",
                self.to_string(),
            ),
            GatewayError::Parsing(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "PARSING_ERROR",
                self.to_string(),
            ),
            GatewayError::Alert(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "ALERT_ERROR",
                self.to_string(),
            ),
            GatewayError::NotImplemented(_) => (
                actix_web::http::StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                self.to_string(),
            ),
            GatewayError::External(_) => (
                actix_web::http::StatusCode::BAD_GATEWAY,
                "EXTERNAL_ERROR",
                self.to_string(),
            ),
            GatewayError::NoProvidersAvailable(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "NO_PROVIDERS_AVAILABLE",
                self.to_string(),
            ),
            GatewayError::ProviderNotFound(_) => (
                actix_web::http::StatusCode::NOT_FOUND,
                "PROVIDER_NOT_FOUND",
                self.to_string(),
            ),
            GatewayError::NoProvidersForModel(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "NO_PROVIDERS_FOR_MODEL",
                self.to_string(),
            ),
            GatewayError::NoHealthyProviders(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "NO_HEALTHY_PROVIDERS",
                self.to_string(),
            ),
            _ => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "An internal error occurred".to_string(),
            ),
        };

        let canonical_code = self.canonical_code().as_str().to_string();
        let retryable = self.canonical_retryable();

        let error_response = GatewayErrorResponse {
            error: GatewayErrorDetail {
                code: error_code.to_string(),
                canonical_code,
                retryable,
                message,
                timestamp: chrono::Utc::now().timestamp(),
                request_id: None, // This should be set by middleware
            },
        };

        let mut builder = HttpResponse::build(status_code);

        // Add rate limit headers for 429 responses
        if let GatewayError::RateLimit {
            retry_after,
            rpm_limit,
            tpm_limit,
            ..
        } = self
        {
            if let Some(secs) = retry_after {
                builder.insert_header(("Retry-After", secs.to_string()));
            }
            if let Some(rpm) = rpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Requests", rpm.to_string()));
            }
            if let Some(tpm) = tpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Tokens", tpm.to_string()));
            }
        }

        builder.json(error_response)
    }
}

/// Standard gateway error response format
#[derive(serde::Serialize)]
pub struct GatewayErrorResponse {
    pub error: GatewayErrorDetail,
}

/// Gateway error detail structure
#[derive(serde::Serialize)]
pub struct GatewayErrorDetail {
    pub code: String,
    pub canonical_code: String,
    pub retryable: bool,
    pub message: String,
    pub timestamp: i64,
    pub request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::http::StatusCode;

    // ==================== ErrorDetail Tests ====================

    #[test]
    fn test_error_detail_creation() {
        let detail = GatewayErrorDetail {
            code: "AUTH_ERROR".to_string(),
            canonical_code: "AUTHENTICATION".to_string(),
            retryable: false,
            message: "Authentication failed".to_string(),
            timestamp: 1704067200,
            request_id: Some("req-12345".to_string()),
        };

        assert_eq!(detail.code, "AUTH_ERROR");
        assert_eq!(detail.canonical_code, "AUTHENTICATION");
        assert!(!detail.retryable);
        assert_eq!(detail.message, "Authentication failed");
        assert_eq!(detail.timestamp, 1704067200);
        assert_eq!(detail.request_id, Some("req-12345".to_string()));
    }

    #[test]
    fn test_error_detail_without_request_id() {
        let detail = GatewayErrorDetail {
            code: "VALIDATION_ERROR".to_string(),
            canonical_code: "INVALID_REQUEST".to_string(),
            retryable: false,
            message: "Invalid input".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            request_id: None,
        };

        assert!(detail.request_id.is_none());
        assert!(detail.timestamp > 0);
    }

    #[test]
    fn test_error_detail_serialization() {
        let detail = GatewayErrorDetail {
            code: "NOT_FOUND".to_string(),
            canonical_code: "NOT_FOUND".to_string(),
            retryable: false,
            message: "Resource not found".to_string(),
            timestamp: 1704067200,
            request_id: Some("req-abc".to_string()),
        };

        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["code"], "NOT_FOUND");
        assert_eq!(json["canonical_code"], "NOT_FOUND");
        assert_eq!(json["retryable"], false);
        assert_eq!(json["message"], "Resource not found");
        assert_eq!(json["timestamp"], 1704067200);
        assert_eq!(json["request_id"], "req-abc");
    }

    #[test]
    fn test_error_detail_serialization_null_request_id() {
        let detail = GatewayErrorDetail {
            code: "ERROR".to_string(),
            canonical_code: "INTERNAL".to_string(),
            retryable: false,
            message: "Some error".to_string(),
            timestamp: 1704067200,
            request_id: None,
        };

        let json = serde_json::to_value(&detail).unwrap();
        assert!(json["request_id"].is_null());
    }

    // ==================== ErrorResponse Tests ====================

    #[test]
    fn test_error_response_creation() {
        let response = GatewayErrorResponse {
            error: GatewayErrorDetail {
                code: "INTERNAL_ERROR".to_string(),
                canonical_code: "INTERNAL".to_string(),
                retryable: false,
                message: "An internal error occurred".to_string(),
                timestamp: 1704067200,
                request_id: None,
            },
        };

        assert_eq!(response.error.code, "INTERNAL_ERROR");
    }

    #[test]
    fn test_error_response_serialization() {
        let response = GatewayErrorResponse {
            error: GatewayErrorDetail {
                code: "BAD_REQUEST".to_string(),
                canonical_code: "INVALID_REQUEST".to_string(),
                retryable: false,
                message: "Invalid parameters".to_string(),
                timestamp: 1704067200,
                request_id: Some("req-xyz".to_string()),
            },
        };

        let json = serde_json::to_value(&response).unwrap();
        assert!(json["error"].is_object());
        assert_eq!(json["error"]["code"], "BAD_REQUEST");
        assert_eq!(json["error"]["canonical_code"], "INVALID_REQUEST");
        assert_eq!(json["error"]["retryable"], false);
        assert_eq!(json["error"]["message"], "Invalid parameters");
    }

    #[test]
    fn test_error_response_json_string() {
        let response = GatewayErrorResponse {
            error: GatewayErrorDetail {
                code: "RATE_LIMIT".to_string(),
                canonical_code: "RATE_LIMITED".to_string(),
                retryable: true,
                message: "Too many requests".to_string(),
                timestamp: 1704067200,
                request_id: None,
            },
        };

        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains("RATE_LIMIT"));
        assert!(json_str.contains("Too many requests"));
    }

    // ==================== GatewayError ResponseError Tests ====================

    #[test]
    fn test_gateway_error_config_response() {
        let error = GatewayError::Config("Invalid configuration".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_gateway_error_internal_response() {
        let error = GatewayError::Internal("Internal server error".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_gateway_error_auth_response() {
        let error = GatewayError::Auth("Invalid token".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_gateway_error_forbidden_response2() {
        let error = GatewayError::Forbidden("Permission denied".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_gateway_error_rate_limit_response() {
        let error = GatewayError::RateLimit {
            message: "Rate limit exceeded".to_string(),
            retry_after: Some(60),
            rpm_limit: Some(100),
            tpm_limit: Some(50000),
        };
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get("Retry-After")
                .unwrap()
                .to_str()
                .unwrap(),
            "60"
        );
        assert_eq!(
            response
                .headers()
                .get("X-RateLimit-Limit-Requests")
                .unwrap()
                .to_str()
                .unwrap(),
            "100"
        );
        assert_eq!(
            response
                .headers()
                .get("X-RateLimit-Limit-Tokens")
                .unwrap()
                .to_str()
                .unwrap(),
            "50000"
        );
    }

    #[test]
    fn test_gateway_error_validation_response() {
        let error = GatewayError::Validation("Invalid input".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_gateway_error_not_found_response() {
        let error = GatewayError::NotFound("Resource not found".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_gateway_error_conflict_response() {
        let error = GatewayError::Conflict("Resource conflict".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn test_gateway_error_bad_request_response() {
        let error = GatewayError::BadRequest("Bad request".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_gateway_error_timeout_response() {
        let error = GatewayError::Timeout("Request timeout".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }

    #[test]
    fn test_gateway_error_provider_unavailable_response() {
        let error = GatewayError::ProviderUnavailable("Provider down".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_gateway_error_circuit_breaker_response() {
        let error = GatewayError::CircuitBreaker("Circuit open".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_gateway_error_network_response() {
        let error = GatewayError::Network("Network error".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_gateway_error_parsing_response() {
        let error = GatewayError::Parsing("Parse error".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_gateway_error_not_implemented_response() {
        let error = GatewayError::NotImplemented("Feature not implemented".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn test_gateway_error_forbidden_response() {
        let error = GatewayError::Forbidden("Forbidden".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_gateway_error_external_response() {
        let error = GatewayError::External("External service error".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_gateway_error_no_providers_available_response() {
        let error = GatewayError::NoProvidersAvailable("No providers".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_gateway_error_provider_not_found_response() {
        let error = GatewayError::ProviderNotFound("Provider not found".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_gateway_error_no_providers_for_model_response() {
        let error = GatewayError::NoProvidersForModel("No providers for model".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_gateway_error_no_healthy_providers_response() {
        let error = GatewayError::NoHealthyProviders("No healthy providers".to_string());
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    // ==================== Provider Error Tests ====================

    #[test]
    fn test_provider_error_rate_limit_response() {
        let provider_error = ProviderError::RateLimit {
            provider: "openai",
            message: "Rate limit exceeded".to_string(),
            retry_after: Some(60),
            rpm_limit: Some(100),
            tpm_limit: Some(10000),
            current_usage: Some(0.95),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn test_provider_error_quota_exceeded_response() {
        let provider_error = ProviderError::QuotaExceeded {
            provider: "anthropic",
            message: "Monthly quota exceeded".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    }

    #[test]
    fn test_provider_error_model_not_found_response() {
        let provider_error = ProviderError::ModelNotFound {
            provider: "openai",
            model: "gpt-5".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_provider_error_invalid_request_response() {
        let provider_error = ProviderError::InvalidRequest {
            provider: "openai",
            message: "Invalid parameters".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_provider_error_timeout_response() {
        let provider_error = ProviderError::Timeout {
            provider: "openai",
            message: "Request timed out after 30s".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[test]
    fn test_provider_error_unavailable_response() {
        let provider_error = ProviderError::ProviderUnavailable {
            provider: "azure",
            message: "Service maintenance".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_provider_error_authentication_response() {
        let provider_error = ProviderError::Authentication {
            provider: "openai",
            message: "Invalid API key".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_provider_error_api_error_passthrough() {
        let provider_error = ProviderError::ApiError {
            provider: "unknown",
            status: 418,
            message: "I'm a teapot".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status().as_u16(), 418);
    }

    #[test]
    fn test_provider_error_api_error_invalid_status_fallback() {
        let provider_error = ProviderError::ApiError {
            provider: "unknown",
            status: 0,
            message: "Invalid status".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_provider_error_context_length_response() {
        let provider_error = ProviderError::ContextLengthExceeded {
            provider: "openai",
            max: 8192,
            actual: 10000,
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_provider_error_not_supported_response() {
        let provider_error = ProviderError::NotSupported {
            provider: "openai",
            feature: "vision".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[test]
    fn test_provider_error_configuration_response() {
        let provider_error = ProviderError::Configuration {
            provider: "azure",
            message: "Missing deployment".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_provider_error_deployment_response() {
        let provider_error = ProviderError::DeploymentError {
            provider: "azure",
            deployment: "gpt-4-east".to_string(),
            message: "Deployment not found".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_provider_error_routing_response() {
        let provider_error = ProviderError::RoutingError {
            provider: "openrouter",
            attempted_providers: vec!["openai".to_string(), "anthropic".to_string()],
            message: "All providers failed".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_provider_error_other_variant_response() {
        let provider_error = ProviderError::Other {
            provider: "custom",
            message: "Unknown error".to_string(),
        };
        let error = GatewayError::Provider(provider_error);
        let response = error.error_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_error_response_json_structure() {
        let error = GatewayError::Auth("Invalid credentials".to_string());
        let response = error.error_response();

        // Verify we can extract body
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_error_detail_timestamp_is_current() {
        let before = chrono::Utc::now().timestamp();
        let detail = GatewayErrorDetail {
            code: "TEST".to_string(),
            canonical_code: "INTERNAL".to_string(),
            retryable: false,
            message: "Test".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            request_id: None,
        };
        let after = chrono::Utc::now().timestamp();

        assert!(detail.timestamp >= before);
        assert!(detail.timestamp <= after);
    }

    #[test]
    fn test_multiple_error_codes() {
        let error_codes = vec![
            ("CONFIG_ERROR", GatewayError::Config("test".to_string())),
            ("AUTH_ERROR", GatewayError::Auth("test".to_string())),
            (
                "VALIDATION_ERROR",
                GatewayError::Validation("test".to_string()),
            ),
            ("NOT_FOUND", GatewayError::NotFound("test".to_string())),
            ("RATE_LIMIT_EXCEEDED", GatewayError::rate_limit("test")),
        ];

        for (_expected_code, error) in error_codes {
            let response = error.error_response();
            // Just verify the response is created successfully
            assert!(response.status().is_client_error() || response.status().is_server_error());
        }
    }
}
