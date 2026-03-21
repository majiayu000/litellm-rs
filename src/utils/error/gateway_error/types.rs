//! Error types for the Gateway

use crate::core::providers::unified_provider::ProviderError;
use thiserror::Error;

/// Result type alias for the Gateway
pub type Result<T> = std::result::Result<T, GatewayError>;

/// Main error type for the Gateway
///
/// Consolidated from ~36 variants to 15 semantic categories.
/// Each variant maps to a distinct HTTP status code or error class.
#[derive(Error, Debug)]
pub enum GatewayError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Storage errors (database, cache, Redis, vector DB, S3)
    #[error("Storage error: {0}")]
    Storage(String),

    /// HTTP client errors
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    /// Serialization/deserialization errors (JSON, YAML, etc.)
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO errors (file system, local file storage)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Authentication and cryptographic errors (auth, JWT, crypto)
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Provider errors
    #[error("Provider error: {0}")]
    Provider(ProviderError),

    /// Rate limiting errors with structured metadata
    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
    },

    /// Validation and parsing errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Timeout errors
    #[error("Timeout error: {0}")]
    Timeout(String),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Conflict errors
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Bad request errors
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Internal server errors
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Service unavailable (provider unavailable, circuit breaker, no healthy providers)
    #[error("Service unavailable: {0}")]
    Unavailable(String),

    /// Network errors (connectivity, external services, WebSocket)
    #[error("Network error: {0}")]
    Network(String),

    /// Forbidden errors
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// Not implemented errors
    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

// Manual From impl for serde_json::Error (was previously #[from])
impl From<serde_json::Error> for GatewayError {
    fn from(err: serde_json::Error) -> Self {
        GatewayError::Serialization(err.to_string())
    }
}

// Manual From impl for serde_yml::Error (previously Yaml variant with #[from])
impl From<serde_yml::Error> for GatewayError {
    fn from(err: serde_yml::Error) -> Self {
        GatewayError::Serialization(err.to_string())
    }
}

// Manual From impl for jsonwebtoken errors (previously Jwt variant with #[from])
impl From<jsonwebtoken::errors::Error> for GatewayError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        GatewayError::Auth(format!("JWT error: {}", err))
    }
}

// Manual From impl for redis errors (previously Redis variant with #[from])
#[cfg(feature = "redis")]
impl From<redis::RedisError> for GatewayError {
    fn from(err: redis::RedisError) -> Self {
        GatewayError::Storage(format!("Redis error: {}", err))
    }
}

