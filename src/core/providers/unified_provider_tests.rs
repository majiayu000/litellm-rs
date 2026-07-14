//! Tests for ProviderError and ContextualError
//!
//! This module contains comprehensive unit tests for error handling.

#[cfg(test)]
mod contextual_error_tests {
    use crate::core::providers::unified_provider::ProviderError;

    #[test]
    fn test_contextual_error_display() {
        let err = ProviderError::network("openai", "Connection refused")
            .with_context("req-12345", Some("gpt-4"));

        let display = format!("{}", err);
        assert!(display.contains("req-12345"));
        assert!(display.contains("Connection refused"));
        assert!(display.contains("gpt-4"));
    }

    #[test]
    fn test_contextual_error_methods() {
        let err = ProviderError::rate_limit("anthropic", Some(60)).with_context("req-abc", None);

        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(60));
        assert_eq!(err.http_status(), 429);
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.request_id(), "req-abc");
        assert!(err.model().is_none());
    }

    #[test]
    fn test_to_error_response() {
        let err = ProviderError::authentication("openai", "Invalid API key")
            .with_context("req-xyz", Some("gpt-4-turbo"));

        let response = err.to_error_response();
        assert_eq!(response["error"]["request_id"], "req-xyz");
        assert_eq!(response["error"]["model"], "gpt-4-turbo");
        assert_eq!(response["error"]["code"], 401);
        assert_eq!(response["error"]["provider"], "openai");
    }
}

#[cfg(test)]
mod provider_error_tests {
    use crate::core::providers::bedrock::BedrockErrorMapper;
    use crate::core::providers::unified_provider::{ProviderError, provider_http_error_facts};

    // ==================== Factory Method Tests ====================

