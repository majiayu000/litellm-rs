//! Cohere Provider Error Handling
//!
//! Uses the unified ProviderError with Cohere-specific constructor functions

use crate::core::providers::unified_provider::ProviderError;

pub type CohereError = ProviderError;

/// Create Cohere authentication error
#[cfg(test)]
pub fn cohere_authentication(message: impl Into<String>) -> CohereError {
    ProviderError::authentication("cohere", message)
}

/// Create Cohere rate limit error
#[cfg(test)]
pub fn cohere_rate_limit(retry_after: Option<u64>) -> CohereError {
    ProviderError::rate_limit("cohere", retry_after)
}

/// Create Cohere model not found error
#[cfg(test)]
pub fn cohere_model_not_found(model: impl Into<String>) -> CohereError {
    ProviderError::model_not_found("cohere", model)
}

/// Create Cohere invalid request error
pub fn cohere_invalid_request(message: impl Into<String>) -> CohereError {
    ProviderError::invalid_request("cohere", message)
}

/// Create Cohere network error
#[cfg(test)]
pub fn cohere_network_error(message: impl Into<String>) -> CohereError {
    ProviderError::network("cohere", message)
}

/// Create Cohere timeout error
#[cfg(test)]
pub fn cohere_timeout(message: impl Into<String>) -> CohereError {
    ProviderError::Timeout {
        provider: "cohere",
        message: message.into(),
    }
}

/// Create Cohere response parsing error
pub fn cohere_response_parsing(message: impl Into<String>) -> CohereError {
    ProviderError::response_parsing("cohere", message)
}

/// Create Cohere API error with status code
#[cfg(test)]
pub fn cohere_api_error(status: u16, message: impl Into<String>) -> CohereError {
    ProviderError::ApiError {
        provider: "cohere",
        status,
        message: message.into(),
    }
}

/// Check if this is a Cohere-specific error
#[cfg(test)]
pub fn is_cohere_error(err: &CohereError) -> bool {
    err.provider() == "cohere"
}

/// Get Cohere error category for metrics
#[cfg(test)]
pub fn cohere_category(err: &CohereError) -> &'static str {
    match err {
        ProviderError::Authentication { .. } => "auth",
        ProviderError::RateLimit { .. } => "rate_limit",
        ProviderError::ModelNotFound { .. } => "model",
        ProviderError::Network { .. } | ProviderError::Timeout { .. } => "network",
        ProviderError::ResponseParsing { .. } | ProviderError::Serialization { .. } => "parsing",
        ProviderError::InvalidRequest { .. } => "invalid_request",
        ProviderError::Configuration { .. } => "configuration",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cohere_authentication_error() {
        let err = cohere_authentication("Invalid API key");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "auth");
    }

    #[test]
    fn test_cohere_rate_limit_error() {
        let err = cohere_rate_limit(Some(60));
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "rate_limit");
    }

    #[test]
    fn test_cohere_model_not_found_error() {
        let err = cohere_model_not_found("unknown-model");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "model");
    }

    #[test]
    fn test_cohere_network_error() {
        let err = cohere_network_error("Connection failed");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "network");
    }

    #[test]
    fn test_cohere_timeout_error() {
        let err = cohere_timeout("Request timed out");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "network");
    }

    #[test]
    fn test_cohere_api_error() {
        let err = cohere_api_error(500, "Internal server error");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "other");
    }

    #[test]
    fn test_cohere_invalid_request() {
        let err = cohere_invalid_request("Bad request format");
        assert!(is_cohere_error(&err));
        assert_eq!(cohere_category(&err), "invalid_request");
    }
}
