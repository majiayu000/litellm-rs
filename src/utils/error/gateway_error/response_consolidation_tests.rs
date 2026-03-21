use super::*;
use actix_web::http::StatusCode;

// Regression tests: 29->15 variant consolidation (commit aea82974).
// Each test documents which old variants merged into each new category
// and which HTTP status code that category must produce.

#[test]
fn test_exhaustive_variant_to_http_status_mapping() {
    // (variant, expected status, old variant group description)
    let cases: Vec<(GatewayError, StatusCode, &str)> = vec![
        // Config: merged EnvConfig, FileConfig, ConfigLoad, ...
        (
            GatewayError::Config("cfg".to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "Config -> 500",
        ),
        // Storage: merged Database, Cache, Redis, VectorDb, S3Storage, ...
        (
            GatewayError::Storage("db".to_string()),
            StatusCode::SERVICE_UNAVAILABLE,
            "Storage -> 503",
        ),
        // HttpClient: merged ReqwestError (#[from])
        (
            GatewayError::HttpClient(reqwest::Client::new().get("not-a-url").build().unwrap_err()),
            StatusCode::BAD_GATEWAY,
            "HttpClient -> 502",
        ),
        // Serialization: merged Json, Yaml, SerializationError, ...
        (
            GatewayError::Serialization("bad json".to_string()),
            StatusCode::BAD_REQUEST,
            "Serialization -> 400",
        ),
        // Io: merged IoError, FileNotFound, PermissionDenied, ...
        (
            GatewayError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "x")),
            StatusCode::INTERNAL_SERVER_ERROR,
            "Io -> 500",
        ),
        // Auth: merged Jwt, Crypto, Unauthorized, AuthError, ...
        (
            GatewayError::Auth("bad creds".to_string()),
            StatusCode::UNAUTHORIZED,
            "Auth -> 401",
        ),
        // RateLimit: merged RateLimit, TooManyRequests, ...
        (
            GatewayError::RateLimit {
                message: "rpm".to_string(),
                retry_after: None,
                rpm_limit: None,
                tpm_limit: None,
            },
            StatusCode::TOO_MANY_REQUESTS,
            "RateLimit -> 429",
        ),
        // Validation: merged Validation, Parsing, InvalidInput, ...
        (
            GatewayError::Validation("field".to_string()),
            StatusCode::BAD_REQUEST,
            "Validation -> 400",
        ),
        // Timeout: merged Timeout, RequestTimeout, ...
        (
            GatewayError::Timeout("30s".to_string()),
            StatusCode::REQUEST_TIMEOUT,
            "Timeout -> 408",
        ),
        // NotFound: merged NotFound, ResourceNotFound, ModelNotFound, ...
        (
            GatewayError::NotFound("item".to_string()),
            StatusCode::NOT_FOUND,
            "NotFound -> 404",
        ),
        // Conflict: merged Conflict, AlreadyExists, ...
        (
            GatewayError::Conflict("dup".to_string()),
            StatusCode::CONFLICT,
            "Conflict -> 409",
        ),
        // BadRequest: merged BadRequest, InvalidRequest, MalformedRequest, ...
        (
            GatewayError::BadRequest("param".to_string()),
            StatusCode::BAD_REQUEST,
            "BadRequest -> 400",
        ),
        // Internal: merged Internal, ServerError, Alert, ExternalService, ...
        (
            GatewayError::Internal("oops".to_string()),
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal -> 500",
        ),
        // Unavailable: merged ServiceUnavailable, CircuitOpen, NoProviders, ...
        (
            GatewayError::Unavailable("down".to_string()),
            StatusCode::SERVICE_UNAVAILABLE,
            "Unavailable -> 503",
        ),
        // Network: merged Network, External, ConnectionError, WebSocket, ...
        (
            GatewayError::Network("conn".to_string()),
            StatusCode::BAD_GATEWAY,
            "Network -> 502",
        ),
        // Forbidden: merged Forbidden, PermissionDenied, AccessDenied, ...
        (
            GatewayError::Forbidden("denied".to_string()),
            StatusCode::FORBIDDEN,
            "Forbidden -> 403",
        ),
        // NotImplemented: merged NotImplemented, FeatureNotSupported, ...
        (
            GatewayError::NotImplemented("feat".to_string()),
            StatusCode::NOT_IMPLEMENTED,
            "NotImplemented -> 501",
        ),
    ];

    for (error, expected_status, description) in cases {
        let response = error.error_response();
        assert_eq!(
            response.status(),
            expected_status,
            "HTTP status mismatch for: {description}"
        );
    }
}

#[test]
fn test_all_gateway_variants_return_error_status_codes() {
    // Every GatewayError variant must produce a 4xx or 5xx — never a 2xx/3xx.
    let variants: Vec<GatewayError> = vec![
        GatewayError::Config("x".to_string()),
        GatewayError::Storage("x".to_string()),
        GatewayError::HttpClient(reqwest::Client::new().get("not-a-url").build().unwrap_err()),
        GatewayError::Serialization("x".to_string()),
        GatewayError::Io(std::io::Error::other("x")),
        GatewayError::Auth("x".to_string()),
        GatewayError::RateLimit {
            message: "x".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        },
        GatewayError::Validation("x".to_string()),
        GatewayError::Timeout("x".to_string()),
        GatewayError::NotFound("x".to_string()),
        GatewayError::Conflict("x".to_string()),
        GatewayError::BadRequest("x".to_string()),
        GatewayError::Internal("x".to_string()),
        GatewayError::Unavailable("x".to_string()),
        GatewayError::Network("x".to_string()),
        GatewayError::Forbidden("x".to_string()),
        GatewayError::NotImplemented("x".to_string()),
    ];

    for variant in variants {
        let status = variant.error_response().status();
        assert!(
            status.is_client_error() || status.is_server_error(),
            "Variant {:?} produced non-error HTTP status {status}",
            variant
        );
    }
}
