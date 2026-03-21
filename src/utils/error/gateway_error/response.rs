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
            GatewayError::Storage(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "STORAGE_ERROR",
                self.to_string(),
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
            GatewayError::Unavailable(_) => (
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "SERVICE_UNAVAILABLE",
                self.to_string(),
            ),
            GatewayError::Network(_) => (
                actix_web::http::StatusCode::BAD_GATEWAY,
                "NETWORK_ERROR",
                self.to_string(),
            ),
            GatewayError::Internal(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                self.to_string(),
            ),
            GatewayError::NotImplemented(_) => (
                actix_web::http::StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                self.to_string(),
            ),
            GatewayError::Serialization(_) => (
                actix_web::http::StatusCode::BAD_REQUEST,
                "SERIALIZATION_ERROR",
                self.to_string(),
            ),
            GatewayError::HttpClient(_) => (
                actix_web::http::StatusCode::BAD_GATEWAY,
                "HTTP_CLIENT_ERROR",
                self.to_string(),
            ),
            GatewayError::Io(_) => (
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "IO_ERROR",
                self.to_string(),
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
#[path = "response_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "response_consolidation_tests.rs"]
mod consolidation_tests;
