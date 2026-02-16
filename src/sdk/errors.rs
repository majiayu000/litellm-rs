//! Error handling

use thiserror::Error;

/// Error
#[derive(Error, Debug)]
pub enum SDKError {
    /// Provider not found
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// Default
    #[error("No default provider configured")]
    NoDefaultProvider,

    /// Error
    #[error("Provider error: {0}")]
    ProviderError(String),

    /// Configuration
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Error
    #[error("Authentication error: {0}")]
    AuthError(String),

    /// Error
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),

    /// Model
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Feature not supported
    #[error("Feature not supported: {0}")]
    NotSupported(String),

    /// Unsupported provider
    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    /// Error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Error
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    /// Request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Error
    #[error("API error: {0}")]
    ApiError(String),

    /// Error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Error
impl From<crate::utils::error::gateway_error::GatewayError> for SDKError {
    fn from(error: crate::utils::error::gateway_error::GatewayError) -> Self {
        match error {
            crate::utils::error::gateway_error::GatewayError::Auth(msg) => SDKError::AuthError(msg),
            crate::utils::error::gateway_error::GatewayError::NotFound(msg) => {
                SDKError::ModelNotFound(msg)
            }
            crate::utils::error::gateway_error::GatewayError::BadRequest(msg) => {
                SDKError::InvalidRequest(msg)
            }
            crate::utils::error::gateway_error::GatewayError::RateLimit(msg) => {
                SDKError::RateLimitError(msg)
            }
            crate::utils::error::gateway_error::GatewayError::ProviderUnavailable(msg) => {
                SDKError::ProviderError(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Internal(msg) => {
                SDKError::Internal(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Network(msg) => {
                SDKError::NetworkError(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Validation(msg) => {
                SDKError::InvalidRequest(msg)
            }
            crate::utils::error::gateway_error::GatewayError::Parsing(msg) => {
                SDKError::Internal(msg)
            }
            // Handle
            _ => SDKError::Internal(error.to_string()),
        }
    }
}

// Temporarily disabled old provider error mapping
/*
impl From<crate::core::providers::ProviderError> for SDKError {
    fn from(error: crate::core::providers::ProviderError) -> Self {
        match error {
            crate::core::providers::ProviderError::Authentication(msg) => SDKError::AuthError(msg),
            crate::core::providers::ProviderError::RateLimit(msg) => SDKError::RateLimitError(msg),
            crate::core::providers::ProviderError::RateLimited(msg) => {
                SDKError::RateLimitError(msg)
            }
            crate::core::providers::ProviderError::ModelNotFound(msg) => {
                SDKError::ModelNotFound(msg)
            }
            crate::core::providers::ProviderError::InvalidRequest(msg) => {
                SDKError::InvalidRequest(msg)
            }
            crate::core::providers::ProviderError::Unavailable(msg) => SDKError::ProviderError(msg),
            crate::core::providers::ProviderError::Network(msg) => SDKError::NetworkError(msg),
            crate::core::providers::ProviderError::Parsing(msg) => SDKError::Internal(msg),
            crate::core::providers::ProviderError::Timeout(msg) => SDKError::NetworkError(msg),
            crate::core::providers::ProviderError::Other(msg) => SDKError::Internal(msg),
            crate::core::providers::ProviderError::Unknown(msg) => SDKError::Internal(msg),
        }
    }
}
*/

/// SDK result type
pub type Result<T> = std::result::Result<T, SDKError>;

impl SDKError {
    /// Error
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            SDKError::NetworkError(_) | SDKError::RateLimitError(_) | SDKError::ProviderError(_)
        )
    }

    /// Error
    pub fn is_auth_error(&self) -> bool {
        matches!(self, SDKError::AuthError(_))
    }

    /// Configuration
    pub fn is_config_error(&self) -> bool {
        matches!(
            self,
            SDKError::ConfigError(_) | SDKError::ProviderNotFound(_) | SDKError::NoDefaultProvider
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::error::gateway_error::GatewayError;

    // ==================== SDKError Display Tests ====================

    #[test]
    fn test_sdk_error_provider_not_found() {
        let error = SDKError::ProviderNotFound("openai".to_string());
        assert_eq!(error.to_string(), "Provider not found: openai");
    }

    #[test]
    fn test_sdk_error_no_default_provider() {
        let error = SDKError::NoDefaultProvider;
        assert_eq!(error.to_string(), "No default provider configured");
    }

    #[test]
    fn test_sdk_error_provider_error() {
        let error = SDKError::ProviderError("API unavailable".to_string());
        assert_eq!(error.to_string(), "Provider error: API unavailable");
    }

    #[test]
    fn test_sdk_error_config_error() {
        let error = SDKError::ConfigError("Missing API key".to_string());
        assert_eq!(error.to_string(), "Configuration error: Missing API key");
    }

    #[test]
    fn test_sdk_error_network_error() {
        let error = SDKError::NetworkError("Connection refused".to_string());
        assert_eq!(error.to_string(), "Network error: Connection refused");
    }

    #[test]
    fn test_sdk_error_auth_error() {
        let error = SDKError::AuthError("Invalid API key".to_string());
        assert_eq!(error.to_string(), "Authentication error: Invalid API key");
    }

    #[test]
    fn test_sdk_error_rate_limit_error() {
        let error = SDKError::RateLimitError("Too many requests".to_string());
        assert_eq!(error.to_string(), "Rate limit exceeded: Too many requests");
    }

    #[test]
    fn test_sdk_error_model_not_found() {
        let error = SDKError::ModelNotFound("gpt-5".to_string());
        assert_eq!(error.to_string(), "Model not found: gpt-5");
    }

    #[test]
    fn test_sdk_error_not_supported() {
        let error = SDKError::NotSupported("streaming".to_string());
        assert_eq!(error.to_string(), "Feature not supported: streaming");
    }

    #[test]
    fn test_sdk_error_unsupported_provider() {
        let error = SDKError::UnsupportedProvider("custom-provider".to_string());
        assert_eq!(error.to_string(), "Unsupported provider: custom-provider");
    }

    #[test]
    fn test_sdk_error_invalid_request() {
        let error = SDKError::InvalidRequest("Missing messages".to_string());
        assert_eq!(error.to_string(), "Invalid request: Missing messages");
    }

    #[test]
    fn test_sdk_error_internal() {
        let error = SDKError::Internal("Unexpected state".to_string());
        assert_eq!(error.to_string(), "Internal error: Unexpected state");
    }

    #[test]
    fn test_sdk_error_api_error() {
        let error = SDKError::ApiError("Server returned 500".to_string());
        assert_eq!(error.to_string(), "API error: Server returned 500");
    }

    #[test]
    fn test_sdk_error_parse_error() {
        let error = SDKError::ParseError("Invalid JSON".to_string());
        assert_eq!(error.to_string(), "Parse error: Invalid JSON");
    }

    // ==================== SDKError is_retryable Tests ====================

    #[test]
    fn test_is_retryable_network_error() {
        let error = SDKError::NetworkError("timeout".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_is_retryable_rate_limit_error() {
        let error = SDKError::RateLimitError("limit exceeded".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_is_retryable_provider_error() {
        let error = SDKError::ProviderError("unavailable".to_string());
        assert!(error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_auth_error() {
        let error = SDKError::AuthError("invalid key".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_config_error() {
        let error = SDKError::ConfigError("bad config".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_invalid_request() {
        let error = SDKError::InvalidRequest("bad request".to_string());
        assert!(!error.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_internal() {
        let error = SDKError::Internal("bug".to_string());
        assert!(!error.is_retryable());
    }

    // ==================== SDKError is_auth_error Tests ====================

    #[test]
    fn test_is_auth_error_true() {
        let error = SDKError::AuthError("unauthorized".to_string());
        assert!(error.is_auth_error());
    }

    #[test]
    fn test_is_auth_error_false_for_others() {
        let errors = vec![
            SDKError::NetworkError("net".to_string()),
            SDKError::ConfigError("cfg".to_string()),
            SDKError::RateLimitError("rate".to_string()),
            SDKError::Internal("int".to_string()),
        ];

        for error in errors {
            assert!(!error.is_auth_error());
        }
    }

    // ==================== SDKError is_config_error Tests ====================

    #[test]
    fn test_is_config_error_config_error() {
        let error = SDKError::ConfigError("bad config".to_string());
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_config_error_provider_not_found() {
        let error = SDKError::ProviderNotFound("xyz".to_string());
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_config_error_no_default_provider() {
        let error = SDKError::NoDefaultProvider;
        assert!(error.is_config_error());
    }

    #[test]
    fn test_is_not_config_error_for_others() {
        let errors = vec![
            SDKError::NetworkError("net".to_string()),
            SDKError::AuthError("auth".to_string()),
            SDKError::RateLimitError("rate".to_string()),
        ];

        for error in errors {
            assert!(!error.is_config_error());
        }
    }

    // ==================== SDKError From GatewayError Tests ====================

    #[test]
    fn test_from_gateway_error_unauthorized() {
        let gateway_error = GatewayError::Auth("Invalid token".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::AuthError(_)));
        assert!(sdk_error.is_auth_error());
    }

    #[test]
    fn test_from_gateway_error_not_found() {
        let gateway_error = GatewayError::NotFound("Resource not found".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::ModelNotFound(_)));
    }

    #[test]
    fn test_from_gateway_error_bad_request() {
        let gateway_error = GatewayError::BadRequest("Invalid params".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_from_gateway_error_rate_limit() {
        let gateway_error = GatewayError::RateLimit("Too many requests".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::RateLimitError(_)));
        assert!(sdk_error.is_retryable());
    }

    #[test]
    fn test_from_gateway_error_provider_unavailable() {
        let gateway_error = GatewayError::ProviderUnavailable("OpenAI down".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::ProviderError(_)));
    }

    #[test]
    fn test_from_gateway_error_internal() {
        let gateway_error = GatewayError::Internal("Unexpected error".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::Internal(_)));
    }

    #[test]
    fn test_from_gateway_error_network() {
        let gateway_error = GatewayError::Network("Connection refused".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::NetworkError(_)));
        assert!(sdk_error.is_retryable());
    }

    #[test]
    fn test_from_gateway_error_validation() {
        let gateway_error = GatewayError::Validation("Invalid model".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_from_gateway_error_parsing() {
        let gateway_error = GatewayError::Parsing("Invalid JSON".to_string());
        let sdk_error: SDKError = gateway_error.into();
        assert!(matches!(sdk_error, SDKError::Internal(_)));
    }

    // ==================== SDKError Debug Tests ====================

    #[test]
    fn test_sdk_error_debug() {
        let error = SDKError::AuthError("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("AuthError"));
    }

    #[test]
    fn test_sdk_error_is_std_error() {
        let error = SDKError::Internal("test".to_string());
        let _: &dyn std::error::Error = &error;
    }

    // ==================== SDKError Edge Cases ====================

    #[test]
    fn test_sdk_error_empty_message() {
        let error = SDKError::ProviderError("".to_string());
        assert_eq!(error.to_string(), "Provider error: ");
    }

    #[test]
    fn test_sdk_error_unicode() {
        let error = SDKError::ApiError("错误信息 🚨".to_string());
        assert!(error.to_string().contains("错误信息"));
    }

    #[test]
    fn test_sdk_error_long_message() {
        let long_msg = "a".repeat(1000);
        let error = SDKError::Internal(long_msg.clone());
        assert!(error.to_string().contains(&long_msg));
    }
}