    #[test]
    fn test_authentication_factory() {
        let err = ProviderError::authentication("openai", "Invalid API key");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 401);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_rate_limit_factory() {
        let err = ProviderError::rate_limit("anthropic", Some(60));
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 429);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(60));
    }

    #[test]
    fn test_rate_limit_factory_no_retry() {
        let err = ProviderError::rate_limit("anthropic", None);
        assert!(err.retry_delay().is_none());
    }

    #[test]
    fn test_rate_limit_with_limits() {
        let err = ProviderError::rate_limit_with_limits(
            "openai",
            Some(60),
            Some(100),
            Some(10000),
            Some(0.9),
        );
        assert_eq!(err.provider(), "openai");
        assert!(err.is_retryable());
    }

    #[test]
    fn test_quota_exceeded_factory() {
        let err = ProviderError::quota_exceeded("vertex_ai", "Monthly quota exceeded");
        assert_eq!(err.provider(), "vertex_ai");
        assert_eq!(err.http_status(), 402);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_model_not_found_factory() {
        let err = ProviderError::model_not_found("openai", "gpt-5");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 404);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_invalid_request_factory() {
        let err = ProviderError::invalid_request("anthropic", "Missing messages");
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 400);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_network_factory() {
        let err = ProviderError::network("openai", "Connection refused");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 502);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(1));
    }

    #[test]
    fn test_provider_unavailable_factory() {
        let err = ProviderError::provider_unavailable("anthropic", "Service down");
        assert_eq!(err.provider(), "anthropic");
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(5));
    }

    #[test]
    fn test_not_supported_factory() {
        let err = ProviderError::not_supported("openai", "vision");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 501);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_not_implemented_factory() {
        let err = ProviderError::not_implemented("anthropic", "streaming");
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 501);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_configuration_factory() {
        let err = ProviderError::configuration("openai", "Missing API key");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 500);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_serialization_factory() {
        let err = ProviderError::serialization("anthropic", "Invalid JSON");
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 500);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_timeout_factory() {
        let err = ProviderError::timeout("openai", "Request timed out after 30s");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 504);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(1));
    }

    // ==================== Enhanced Error Variant Tests ====================

    #[test]
    fn test_context_length_exceeded() {
        let err = ProviderError::context_length_exceeded("openai", 4096, 5000);
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 400);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_api_error() {
        let err = ProviderError::api_error("anthropic", 500, "Internal server error");
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 500);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_api_error_429() {
        let err = ProviderError::api_error("openai", 429, "Rate limited");
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(60));
    }

    #[test]
    fn test_api_error_400() {
        let err = ProviderError::api_error("openai", 400, "Bad request");
        assert!(!err.is_retryable());
        assert!(err.retry_delay().is_none());
    }

    #[test]
    fn test_token_limit_exceeded() {
        let err = ProviderError::token_limit_exceeded("openai", "Max tokens exceeded");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 400);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_feature_disabled() {
        let err = ProviderError::feature_disabled("vertex_ai", "code_execution");
        assert_eq!(err.provider(), "vertex_ai");
        assert_eq!(err.http_status(), 501);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_deployment_error() {
        let err = ProviderError::deployment_error("my-deployment", "Deployment not found");
        assert_eq!(err.provider(), "azure");
        assert_eq!(err.http_status(), 404);
        assert!(err.is_retryable());
    }

    #[test]
    fn test_response_parsing() {
        let err = ProviderError::response_parsing("openai", "Invalid JSON response");
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 502);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_routing_error() {
        let err = ProviderError::routing_error(
            "openrouter",
            vec!["openai".to_string(), "anthropic".to_string()],
            "All providers failed",
        );
        assert_eq!(err.provider(), "openrouter");
        assert_eq!(err.http_status(), 503);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_transformation_error() {
        let err = ProviderError::transformation_error(
            "openrouter",
            "openai",
            "anthropic",
            "Invalid message format",
        );
        assert_eq!(err.provider(), "openrouter");
        assert_eq!(err.http_status(), 500);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_content_filtered_not_retryable() {
        let err = ProviderError::content_filtered("openai", "Content policy violation", None, None);
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 400);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_content_filtered_retryable() {
        let err = ProviderError::content_filtered(
            "openai",
            "Content might be inappropriate",
            Some(vec!["violence".to_string()]),
            Some(true),
        );
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(10));
    }

    #[test]
    fn test_cancelled() {
        let err = ProviderError::cancelled(
            "openai",
            "chat_completion",
            Some("User cancelled".to_string()),
        );
        assert_eq!(err.provider(), "openai");
        assert_eq!(err.http_status(), 499);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_streaming_error() {
        let err = ProviderError::streaming_error(
            "anthropic",
            "chat",
            Some(100),
            Some("last chunk".to_string()),
            "Stream interrupted",
        );
        assert_eq!(err.provider(), "anthropic");
        assert_eq!(err.http_status(), 502);
        assert!(err.is_retryable());
        assert_eq!(err.retry_delay(), Some(2));
    }

    #[test]
    fn test_other_error() {
        let err = ProviderError::other("unknown", "Unknown error occurred");
        assert_eq!(err.provider(), "unknown");
        assert_eq!(err.http_status(), 502);
        assert!(!err.is_retryable());
    }

    // ==================== Error Type String Tests ====================

    #[test]
    fn test_error_type_strings() {
        use crate::core::types::errors::ProviderErrorTrait;

        assert_eq!(
            ProviderError::authentication("a", "b").error_type(),
            "authentication"
        );
        assert_eq!(
            ProviderError::rate_limit("a", None).error_type(),
            "rate_limit"
        );
        assert_eq!(
            ProviderError::quota_exceeded("a", "b").error_type(),
            "quota_exceeded"
        );
        assert_eq!(
            ProviderError::model_not_found("a", "b").error_type(),
            "model_not_found"
        );
        assert_eq!(ProviderError::network("a", "b").error_type(), "network");
        assert_eq!(ProviderError::timeout("a", "b").error_type(), "timeout");
    }

    // ==================== Display Tests ====================

    #[test]
    fn test_error_display_authentication() {
        let err = ProviderError::authentication("openai", "Invalid API key");
        let display = format!("{}", err);
        assert!(display.contains("openai"));
        assert!(display.contains("Invalid API key"));
    }

    #[test]
    fn test_error_display_rate_limit() {
        let err = ProviderError::rate_limit("anthropic", Some(60));
        let display = format!("{}", err);
        assert!(display.contains("anthropic"));
        assert!(display.contains("Rate limit"));
    }

    #[test]
    fn test_error_display_model_not_found() {
        let err = ProviderError::model_not_found("openai", "gpt-5");
        let display = format!("{}", err);
        assert!(display.contains("openai"));
        assert!(display.contains("gpt-5"));
    }

    // ==================== Conversion Tests ====================

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let err: ProviderError = json_err.into();
        assert_eq!(err.provider(), "unknown");
    }

    // ==================== HTTP Status Mapping Tests ====================

    #[test]
    fn test_http_status_mapping() {
        assert_eq!(ProviderError::authentication("a", "b").http_status(), 401);
        assert_eq!(ProviderError::rate_limit("a", None).http_status(), 429);
        assert_eq!(ProviderError::quota_exceeded("a", "b").http_status(), 402);
        assert_eq!(ProviderError::model_not_found("a", "b").http_status(), 404);
        assert_eq!(ProviderError::invalid_request("a", "b").http_status(), 400);
        assert_eq!(ProviderError::not_supported("a", "b").http_status(), 501);
        assert_eq!(ProviderError::not_implemented("a", "b").http_status(), 501);
        assert_eq!(ProviderError::network("a", "b").http_status(), 502);
        assert_eq!(ProviderError::serialization("a", "b").http_status(), 500);
    }

    #[test]
    fn test_http_status_delegates_to_canonical_facts() {
        let cases = vec![
            ProviderError::not_supported("a", "b"),
            ProviderError::feature_disabled("a", "b"),
            ProviderError::network("a", "b"),
            ProviderError::timeout("a", "b"),
            ProviderError::token_limit_exceeded("a", "b"),
            ProviderError::streaming_error("a", "b", None, None, "c"),
            ProviderError::other("a", "b"),
        ];

        for error in cases {
            assert_eq!(
                error.http_status(),
                provider_http_error_facts(&error).status
            );
        }
    }

    #[cfg(feature = "gateway")]
    #[test]
    fn test_gateway_provider_mapping_uses_canonical_provider_facts() {
        use crate::utils::error::gateway_error::{GatewayError, gateway_http_error_facts};

        let provider_error = ProviderError::feature_disabled("a", "b");
        let core_facts = provider_http_error_facts(&provider_error);
        let gateway_facts = gateway_http_error_facts(&GatewayError::Provider(provider_error));

        assert_eq!(gateway_facts.status.as_u16(), core_facts.status);
        assert_eq!(gateway_facts.gateway_code, core_facts.gateway_code);
        assert_eq!(gateway_facts.openai_code, core_facts.openai_code);
    }

    // ==================== Retryable Tests ====================

    #[test]
    fn test_retryable_errors() {
        assert!(ProviderError::network("a", "b").is_retryable());
        assert!(ProviderError::timeout("a", "b").is_retryable());
        assert!(ProviderError::rate_limit("a", None).is_retryable());
        assert!(ProviderError::provider_unavailable("a", "b").is_retryable());
        assert!(ProviderError::deployment_error("a", "b").is_retryable());
        assert!(ProviderError::streaming_error("a", "b", None, None, "c").is_retryable());
        let failed_dependency =
            BedrockErrorMapper::map_service_error("DependencyFailedException", "not ready")
                .expect("modeled Bedrock service error");
        assert!(failed_dependency.is_retryable());
        assert_eq!(failed_dependency.retry_delay(), Some(3));
        let ordinary_failed_dependency = ProviderError::api_error(
            "bedrock",
            424,
            "ModelNotReadyException: misleading ordinary HTTP message",
        );
        assert!(!ordinary_failed_dependency.is_retryable());
        assert_eq!(ordinary_failed_dependency.retry_delay(), None);
        let unrelated_failed_dependency = ProviderError::api_error("custom_httpx", 424, "failed");
        assert!(!unrelated_failed_dependency.is_retryable());
        assert_eq!(unrelated_failed_dependency.retry_delay(), None);
    }

    #[test]
    fn test_non_retryable_errors() {
        assert!(!ProviderError::authentication("a", "b").is_retryable());
        assert!(!ProviderError::quota_exceeded("a", "b").is_retryable());
        assert!(!ProviderError::model_not_found("a", "b").is_retryable());
        assert!(!ProviderError::invalid_request("a", "b").is_retryable());
        assert!(!ProviderError::not_supported("a", "b").is_retryable());
        assert!(!ProviderError::configuration("a", "b").is_retryable());
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_error_clone() {
        let err = ProviderError::authentication("openai", "Invalid key");
        let cloned = err.clone();
        assert_eq!(err.provider(), cloned.provider());
    }

    // ==================== Debug Tests ====================

    #[test]
    fn test_error_debug() {
        let err = ProviderError::authentication("openai", "Invalid key");
        let debug = format!("{:?}", err);
        assert!(debug.contains("Authentication"));
    }
}
