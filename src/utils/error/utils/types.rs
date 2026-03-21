use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub error_type: String,
    pub message: String,
    pub provider: String,
    pub request_id: Option<String>,
    pub timestamp: i64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorCategory {
    ClientError,    // 4xx errors
    ServerError,    // 5xx errors
    TransientError, // Retryable errors
    PermanentError, // Non-retryable errors
}

pub struct ErrorUtils;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_serialization() {
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), "value".to_string());

        let context = ErrorContext {
            error_type: "RateLimit".to_string(),
            message: "Too many requests".to_string(),
            provider: "openai".to_string(),
            request_id: Some("req-123".to_string()),
            timestamp: 1234567890,
            metadata,
        };

        // Test serialization
        let json = serde_json::to_string(&context).unwrap();
        assert!(json.contains("RateLimit"));
        assert!(json.contains("Too many requests"));
        assert!(json.contains("openai"));
        assert!(json.contains("req-123"));

        // Test deserialization
        let deserialized: ErrorContext = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.error_type, "RateLimit");
        assert_eq!(deserialized.message, "Too many requests");
        assert_eq!(deserialized.provider, "openai");
        assert_eq!(deserialized.request_id, Some("req-123".to_string()));
        assert_eq!(deserialized.timestamp, 1234567890);
        assert_eq!(deserialized.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_error_category_equality() {
        assert_eq!(ErrorCategory::ClientError, ErrorCategory::ClientError);
        assert_eq!(ErrorCategory::ServerError, ErrorCategory::ServerError);
        assert_eq!(ErrorCategory::TransientError, ErrorCategory::TransientError);
        assert_eq!(ErrorCategory::PermanentError, ErrorCategory::PermanentError);
        assert_ne!(ErrorCategory::ClientError, ErrorCategory::ServerError);
    }
}
