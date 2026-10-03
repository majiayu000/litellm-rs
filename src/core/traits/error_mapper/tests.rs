//! Tests for error mapper implementations
//!
//! This module contains comprehensive tests for all error mapper types.

use super::implementations::{AnthropicErrorMapper, OpenAIErrorMapper};
use super::trait_def::ErrorMapper;
use super::types::GenericErrorMapper;
use crate::core::types::errors::ProviderErrorTrait;
use serde_json::json;

// Create a test error type for testing
#[derive(Debug, PartialEq)]
enum TestError {
    Authentication(String),
    RateLimit(Option<u64>),
    Network(String),
    NotSupported(String),
    Parsing(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authentication(msg) => write!(f, "Authentication: {}", msg),
            Self::RateLimit(retry) => write!(f, "Rate limit: {:?}", retry),
            Self::Network(msg) => write!(f, "Network: {}", msg),
            Self::NotSupported(msg) => write!(f, "Not supported: {}", msg),
            Self::Parsing(msg) => write!(f, "Parsing: {}", msg),
        }
    }
}

impl std::error::Error for TestError {}

impl ProviderErrorTrait for TestError {
    fn error_type(&self) -> &'static str {
        match self {
            Self::Authentication(_) => "authentication_error",
            Self::RateLimit(_) => "rate_limit_error",
            Self::Network(_) => "network_error",
            Self::NotSupported(_) => "not_supported_error",
            Self::Parsing(_) => "parsing_error",
        }
    }

    fn retry_delay(&self) -> Option<u64> {
        match self {
            Self::RateLimit(delay) => *delay,
            Self::Network(_) => Some(1),
            _ => None,
        }
    }

    fn http_status(&self) -> u16 {
        match self {
            Self::Authentication(_) => 401,
            Self::RateLimit(_) => 429,
            Self::NotSupported(_) => 404,
            Self::Network(_) => 500,
            Self::Parsing(_) => 400,
        }
    }

    fn not_supported(feature: &str) -> Self {
        Self::NotSupported(feature.to_string())
    }

    fn authentication_failed(reason: &str) -> Self {
        Self::Authentication(reason.to_string())
    }

    fn rate_limited(retry_after: Option<u64>) -> Self {
        Self::RateLimit(retry_after)
    }

    fn network_error(details: &str) -> Self {
        Self::Network(details.to_string())
    }

    fn parsing_error(details: &str) -> Self {
        Self::Parsing(details.to_string())
    }

    fn not_implemented(feature: &str) -> Self {
        Self::NotSupported(format!("Feature not implemented: {}", feature))
    }
}

#[test]
fn test_generic_error_mapper() {
    let mapper = GenericErrorMapper;

    // Test authentication error
    let auth_error: TestError = mapper.map_http_error(401, "Unauthorized");
    assert_eq!(
        auth_error,
        TestError::Authentication("Authentication failed: Invalid credentials".to_string())
    );

    // Test rate limit error
    let rate_limit_error: TestError = mapper.map_http_error(429, "Too Many Requests");
    assert_eq!(rate_limit_error, TestError::RateLimit(None));

    // Test server error
    let server_error: TestError = mapper.map_http_error(500, "Internal Server Error");
    assert_eq!(
        server_error,
        TestError::Network("Internal server error".to_string())
    );
}

#[test]
fn test_openai_error_mapper() {
    let mapper = OpenAIErrorMapper;

    // Test OpenAI JSON error format
    let error_json = json!({
        "error": {
            "type": "invalid_request_error",
            "code": "model_not_found",
            "message": "The model 'gpt-5' does not exist"
        }
    });

    let error: TestError = mapper.map_json_error(&error_json);
    assert_eq!(
        error,
        TestError::NotSupported("Model not found".to_string())
    );

    // Test rate limit with retry after
    let rate_limit_json = json!({
        "error": {
            "type": "rate_limit_error",
            "message": "Rate limit exceeded",
            "retry_after": 60
        }
    });

    let rate_error: TestError = mapper.map_json_error(&rate_limit_json);
    assert_eq!(rate_error, TestError::RateLimit(Some(60)));
}

#[test]
fn test_anthropic_error_mapper() {
    let mapper = AnthropicErrorMapper;

    // Test Anthropic JSON error format
    let error_json = json!({
        "type": "authentication_error",
        "message": "Invalid API key provided"
    });

    let error: TestError = mapper.map_json_error(&error_json);
    assert_eq!(
        error,
        TestError::Authentication("Invalid API key provided".to_string())
    );

    // Test validation error
    let validation_json = json!({
        "type": "validation_error",
        "message": "Missing required parameter: messages"
    });

    let validation_error: TestError = mapper.map_json_error(&validation_json);
    assert_eq!(
        validation_error,
        TestError::Network("Validation error: Missing required parameter: messages".to_string())
    );
}
