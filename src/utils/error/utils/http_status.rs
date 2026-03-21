use crate::core::providers::unified_provider::ProviderError;

use super::types::ErrorUtils;

impl ErrorUtils {
    pub fn map_http_status_to_error(
        provider: &'static str,
        status_code: u16,
        message: Option<String>,
    ) -> ProviderError {
        let msg = message.unwrap_or_else(|| format!("HTTP error {}", status_code));

        match status_code {
            400 => ProviderError::InvalidRequest {
                provider,
                message: msg,
            },
            401 => ProviderError::Authentication {
                provider,
                message: msg,
            },
            403 => ProviderError::Authentication {
                provider,
                message: format!("Permission denied: {}", msg),
            },
            404 => ProviderError::ModelNotFound {
                provider,
                model: msg,
            },
            429 => ProviderError::rate_limit_with_retry(provider, msg, Some(60)),
            408 | 504 => ProviderError::Timeout {
                provider,
                message: msg,
            },
            500 | 502 | 503 => ProviderError::ProviderUnavailable {
                provider,
                message: msg,
            },
            _ => ProviderError::Other {
                provider,
                message: msg,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::unified_provider::ProviderError;

    #[test]
    fn test_map_http_status_400_bad_request() {
        let error = ErrorUtils::map_http_status_to_error(
            "custom-provider",
            400,
            Some("Bad request".to_string()),
        );
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "custom-provider");
                assert_eq!(message, "Bad request");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_map_http_status_400_no_message() {
        let error = ErrorUtils::map_http_status_to_error("openai", 400, None);
        match error {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "HTTP error 400");
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_map_http_status_401_unauthorized() {
        let error =
            ErrorUtils::map_http_status_to_error("openai", 401, Some("Unauthorized".to_string()));
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Unauthorized");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_map_http_status_403_forbidden() {
        let error =
            ErrorUtils::map_http_status_to_error("openai", 403, Some("Access denied".to_string()));
        match error {
            ProviderError::Authentication { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Permission denied: Access denied");
            }
            _ => panic!("Expected Authentication error"),
        }
    }

    #[test]
    fn test_map_http_status_404_not_found() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            404,
            Some("Model not found".to_string()),
        );
        match error {
            ProviderError::ModelNotFound { provider, model } => {
                assert_eq!(provider, "openai");
                assert_eq!(model, "Model not found");
            }
            _ => panic!("Expected ModelNotFound error"),
        }
    }

    #[test]
    fn test_map_http_status_429_rate_limit() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            429,
            Some("Too many requests".to_string()),
        );
        match error {
            ProviderError::RateLimit {
                provider,
                message,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Too many requests");
                assert_eq!(retry_after, Some(60));
            }
            _ => panic!("Expected RateLimit error"),
        }
    }

    #[test]
    fn test_map_http_status_408_timeout() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            408,
            Some("Request timeout".to_string()),
        );
        match error {
            ProviderError::Timeout { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Request timeout");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_map_http_status_504_gateway_timeout() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            504,
            Some("Gateway timeout".to_string()),
        );
        match error {
            ProviderError::Timeout { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Gateway timeout");
            }
            _ => panic!("Expected Timeout error"),
        }
    }

    #[test]
    fn test_map_http_status_500_internal_server_error() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            500,
            Some("Internal server error".to_string()),
        );
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Internal server error");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_map_http_status_502_bad_gateway() {
        let error =
            ErrorUtils::map_http_status_to_error("openai", 502, Some("Bad gateway".to_string()));
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Bad gateway");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_map_http_status_503_service_unavailable() {
        let error = ErrorUtils::map_http_status_to_error(
            "openai",
            503,
            Some("Service unavailable".to_string()),
        );
        match error {
            ProviderError::ProviderUnavailable { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "Service unavailable");
            }
            _ => panic!("Expected ProviderUnavailable error"),
        }
    }

    #[test]
    fn test_map_http_status_unknown() {
        let error =
            ErrorUtils::map_http_status_to_error("openai", 418, Some("I'm a teapot".to_string()));
        match error {
            ProviderError::Other { provider, message } => {
                assert_eq!(provider, "openai");
                assert_eq!(message, "I'm a teapot");
            }
            _ => panic!("Expected Other error"),
        }
    }
}