// Manual From impl for sea_orm errors (previously Database variant with #[from])
#[cfg(feature = "storage")]
impl From<sea_orm::DbErr> for GatewayError {
    fn from(err: sea_orm::DbErr) -> Self {
        GatewayError::Storage(format!("Database error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let error = GatewayError::Config("Invalid API key format".to_string());
        assert_eq!(
            error.to_string(),
            "Configuration error: Invalid API key format"
        );
    }

    #[test]
    fn test_auth_error_display() {
        let error = GatewayError::Auth("Invalid credentials".to_string());
        assert_eq!(
            error.to_string(),
            "Authentication error: Invalid credentials"
        );
    }

    #[test]
    fn test_forbidden_error_display() {
        let error = GatewayError::Forbidden("Access denied".to_string());
        assert_eq!(error.to_string(), "Forbidden: Access denied");
    }

    #[test]
    fn test_storage_error_display() {
        let error = GatewayError::Storage("Cache connection failed".to_string());
        assert_eq!(error.to_string(), "Storage error: Cache connection failed");
    }

    #[test]
    fn test_rate_limit_error_display() {
        let error = GatewayError::RateLimit {
            message: "100 requests per minute exceeded".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        assert_eq!(
            error.to_string(),
            "Rate limit exceeded: 100 requests per minute exceeded"
        );
    }

    #[test]
    fn test_validation_error_display() {
        let error = GatewayError::Validation("Model name is required".to_string());
        assert_eq!(
            error.to_string(),
            "Validation error: Model name is required"
        );
    }

    #[test]
    fn test_timeout_error_display() {
        let error = GatewayError::Timeout("Request timed out after 30s".to_string());
        assert_eq!(
            error.to_string(),
            "Timeout error: Request timed out after 30s"
        );
    }

    #[test]
    fn test_not_found_error_display() {
        let error = GatewayError::NotFound("User not found".to_string());
        assert_eq!(error.to_string(), "Not found: User not found");
    }

    #[test]
    fn test_bad_request_error_display() {
        let error = GatewayError::BadRequest("Invalid JSON payload".to_string());
        assert_eq!(error.to_string(), "Bad request: Invalid JSON payload");
    }

    #[test]
    fn test_internal_error_display() {
        let error = GatewayError::Internal("Unexpected error occurred".to_string());
        assert_eq!(
            error.to_string(),
            "Internal server error: Unexpected error occurred"
        );
    }

    #[test]
    fn test_unavailable_error_display() {
        let error = GatewayError::Unavailable("OpenAI is down".to_string());
        assert_eq!(error.to_string(), "Service unavailable: OpenAI is down");
    }

    #[test]
    fn test_network_error_display() {
        let error = GatewayError::Network("Connection refused".to_string());
        assert_eq!(error.to_string(), "Network error: Connection refused");
    }

    #[test]
    fn test_not_implemented_error_display() {
        let error = GatewayError::NotImplemented("Feature X is not implemented".to_string());
        assert_eq!(
            error.to_string(),
            "Not implemented: Feature X is not implemented"
        );
    }

    #[test]
    fn test_error_debug_format() {
        let error = GatewayError::Config("Test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Config"));
        assert!(debug_str.contains("Test"));
    }

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_result_err() {
        let err = GatewayError::Validation("Invalid".to_string());
        let result: Result<i32> = Err(err);
        assert!(result.is_err());
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let gateway_error: GatewayError = io_error.into();
        assert!(gateway_error.to_string().contains("IO error"));
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let json_result: std::result::Result<serde_json::Value, _> =
            serde_json::from_str("invalid json{");
        let json_error = json_result.unwrap_err();
        let gateway_error: GatewayError = json_error.into();
        assert!(gateway_error.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_error_is_std_error() {
        let error = GatewayError::Config("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_serde_yml_error_conversion() {
        let yaml_result: std::result::Result<serde_yml::Value, _> =
            serde_yml::from_str("key: [unclosed");
        let yaml_error = yaml_result.unwrap_err();
        let gateway_error: GatewayError = yaml_error.into();
        assert!(
            matches!(gateway_error, GatewayError::Serialization(_)),
            "From<serde_yml::Error> must produce GatewayError::Serialization"
        );
        assert!(gateway_error.to_string().contains("Serialization error"));
    }

    #[test]
    fn test_jwt_error_conversion() {
        use jsonwebtoken::errors::{Error as JwtError, ErrorKind};
        let jwt_error = JwtError::from(ErrorKind::InvalidToken);
        let gateway_error: GatewayError = jwt_error.into();
        assert!(
            matches!(gateway_error, GatewayError::Auth(_)),
            "From<jsonwebtoken::errors::Error> must produce GatewayError::Auth"
        );
        let msg = gateway_error.to_string();
        assert!(
            msg.contains("JWT error"),
            "Auth message must include 'JWT error' prefix, got: {msg}"
        );
    }

    #[test]
    fn test_jwt_error_conversion_preserves_error_kind() {
        use jsonwebtoken::errors::{Error as JwtError, ErrorKind};
        let jwt_error = JwtError::from(ErrorKind::ExpiredSignature);
        let gateway_error: GatewayError = jwt_error.into();
        let msg = gateway_error.to_string();
        assert!(msg.starts_with("Authentication error: JWT error:"));
        assert!(
            msg.contains("ExpiredSignature"),
            "Auth message must preserve the JWT error kind, got: {msg}"
        );
    }

    #[cfg(feature = "redis")]
    #[test]
    fn test_redis_error_conversion() {
        let redis_err = redis::RedisError::from((
            redis::ErrorKind::AuthenticationFailed,
            "NOAUTH Authentication required",
        ));
        let gateway_err: GatewayError = redis_err.into();
        assert!(
            matches!(gateway_err, GatewayError::Storage(_)),
            "From<redis::RedisError> must produce GatewayError::Storage"
        );
        let msg = gateway_err.to_string();
        assert!(msg.contains("Redis error"));
    }

    #[cfg(feature = "storage")]
    #[test]
    fn test_sea_orm_db_err_conversion() {
        let db_err = sea_orm::DbErr::Custom("connection refused".to_string());
        let gateway_err: GatewayError = db_err.into();
        assert!(
            matches!(gateway_err, GatewayError::Storage(_)),
            "From<sea_orm::DbErr> must produce GatewayError::Storage"
        );
        let msg = gateway_err.to_string();
        assert!(msg.contains("Database error"));
    }
}
