use crate::core::providers::unified_provider::ProviderError;

use super::types::{ErrorCategory, ErrorUtils};

impl ErrorUtils {
    pub fn format_error_for_user(error: &ProviderError) -> String {
        match error {
            ProviderError::Authentication { message, .. } => {
                format!("Authentication failed: {}", message)
            }
            ProviderError::InvalidRequest { message, .. } => {
                format!("Request validation failed: {}", message)
            }
            ProviderError::RateLimit { message, .. } => {
                format!("Rate limit exceeded: {}", message)
            }
            ProviderError::QuotaExceeded { message, .. } => {
                format!("Quota exceeded: {}", message)
            }
            ProviderError::ModelNotFound { model, .. } => {
                format!("Model not supported: {}", model)
            }
            ProviderError::Timeout { message, .. } => {
                format!("Request timeout: {}", message)
            }
            ProviderError::Other { message, .. } => {
                format!("Provider error: {}", message)
            }
            ProviderError::Network { message, .. } => {
                format!("Network error: {}", message)
            }
            ProviderError::ProviderUnavailable { message, .. } => {
                format!("Provider unavailable: {}", message)
            }
            ProviderError::Serialization { message, .. } => {
                format!("Parsing error: {}", message)
            }
            _ => {
                format!("Provider error: {}", error)
            }
        }
    }

    pub fn get_error_category(error: &ProviderError) -> ErrorCategory {
        match error {
            ProviderError::InvalidRequest { .. } => ErrorCategory::ClientError,
            ProviderError::Authentication { .. } => ErrorCategory::ClientError,
            ProviderError::ModelNotFound { .. } => ErrorCategory::ClientError,
            ProviderError::RateLimit { .. } => ErrorCategory::TransientError,
            ProviderError::QuotaExceeded { .. } => ErrorCategory::ClientError,
            ProviderError::Network { .. } => ErrorCategory::TransientError,
            ProviderError::Timeout { .. } => ErrorCategory::TransientError,
            ProviderError::ProviderUnavailable { .. } => ErrorCategory::TransientError,
            ProviderError::Configuration { .. } => ErrorCategory::PermanentError,
            ProviderError::NotSupported { .. } => ErrorCategory::PermanentError,
            ProviderError::NotImplemented { .. } => ErrorCategory::PermanentError,
            _ => ErrorCategory::ServerError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::unified_provider::ProviderError;

    #[test]
    fn test_format_error_for_user_authentication() {
        let error = ProviderError::Authentication {
            provider: "openai",
            message: "Invalid API key".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Authentication failed: Invalid API key");
    }

    #[test]
    fn test_format_error_for_user_invalid_request() {
        let error = ProviderError::InvalidRequest {
            provider: "openai",
            message: "Missing required field".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(
            formatted,
            "Request validation failed: Missing required field"
        );
    }

    #[test]
    fn test_format_error_for_user_rate_limit() {
        let error = ProviderError::RateLimit {
            provider: "openai",
            message: "Too many requests".to_string(),
            retry_after: Some(60),
            rpm_limit: None,
            tpm_limit: None,
            current_usage: None,
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Rate limit exceeded: Too many requests");
    }

    #[test]
    fn test_format_error_for_user_quota_exceeded() {
        let error = ProviderError::QuotaExceeded {
            provider: "openai",
            message: "Monthly quota exceeded".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Quota exceeded: Monthly quota exceeded");
    }

    #[test]
    fn test_format_error_for_user_model_not_found() {
        let error = ProviderError::ModelNotFound {
            provider: "openai",
            model: "gpt-unknown".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Model not supported: gpt-unknown");
    }

    #[test]
    fn test_format_error_for_user_timeout() {
        let error = ProviderError::Timeout {
            provider: "openai",
            message: "Request timed out after 30s".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Request timeout: Request timed out after 30s");
    }

    #[test]
    fn test_format_error_for_user_network() {
        let error = ProviderError::Network {
            provider: "openai",
            message: "Connection refused".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Network error: Connection refused");
    }

    #[test]
    fn test_format_error_for_user_provider_unavailable() {
        let error = ProviderError::ProviderUnavailable {
            provider: "openai",
            message: "Service is down".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Provider unavailable: Service is down");
    }

    #[test]
    fn test_format_error_for_user_serialization() {
        let error = ProviderError::Serialization {
            provider: "openai",
            message: "Failed to parse JSON".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Parsing error: Failed to parse JSON");
    }

    #[test]
    fn test_format_error_for_user_other() {
        let error = ProviderError::Other {
            provider: "openai",
            message: "Unknown error".to_string(),
        };
        let formatted = ErrorUtils::format_error_for_user(&error);
        assert_eq!(formatted, "Provider error: Unknown error");
    }

    #[test]
    fn test_get_error_category_client_errors() {
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::InvalidRequest {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::ClientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::Authentication {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::ClientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::ModelNotFound {
                provider: "test",
                model: "test".to_string()
            }),
            ErrorCategory::ClientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::QuotaExceeded {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::ClientError
        );
    }

    #[test]
    fn test_get_error_category_transient_errors() {
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::RateLimit {
                provider: "test",
                message: "test".to_string(),
                retry_after: None,
                rpm_limit: None,
                tpm_limit: None,
                current_usage: None,
            }),
            ErrorCategory::TransientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::Network {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::TransientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::Timeout {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::TransientError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::ProviderUnavailable {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::TransientError
        );
    }

    #[test]
    fn test_get_error_category_permanent_errors() {
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::Configuration {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::PermanentError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::NotSupported {
                provider: "test",
                feature: "test".to_string()
            }),
            ErrorCategory::PermanentError
        );
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::NotImplemented {
                provider: "test",
                feature: "test".to_string()
            }),
            ErrorCategory::PermanentError
        );
    }

    #[test]
    fn test_get_error_category_server_error_default() {
        assert_eq!(
            ErrorUtils::get_error_category(&ProviderError::Other {
                provider: "test",
                message: "test".to_string()
            }),
            ErrorCategory::ServerError
        );
    }
}
