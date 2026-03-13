//! Error handling integration tests
//!
//! Tests for error types, conversions, and error recovery mechanisms.
//! These tests verify that errors flow correctly through the system.

#[cfg(all(test, feature = "gateway"))]
mod tests {
    use actix_web::ResponseError;
    use litellm_rs::GatewayError;
    use litellm_rs::core::providers::unified_provider::ProviderError;

    // ==================== ProviderError to GatewayError Conversion ====================

    /// Test that authentication errors map correctly
    #[test]
    fn test_auth_error_flow() {
        let provider_err = ProviderError::authentication("openai", "Invalid API key");
        let gateway_err: GatewayError = provider_err.into();

        assert!(matches!(gateway_err, GatewayError::Auth(_)));
        // Check via error_response
        let response = gateway_err.error_response();
        assert_eq!(response.status().as_u16(), 401);
    }

    /// Test that rate limit errors map correctly
    #[test]
    fn test_rate_limit_error_flow() {
        let provider_err = ProviderError::rate_limit("anthropic", Some(60));
        let gateway_err: GatewayError = provider_err.into();

        assert!(matches!(gateway_err, GatewayError::RateLimit { .. }));
        let response = gateway_err.error_response();
        assert_eq!(response.status().as_u16(), 429);
    }

    /// Test that model not found errors map correctly
    #[test]
    fn test_model_not_found_error_flow() {
        let provider_err = ProviderError::model_not_found("openai", "gpt-5-turbo");
        let gateway_err: GatewayError = provider_err.into();

        assert!(matches!(gateway_err, GatewayError::NotFound(_)));
        let response = gateway_err.error_response();
        assert_eq!(response.status().as_u16(), 404);
    }

    /// Test that configuration errors map correctly
    #[test]
    fn test_configuration_error_flow() {
        let provider_err = ProviderError::configuration("azure", "Missing deployment name");
        let gateway_err: GatewayError = provider_err.into();

        assert!(matches!(gateway_err, GatewayError::Config(_)));
        // Config errors are internal server errors
        let response = gateway_err.error_response();
        assert_eq!(response.status().as_u16(), 500);
    }

    /// Test that timeout errors map correctly
    #[test]
    fn test_timeout_error_flow() {
        let provider_err = ProviderError::timeout("openai", "Request timed out after 30s");
        let gateway_err: GatewayError = provider_err.into();

        assert!(matches!(gateway_err, GatewayError::Timeout(_)));
        let response = gateway_err.error_response();
        assert_eq!(response.status().as_u16(), 408); // Request Timeout
    }

    // ==================== ProviderError Properties ====================

    /// Test error retryability
    #[test]
    fn test_error_retryability() {
        // Rate limit should be retryable
        let rate_limit = ProviderError::rate_limit("openai", Some(60));
        assert!(rate_limit.is_retryable());

        // Auth errors should not be retryable
        let auth_err = ProviderError::authentication("openai", "Invalid key");
        assert!(!auth_err.is_retryable());

        // Model not found should not be retryable
        let model_err = ProviderError::model_not_found("openai", "gpt-5");
        assert!(!model_err.is_retryable());

        // Network errors should be retryable
        let network_err = ProviderError::network("openai", "Connection reset");
        assert!(network_err.is_retryable());
    }

    /// Test retry delay suggestions
    #[test]
    fn test_retry_delay_suggestions() {
        // Rate limit with retry_after should suggest that delay
        let rate_limit = ProviderError::rate_limit("openai", Some(60));
        assert_eq!(rate_limit.retry_delay(), Some(60));

        // Network errors should have a short retry delay
        let network_err = ProviderError::network("openai", "Connection reset");
        assert!(network_err.retry_delay().is_some());
    }

    /// Test error provider extraction
    #[test]
    fn test_error_provider_extraction() {
        let err = ProviderError::authentication("anthropic", "Invalid key");
        assert_eq!(err.provider(), "anthropic");

        let err = ProviderError::rate_limit("openai", None);
        assert_eq!(err.provider(), "openai");

        let err = ProviderError::model_not_found("groq", "llama-5");
        assert_eq!(err.provider(), "groq");
    }

    // ==================== Error Context ====================

    /// Test error context chaining
    #[test]
    fn test_error_context() {
        let err = ProviderError::configuration("openai", "Missing API key")
            .with_context("request-123", None);

        let err_string = err.to_string();
        assert!(err_string.contains("Missing API key") || err_string.contains("Configuration"));
    }

    // ==================== GatewayError Properties ====================

    /// Test GatewayError status codes via error_response
    #[test]
    fn test_gateway_error_status_codes() {
        assert_eq!(
            GatewayError::Auth("test".to_string())
                .error_response()
                .status()
                .as_u16(),
            401
        );
        assert_eq!(
            GatewayError::BadRequest("test".to_string())
                .error_response()
                .status()
                .as_u16(),
            400
        );
        assert_eq!(
            GatewayError::NotFound("test".to_string())
                .error_response()
                .status()
                .as_u16(),
            404
        );
        assert_eq!(
            GatewayError::rate_limit("test")
                .error_response()
                .status()
                .as_u16(),
            429
        );
        assert_eq!(
            GatewayError::Timeout("test".to_string())
                .error_response()
                .status()
                .as_u16(),
            408
        );
        assert_eq!(
            GatewayError::Internal("test".to_string())
                .error_response()
                .status()
                .as_u16(),
            500
        );
    }

    /// Test GatewayError display
    #[test]
    fn test_gateway_error_display() {
        let err = GatewayError::Auth("Invalid credentials".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Invalid credentials") || display.contains("Auth"));
    }

    // ==================== Error Recovery ====================

    /// Test that provider unavailable errors are retryable
    #[test]
    fn test_provider_unavailable_is_retryable() {
        let err = ProviderError::ProviderUnavailable {
            provider: "openai",
            message: "Service temporarily unavailable".to_string(),
        };
        assert!(err.is_retryable());
    }

    /// Test that timeout errors are retryable
    #[test]
    fn test_timeout_is_retryable() {
        let err = ProviderError::timeout("openai", "Request timed out");
        assert!(err.is_retryable());
    }

    // ==================== API Error Mapping ====================

    /// Test HTTP status code to error type mapping
    #[test]
    fn test_api_error_status_mapping() {
        // 401 -> Auth
        let err = ProviderError::ApiError {
            provider: "openai",
            status: 401,
            message: "Unauthorized".to_string(),
        };
        let gateway: GatewayError = err.into();
        assert!(matches!(gateway, GatewayError::Auth(_)));

        // 404 -> NotFound
        let err = ProviderError::ApiError {
            provider: "openai",
            status: 404,
            message: "Not found".to_string(),
        };
        let gateway: GatewayError = err.into();
        assert!(matches!(gateway, GatewayError::NotFound(_)));

        // 429 -> RateLimit
        let err = ProviderError::ApiError {
            provider: "openai",
            status: 429,
            message: "Too many requests".to_string(),
        };
        let gateway: GatewayError = err.into();
        assert!(matches!(gateway, GatewayError::RateLimit { .. }));

        // 400 -> BadRequest
        let err = ProviderError::ApiError {
            provider: "openai",
            status: 400,
            message: "Bad request".to_string(),
        };
        let gateway: GatewayError = err.into();
        assert!(matches!(gateway, GatewayError::BadRequest(_)));

        // 500 -> Internal
        let err = ProviderError::ApiError {
            provider: "openai",
            status: 500,
            message: "Internal error".to_string(),
        };
        let gateway: GatewayError = err.into();
        assert!(matches!(gateway, GatewayError::Internal(_)));
    }
}
