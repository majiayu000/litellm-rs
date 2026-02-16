//! Error types for the Gateway

use crate::core::providers::unified_provider::ProviderError;
use thiserror::Error;

/// Result type alias for the Gateway
pub type Result<T> = std::result::Result<T, GatewayError>;

/// Main error type for the Gateway
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum GatewayError {
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Database errors
    #[cfg(feature = "storage")]
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),

    /// Database errors (storage feature disabled)
    #[cfg(not(feature = "storage"))]
    #[error("Database error: {0}")]
    Database(String),

    /// Redis errors
    #[cfg(feature = "redis")]
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// Redis errors (redis feature disabled)
    #[cfg(not(feature = "redis"))]
    #[error("Redis error: {0}")]
    Redis(String),

    /// HTTP client errors
    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// YAML parsing errors
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Provider errors
    #[error("Provider error: {0}")]
    Provider(ProviderError),

    /// Rate limiting errors
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Cache errors
    #[error("Cache error: {0}")]
    Cache(String),

    /// Circuit breaker errors
    #[error("Circuit breaker error: {0}")]
    CircuitBreaker(String),

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

    /// Service unavailable errors
    #[error("Service unavailable: {0}")]
    ProviderUnavailable(String),

    /// JWT errors
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Crypto errors
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// File storage errors
    #[error("File storage error: {0}")]
    FileStorage(String),

    /// Vector database errors
    #[error("Vector database error: {0}")]
    VectorDb(String),

    /// Network errors
    #[error("Network error: {0}")]
    Network(String),

    /// Parsing errors
    #[error("Parsing error: {0}")]
    Parsing(String),

    /// Alert errors
    #[error("Alert error: {0}")]
    Alert(String),

    /// Not implemented errors
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Forbidden errors
    #[error("Forbidden: {0}")]
    Forbidden(String),

    /// External service errors
    #[error("External service error: {0}")]
    External(String),

    /// No providers available
    #[error("No providers available: {0}")]
    NoProvidersAvailable(String),

    /// Provider not found
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// No providers for model
    #[error("No providers for model: {0}")]
    NoProvidersForModel(String),

    /// No healthy providers
    #[error("No healthy providers: {0}")]
    NoHealthyProviders(String),

    /// S3 storage errors
    #[cfg(feature = "s3")]
    #[error("S3 error: {0}")]
    S3(String),

    /// Vector database client errors
    #[cfg(feature = "vector-db")]
    #[error("Qdrant error: {0}")]
    Qdrant(String),

    /// WebSocket errors
    #[cfg(feature = "websockets")]
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Error Display Tests ====================

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
    fn test_external_error_display() {
        let error = GatewayError::External("Third-party API error".to_string());
        assert_eq!(
            error.to_string(),
            "External service error: Third-party API error"
        );
    }

    #[test]
    fn test_rate_limit_error_display() {
        let error = GatewayError::RateLimit("100 requests per minute exceeded".to_string());
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
    fn test_cache_error_display() {
        let error = GatewayError::Cache("Cache connection failed".to_string());
        assert_eq!(error.to_string(), "Cache error: Cache connection failed");
    }

    #[test]
    fn test_circuit_breaker_error_display() {
        let error = GatewayError::CircuitBreaker("Circuit is open for provider X".to_string());
        assert_eq!(
            error.to_string(),
            "Circuit breaker error: Circuit is open for provider X"
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
    fn test_conflict_error_display() {
        let error = GatewayError::Conflict("Resource already exists".to_string());
        assert_eq!(error.to_string(), "Conflict: Resource already exists");
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
    fn test_provider_unavailable_error_display() {
        let error = GatewayError::ProviderUnavailable("OpenAI is down".to_string());
        assert_eq!(error.to_string(), "Service unavailable: OpenAI is down");
    }

    #[test]
    fn test_crypto_error_display() {
        let error = GatewayError::Crypto("Encryption failed".to_string());
        assert_eq!(error.to_string(), "Crypto error: Encryption failed");
    }

    #[test]
    fn test_file_storage_error_display() {
        let error = GatewayError::FileStorage("Failed to write file".to_string());
        assert_eq!(
            error.to_string(),
            "File storage error: Failed to write file"
        );
    }

    #[test]
    fn test_vector_db_error_display() {
        let error = GatewayError::VectorDb("Vector search failed".to_string());
        assert_eq!(
            error.to_string(),
            "Vector database error: Vector search failed"
        );
    }

    #[test]
    fn test_network_error_display() {
        let error = GatewayError::Network("Connection refused".to_string());
        assert_eq!(error.to_string(), "Network error: Connection refused");
    }

    #[test]
    fn test_parsing_error_display() {
        let error = GatewayError::Parsing("Invalid date format".to_string());
        assert_eq!(error.to_string(), "Parsing error: Invalid date format");
    }

    #[test]
    fn test_alert_error_display() {
        let error = GatewayError::Alert("Failed to send alert".to_string());
        assert_eq!(error.to_string(), "Alert error: Failed to send alert");
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
    fn test_no_providers_available_error_display() {
        let error = GatewayError::NoProvidersAvailable("All providers are down".to_string());
        assert_eq!(
            error.to_string(),
            "No providers available: All providers are down"
        );
    }

    #[test]
    fn test_provider_not_found_error_display() {
        let error = GatewayError::ProviderNotFound("openai".to_string());
        assert_eq!(error.to_string(), "Provider not found: openai");
    }

    #[test]
    fn test_no_providers_for_model_error_display() {
        let error = GatewayError::NoProvidersForModel("gpt-5".to_string());
        assert_eq!(error.to_string(), "No providers for model: gpt-5");
    }

    #[test]
    fn test_no_healthy_providers_error_display() {
        let error =
            GatewayError::NoHealthyProviders("All providers failed health check".to_string());
        assert_eq!(
            error.to_string(),
            "No healthy providers: All providers failed health check"
        );
    }

    // ==================== Error Debug Tests ====================

    #[test]
    fn test_error_debug_format() {
        let error = GatewayError::Config("Test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("Config"));
        assert!(debug_str.contains("Test"));
    }

    // ==================== Result Type Tests ====================

    #[test]
    fn test_result_ok() {
        let result: Result<i32> = Ok(42);
        assert!(result.is_ok());
        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn test_result_err() {
        let err = GatewayError::Validation("Invalid".to_string());
        let result: Result<i32> = Err(err);
        assert!(result.is_err());
        match result {
            Err(e) => assert!(e.to_string().contains("Validation")),
            Ok(_) => panic!("Expected Err variant"),
        }
    }

    // ==================== Error Conversion Tests ====================

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

    // ==================== Error Source Tests ====================

    #[test]
    fn test_error_is_std_error() {
        let error = GatewayError::Config("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    // ==================== Error Category Tests ====================

    #[test]
    fn test_authentication_errors() {
        let error = GatewayError::Auth("Invalid token".to_string());
        let msg = error.to_string().to_lowercase();
        assert!(msg.contains("auth"));
    }

    #[test]
    fn test_provider_errors() {
        let errors = [
            GatewayError::ProviderUnavailable("down".to_string()),
            GatewayError::ProviderNotFound("openai".to_string()),
            GatewayError::NoProvidersAvailable("none".to_string()),
            GatewayError::NoProvidersForModel("gpt-4".to_string()),
            GatewayError::NoHealthyProviders("all failed".to_string()),
        ];

        assert_eq!(errors.len(), 5);
    }

    #[test]
    fn test_validation_and_request_errors() {
        let errors = vec![
            GatewayError::Validation("field required".to_string()),
            GatewayError::BadRequest("invalid payload".to_string()),
        ];

        for error in errors {
            let msg = error.to_string().to_lowercase();
            assert!(
                msg.contains("validation") || msg.contains("request"),
                "Expected validation/request error, got: {}",
                msg
            );
        }
    }

    // ==================== Feature-gated Error Tests ====================

    #[cfg(feature = "s3")]
    #[test]
    fn test_s3_error_display() {
        let error = GatewayError::S3("Bucket not found".to_string());
        assert_eq!(error.to_string(), "S3 error: Bucket not found");
    }

    #[cfg(feature = "vector-db")]
    #[test]
    fn test_qdrant_error_display() {
        let error = GatewayError::Qdrant("Collection not found".to_string());
        assert_eq!(error.to_string(), "Qdrant error: Collection not found");
    }

    #[cfg(feature = "websockets")]
    #[test]
    fn test_websocket_error_display() {
        let error = GatewayError::WebSocket("Connection closed".to_string());
        assert_eq!(error.to_string(), "WebSocket error: Connection closed");
    }
}
