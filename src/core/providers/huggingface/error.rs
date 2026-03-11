//! HuggingFace Provider Error Handling
//!
//! Error types and mapping for HuggingFace API responses.

pub use crate::core::providers::unified_provider::ProviderError;

/// HuggingFace-specific error type alias
pub type HuggingFaceError = ProviderError;

// Standard error helper methods
crate::impl_provider_error_helpers!("huggingface", huggingface);

/// HuggingFace-specific error constructors (non-standard)
impl ProviderError {
    /// Create HuggingFace provider not found error
    pub fn huggingface_provider_not_found(model: &str, provider: &str) -> Self {
        Self::InvalidRequest {
            provider: "huggingface",
            message: format!(
                "Model '{}' is not available for provider '{}'. Check provider mapping.",
                model, provider
            ),
        }
    }

    /// Create HuggingFace staging model warning error
    pub fn huggingface_staging_model(model: &str, provider: &str) -> Self {
        Self::Other {
            provider: "huggingface",
            message: format!(
                "Model '{}' is in staging mode for provider '{}'. Meant for test purposes only.",
                model, provider
            ),
        }
    }
}

/// Parse HuggingFace API error response
pub fn parse_hf_error_response(status: u16, body: &str) -> HuggingFaceError {
    // Try to parse as JSON error response
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(error_msg) = json
            .get("error")
            .and_then(|e| e.as_str())
            .or_else(|| json.get("message").and_then(|m| m.as_str()))
    {
        return match status {
            401 => ProviderError::huggingface_authentication(error_msg),
            403 => ProviderError::huggingface_authentication(format!("Forbidden: {}", error_msg)),
            404 => ProviderError::huggingface_model_not_found(error_msg),
            429 => {
                let retry_after = json.get("retry_after").and_then(|r| r.as_u64());
                ProviderError::huggingface_rate_limit(retry_after)
            }
            _ => ProviderError::huggingface_api_error(status, error_msg),
        };
    }

    // Fallback to raw body
    match status {
        401 => ProviderError::huggingface_authentication(body),
        403 => ProviderError::huggingface_authentication(format!("Forbidden: {}", body)),
        404 => ProviderError::huggingface_model_not_found(body),
        429 => ProviderError::huggingface_rate_limit(None),
        _ => ProviderError::huggingface_api_error(status, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authentication_error() {
        let err = ProviderError::huggingface_authentication("Invalid token");
        match err {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "huggingface");
                assert!(message.contains("Invalid token"));
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_model_not_found_error() {
        let err = ProviderError::huggingface_model_not_found("non-existent/model");
        match err {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "huggingface");
                assert_eq!(model, "non-existent/model");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_provider_not_found_error() {
        let err = ProviderError::huggingface_provider_not_found("model-id", "unknown-provider");
        match err {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "huggingface");
                assert!(message.contains("model-id"));
                assert!(message.contains("unknown-provider"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_rate_limit_error() {
        let err = ProviderError::huggingface_rate_limit(Some(60));
        match err {
            ProviderError::RateLimit {
                provider,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "huggingface");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_parse_json_error_response() {
        let body = r#"{"error": "Model not found"}"#;
        let err = parse_hf_error_response(404, body);
        match err {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "huggingface");
                assert_eq!(model, "Model not found");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_parse_rate_limit_with_retry() {
        let body = r#"{"error": "Rate limit exceeded", "retry_after": 30}"#;
        let err = parse_hf_error_response(429, body);
        match err {
            ProviderError::RateLimit {
                provider,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "huggingface");
                assert_eq!(retry_after, Some(30));
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_parse_raw_error_response() {
        let body = "Unauthorized";
        let err = parse_hf_error_response(401, body);
        match err {
            ProviderError::Authentication { provider, .. } => {
                assert_eq!(provider, "huggingface");
            }
            _ => panic!("Expected Authentication error"),
        }
    }
}
