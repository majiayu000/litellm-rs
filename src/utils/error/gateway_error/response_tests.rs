use super::*;
use crate::core::providers::unified_provider::ProviderError;
use actix_web::http::StatusCode;

fn assert_header(response: &actix_web::HttpResponse, name: &str, expected: &str) {
    assert_eq!(
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok()),
        Some(expected)
    );
}

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
    assert_header(&response, "Retry-After", "60");
    assert_header(&response, "X-RateLimit-Limit-Requests", "100");
    assert_header(&response, "X-RateLimit-Limit-Tokens", "50000");
}

#[test]
fn test_provider_rate_limit_response_preserves_headers() {
    let error = GatewayError::Provider(ProviderError::RateLimit {
        provider: "openai",
        message: "Upstream rate limited".to_string(),
        retry_after: Some(42),
        rpm_limit: Some(120),
        tpm_limit: Some(60000),
        current_usage: None,
    });

    let response = error.error_response();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_header(&response, "Retry-After", "42");
    assert_header(&response, "X-RateLimit-Limit-Requests", "120");
    assert_header(&response, "X-RateLimit-Limit-Tokens", "60000");
}

#[test]
fn test_provider_rate_limit_without_metadata_does_not_fake_headers() {
    let error = GatewayError::Provider(ProviderError::RateLimit {
        provider: "openai",
        message: "Upstream rate limited".to_string(),
        retry_after: None,
        rpm_limit: None,
        tpm_limit: None,
        current_usage: None,
    });

    let response = error.error_response();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().get("Retry-After").is_none());
    assert!(
        response
            .headers()
            .get("X-RateLimit-Limit-Requests")
            .is_none()
    );
    assert!(response.headers().get("X-RateLimit-Limit-Tokens").is_none());
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
    let error = GatewayError::Unavailable("Provider down".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn test_gateway_error_circuit_breaker_response() {
    let error = GatewayError::Unavailable("Circuit open".to_string());
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
    let error = GatewayError::Validation("Parse error".to_string());
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
    let error = GatewayError::Network("External service error".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[test]
fn test_gateway_error_no_providers_available_response() {
    let error = GatewayError::Unavailable("No providers".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn test_gateway_error_provider_not_found_response() {
    let error = GatewayError::NotFound("Provider not found".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_gateway_error_no_providers_for_model_response() {
    let error = GatewayError::BadRequest("No providers for model".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_gateway_error_no_healthy_providers_response() {
    let error = GatewayError::Unavailable("No healthy providers".to_string());
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

// ==================== Previously-wildcard Variant Tests ====================

#[test]
fn test_gateway_error_jwt_response() {
    use jsonwebtoken::{errors::Error as JwtError, errors::ErrorKind};
    let jwt_error = JwtError::from(ErrorKind::InvalidToken);
    let error = GatewayError::Auth(format!("JWT error: {}", jwt_error));
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_gateway_error_serialization_response() {
    let json_err: serde_json::Error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error: GatewayError = json_err.into();
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn test_gateway_error_cache_response() {
    let error = GatewayError::Storage("Cache unavailable".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn test_gateway_error_crypto_response() {
    let error = GatewayError::Auth("Encryption failed".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn test_gateway_error_file_storage_response() {
    let error = GatewayError::Internal("Write failed".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn test_gateway_error_http_client_response() {
    let req_err = reqwest::Client::new()
        .get("not-a-valid-url")
        .build()
        .unwrap_err();
    let error = GatewayError::HttpClient(req_err);
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[test]
fn test_gateway_error_io_response() {
    let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
    let error = GatewayError::Io(io_err);
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn test_gateway_error_vector_db_response() {
    let error = GatewayError::Storage("Vector search failed".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn test_gateway_error_yaml_response() {
    let yaml_err: serde_yml::Error =
        serde_yml::from_str::<serde_yml::Value>("key: [unclosed").unwrap_err();
    let error = GatewayError::Serialization(yaml_err.to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[cfg(feature = "vector-db")]
#[test]
fn test_gateway_error_qdrant_response() {
    let error = GatewayError::Storage("Qdrant unavailable".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[cfg(feature = "websockets")]
#[test]
fn test_gateway_error_websocket_response() {
    let error = GatewayError::Network("WS connection failed".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[cfg(feature = "s3")]
#[test]
fn test_gateway_error_s3_response() {
    let error = GatewayError::Storage("Bucket not found".to_string());
    let response = error.error_response();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
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
