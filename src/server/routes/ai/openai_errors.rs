//! OpenAI-compatible error response helpers for `/v1/*` endpoints.

use crate::core::providers::ProviderError;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, http::header};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OpenAiErrorResponse {
    error: OpenAiErrorDetail,
}

#[derive(Serialize)]
struct OpenAiErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    param: Option<String>,
    code: Option<String>,
}

struct OpenAiErrorSpec {
    status: StatusCode,
    message: String,
    error_type: String,
    param: Option<String>,
    code: Option<String>,
}

#[derive(Deserialize)]
struct UpstreamErrorEnvelope {
    error: UpstreamErrorDetail,
}

#[derive(Deserialize)]
struct UpstreamErrorDetail {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
    param: Option<String>,
    code: Option<serde_json::Value>,
}

pub(crate) fn validation_error(message: impl Into<String>) -> HttpResponse {
    build_response(spec(
        StatusCode::BAD_REQUEST,
        message.into(),
        "invalid_request_error",
        "invalid_request",
    ))
}

pub(crate) fn unauthorized_error(message: impl Into<String>) -> HttpResponse {
    build_response(spec(
        StatusCode::UNAUTHORIZED,
        message.into(),
        "authentication_error",
        "authentication_error",
    ))
}

pub(crate) fn gateway_error_response(error: &GatewayError) -> HttpResponse {
    let spec = gateway_error_spec(error);
    let mut builder = HttpResponse::build(spec.status);

    match error {
        GatewayError::RateLimit {
            retry_after,
            rpm_limit,
            tpm_limit,
            ..
        } => {
            if let Some(secs) = retry_after {
                builder.insert_header((header::RETRY_AFTER, secs.to_string()));
            }
            if let Some(rpm) = rpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Requests", rpm.to_string()));
            }
            if let Some(tpm) = tpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Tokens", tpm.to_string()));
            }
        }
        GatewayError::Provider(ProviderError::RateLimit {
            retry_after,
            rpm_limit,
            tpm_limit,
            ..
        }) => {
            if let Some(secs) = retry_after {
                builder.insert_header((header::RETRY_AFTER, secs.to_string()));
            }
            if let Some(rpm) = rpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Requests", rpm.to_string()));
            }
            if let Some(tpm) = tpm_limit {
                builder.insert_header(("X-RateLimit-Limit-Tokens", tpm.to_string()));
            }
        }
        _ => {}
    }

    builder.json(response_body(
        spec.message,
        spec.error_type,
        spec.param,
        spec.code,
    ))
}

fn build_response(spec: OpenAiErrorSpec) -> HttpResponse {
    HttpResponse::build(spec.status).json(response_body(
        spec.message,
        spec.error_type,
        spec.param,
        spec.code,
    ))
}

fn response_body(
    message: String,
    error_type: String,
    param: Option<String>,
    code: Option<String>,
) -> OpenAiErrorResponse {
    OpenAiErrorResponse {
        error: OpenAiErrorDetail {
            message,
            error_type,
            param,
            code,
        },
    }
}

fn gateway_error_spec(error: &GatewayError) -> OpenAiErrorSpec {
    match error {
        GatewayError::Config(_) | GatewayError::Internal(_) | GatewayError::Io(_) => spec(
            StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
            "server_error",
            "internal_error",
        ),
        GatewayError::Storage(_) => spec(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
            "server_error",
            "service_unavailable",
        ),
        GatewayError::HttpClient(_) | GatewayError::Network(_) => spec(
            StatusCode::BAD_GATEWAY,
            error.to_string(),
            "server_error",
            "network_error",
        ),
        GatewayError::Serialization(_)
        | GatewayError::Validation(_)
        | GatewayError::BadRequest(_) => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "invalid_request_error",
            "invalid_request",
        ),
        GatewayError::Auth(_) => spec(
            StatusCode::UNAUTHORIZED,
            error.to_string(),
            "authentication_error",
            "authentication_error",
        ),
        GatewayError::Forbidden(_) => spec(
            StatusCode::FORBIDDEN,
            error.to_string(),
            "permission_error",
            "permission_denied",
        ),
        GatewayError::Provider(provider_error) => provider_error_spec(provider_error),
        GatewayError::RateLimit { .. } => spec(
            StatusCode::TOO_MANY_REQUESTS,
            error.to_string(),
            "rate_limit_error",
            "rate_limit_exceeded",
        ),
        GatewayError::Timeout(_) => spec(
            StatusCode::REQUEST_TIMEOUT,
            error.to_string(),
            "server_error",
            "timeout",
        ),
        GatewayError::NotFound(_) => spec(
            StatusCode::NOT_FOUND,
            error.to_string(),
            "invalid_request_error",
            "not_found",
        ),
        GatewayError::Conflict(_) => spec(
            StatusCode::CONFLICT,
            error.to_string(),
            "invalid_request_error",
            "conflict",
        ),
        GatewayError::Unavailable(_) => spec(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
            "server_error",
            "service_unavailable",
        ),
        GatewayError::NotImplemented(_) => spec(
            StatusCode::NOT_IMPLEMENTED,
            error.to_string(),
            "invalid_request_error",
            "not_implemented",
        ),
    }
}

fn provider_error_spec(error: &ProviderError) -> OpenAiErrorSpec {
    match error {
        ProviderError::Authentication { .. } => spec(
            StatusCode::UNAUTHORIZED,
            error.to_string(),
            "authentication_error",
            "authentication_error",
        ),
        ProviderError::RateLimit { .. } => spec(
            StatusCode::TOO_MANY_REQUESTS,
            error.to_string(),
            "rate_limit_error",
            "rate_limit_exceeded",
        ),
        ProviderError::QuotaExceeded { .. } => spec(
            StatusCode::PAYMENT_REQUIRED,
            error.to_string(),
            "insufficient_quota",
            "insufficient_quota",
        ),
        ProviderError::ModelNotFound { .. } => spec(
            StatusCode::NOT_FOUND,
            error.to_string(),
            "invalid_request_error",
            "model_not_found",
        ),
        ProviderError::InvalidRequest { .. } => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "invalid_request_error",
            "invalid_request",
        ),
        ProviderError::Network { .. } => spec(
            StatusCode::BAD_GATEWAY,
            error.to_string(),
            "server_error",
            "provider_network_error",
        ),
        ProviderError::ProviderUnavailable { .. } => spec(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
            "server_error",
            "provider_unavailable",
        ),
        ProviderError::NotSupported { .. }
        | ProviderError::NotImplemented { .. }
        | ProviderError::FeatureDisabled { .. } => spec(
            StatusCode::NOT_IMPLEMENTED,
            error.to_string(),
            "invalid_request_error",
            "not_supported",
        ),
        ProviderError::Configuration { .. }
        | ProviderError::Serialization { .. }
        | ProviderError::TransformationError { .. } => spec(
            StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
            "server_error",
            "internal_error",
        ),
        ProviderError::Timeout { .. } => spec(
            StatusCode::GATEWAY_TIMEOUT,
            error.to_string(),
            "server_error",
            "timeout",
        ),
        ProviderError::ContextLengthExceeded { .. } => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "invalid_request_error",
            "context_length_exceeded",
        ),
        ProviderError::ContentFiltered { .. } => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "invalid_request_error",
            "content_filter",
        ),
        ProviderError::ApiError {
            status, message, ..
        } => api_error_spec(*status, message.clone()),
        ProviderError::TokenLimitExceeded { .. } => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "invalid_request_error",
            "token_limit_exceeded",
        ),
        ProviderError::DeploymentError { .. } => spec(
            StatusCode::NOT_FOUND,
            error.to_string(),
            "invalid_request_error",
            "deployment_not_found",
        ),
        ProviderError::ResponseParsing { .. } | ProviderError::Streaming { .. } => spec(
            StatusCode::BAD_GATEWAY,
            error.to_string(),
            "server_error",
            "provider_response_error",
        ),
        ProviderError::RoutingError { .. } => spec(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
            "server_error",
            "provider_routing_error",
        ),
        ProviderError::Cancelled { .. } => spec(
            StatusCode::BAD_REQUEST,
            error.to_string(),
            "server_error",
            "cancelled",
        ),
        ProviderError::Other { .. } => spec(
            StatusCode::BAD_GATEWAY,
            error.to_string(),
            "server_error",
            "provider_error",
        ),
    }
}

fn api_error_spec(status: u16, message: String) -> OpenAiErrorSpec {
    let status_code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut spec = match status {
        400 => spec(
            status_code,
            message,
            "invalid_request_error",
            "invalid_request",
        ),
        401 => spec(
            status_code,
            message,
            "authentication_error",
            "authentication_error",
        ),
        403 => spec(
            status_code,
            message,
            "permission_error",
            "permission_denied",
        ),
        404 => spec(status_code, message, "invalid_request_error", "not_found"),
        408 => spec(status_code, message, "server_error", "timeout"),
        409 => spec(status_code, message, "invalid_request_error", "conflict"),
        429 => spec(
            status_code,
            message,
            "rate_limit_error",
            "rate_limit_exceeded",
        ),
        500..=599 => spec(status_code, message, "server_error", "provider_api_error"),
        _ => spec(status_code, message, "server_error", "provider_api_error"),
    };

    if let Some(upstream) = parse_upstream_error_detail(&spec.message) {
        if let Some(message) = upstream.message {
            spec.message = message;
        }
        if let Some(error_type) = upstream.error_type {
            spec.error_type = error_type;
        }
        if let Some(param) = upstream.param {
            spec.param = Some(param);
        }
        if let Some(code) = upstream.code.and_then(error_code_to_string) {
            spec.code = Some(code);
        }
    }

    spec
}

fn parse_upstream_error_detail(message: &str) -> Option<UpstreamErrorDetail> {
    serde_json::from_str::<UpstreamErrorEnvelope>(message)
        .ok()
        .map(|envelope| envelope.error)
}

fn error_code_to_string(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) if !value.is_empty() => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn spec(
    status: StatusCode,
    message: String,
    error_type: &'static str,
    code: &'static str,
) -> OpenAiErrorSpec {
    OpenAiErrorSpec {
        status,
        message,
        error_type: error_type.to_string(),
        param: None,
        code: Some(code.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::body::to_bytes;
    use serde_json::Value;

    #[actix_web::test]
    async fn validation_error_uses_openai_shape() {
        let response = validation_error("model must not be empty");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_json(response).await;
        assert_eq!(body["error"]["message"], "model must not be empty");
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["param"], Value::Null);
        assert_eq!(body["error"]["code"], "invalid_request");
        assert!(body.get("success").is_none());
    }

    #[actix_web::test]
    async fn provider_rate_limit_uses_openai_shape_and_retry_after() {
        let error = GatewayError::Provider(ProviderError::RateLimit {
            provider: "openai",
            message: "Rate limit exceeded".to_string(),
            retry_after: Some(2),
            rpm_limit: Some(120),
            tpm_limit: Some(60000),
            current_usage: None,
        });

        let response = gateway_error_response(&error);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "2");
        assert_eq!(
            response
                .headers()
                .get("X-RateLimit-Limit-Requests")
                .and_then(|value| value.to_str().ok()),
            Some("120")
        );
        assert_eq!(
            response
                .headers()
                .get("X-RateLimit-Limit-Tokens")
                .and_then(|value| value.to_str().ok()),
            Some("60000")
        );
        let body = to_json(response).await;
        assert_eq!(body["error"]["type"], "rate_limit_error");
        assert_eq!(body["error"]["code"], "rate_limit_exceeded");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Rate limit exceeded")
        );
        assert!(body["error"]["retryable"].is_null());
    }

    #[actix_web::test]
    async fn provider_rate_limit_without_metadata_does_not_fake_openai_headers() {
        let error = GatewayError::Provider(ProviderError::RateLimit {
            provider: "openai",
            message: "Rate limit exceeded".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        });

        let response = gateway_error_response(&error);

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(response.headers().get(header::RETRY_AFTER).is_none());
        assert!(
            response
                .headers()
                .get("X-RateLimit-Limit-Requests")
                .is_none()
        );
        assert!(response.headers().get("X-RateLimit-Limit-Tokens").is_none());
    }

    #[actix_web::test]
    async fn provider_timeout_http_mapping_lives_at_openai_adapter_boundary() {
        let error = GatewayError::Provider(ProviderError::timeout("openai", "upstream timed out"));

        let response = gateway_error_response(&error);

        assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
        let body = to_json(response).await;
        assert_eq!(body["error"]["type"], "server_error");
        assert_eq!(body["error"]["code"], "timeout");
    }

    #[actix_web::test]
    async fn provider_api_error_preserves_upstream_openai_error_fields() {
        let upstream = serde_json::json!({
            "error": {
                "message": "context window exceeded",
                "type": "invalid_request_error",
                "param": "messages",
                "code": "context_length_exceeded"
            }
        });
        let error = GatewayError::Provider(ProviderError::api_error(
            "openai",
            400,
            upstream.to_string(),
        ));

        let response = gateway_error_response(&error);

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_json(response).await;
        assert_eq!(body["error"]["message"], "context window exceeded");
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["param"], "messages");
        assert_eq!(body["error"]["code"], "context_length_exceeded");
    }

    async fn to_json(response: HttpResponse) -> Value {
        let body = to_bytes(response.into_body()).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }
}
