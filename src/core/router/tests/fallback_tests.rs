//! Fallback configuration tests

use crate::core::providers::unified_provider::ProviderError;
use crate::core::router::fallback::{FallbackConfig, FallbackType};
use crate::core::router::unified::Router;

#[test]
fn test_fallback_config_builder() {
    let config = FallbackConfig::new()
        .add_general(
            "gpt-4",
            vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string()],
        )
        .add_context_window(
            "gpt-4",
            vec!["gpt-4-32k".to_string(), "claude-3-opus".to_string()],
        )
        .add_content_policy("gpt-4", vec!["claude-3-opus".to_string()])
        .add_rate_limit("gpt-4", vec!["azure-gpt4".to_string()]);

    let general = config.get_fallbacks_for_type("gpt-4", FallbackType::General);
    assert_eq!(general.len(), 2);
    assert_eq!(general[0], "gpt-4-turbo");
    assert_eq!(general[1], "gpt-3.5-turbo");

    let context = config.get_fallbacks_for_type("gpt-4", FallbackType::ContextWindow);
    assert_eq!(context.len(), 2);
    assert_eq!(context[0], "gpt-4-32k");
    assert_eq!(context[1], "claude-3-opus");

    let content = config.get_fallbacks_for_type("gpt-4", FallbackType::ContentPolicy);
    assert_eq!(content.len(), 1);
    assert_eq!(content[0], "claude-3-opus");

    let rate_limit = config.get_fallbacks_for_type("gpt-4", FallbackType::RateLimit);
    assert_eq!(rate_limit.len(), 1);
    assert_eq!(rate_limit[0], "azure-gpt4");
}

#[tokio::test]
async fn test_get_fallbacks_general() {
    let fallback_config = FallbackConfig::new().add_general(
        "gpt-4",
        vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string()],
    );

    let router = Router::default().with_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 2);
    assert_eq!(fallbacks[0], "gpt-4-turbo");
    assert_eq!(fallbacks[1], "gpt-3.5-turbo");
}

#[tokio::test]
async fn test_get_fallbacks_context_window() {
    let fallback_config = FallbackConfig::new().add_context_window(
        "gpt-4",
        vec!["gpt-4-32k".to_string(), "claude-3-opus".to_string()],
    );

    let router = Router::default().with_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::ContextWindow);
    assert_eq!(fallbacks.len(), 2);
    assert_eq!(fallbacks[0], "gpt-4-32k");
    assert_eq!(fallbacks[1], "claude-3-opus");
}

#[tokio::test]
async fn test_get_fallbacks_empty() {
    let router = Router::default();

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 0);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::ContextWindow);
    assert_eq!(fallbacks.len(), 0);
}

#[tokio::test]
async fn test_get_fallbacks_falls_back_to_general() {
    let fallback_config = FallbackConfig::new().add_general(
        "gpt-4",
        vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string()],
    );

    let router = Router::default().with_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::ContextWindow);
    assert_eq!(fallbacks.len(), 2);
    assert_eq!(fallbacks[0], "gpt-4-turbo");
    assert_eq!(fallbacks[1], "gpt-3.5-turbo");

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::ContentPolicy);
    assert_eq!(fallbacks.len(), 2);
}

#[tokio::test]
async fn test_get_fallbacks_with_alias() {
    let fallback_config =
        FallbackConfig::new().add_general("gpt-4", vec!["gpt-4-turbo".to_string()]);

    let router = Router::default().with_fallback_config(fallback_config);
    router.add_model_alias("gpt4", "gpt-4").unwrap();

    let fallbacks = router.get_fallbacks("gpt4", FallbackType::General);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "gpt-4-turbo");
}

#[tokio::test]
async fn test_get_fallbacks_specific_overrides_general() {
    let fallback_config = FallbackConfig::new()
        .add_general("gpt-4", vec!["gpt-4-turbo".to_string()])
        .add_context_window(
            "gpt-4",
            vec!["gpt-4-32k".to_string(), "claude-3-opus".to_string()],
        );

    let router = Router::default().with_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::ContextWindow);
    assert_eq!(fallbacks.len(), 2);
    assert_eq!(fallbacks[0], "gpt-4-32k");
    assert_eq!(fallbacks[1], "claude-3-opus");

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "gpt-4-turbo");
}

#[test]
fn test_infer_fallback_type_context_window() {
    let error = ProviderError::ContextLengthExceeded {
        provider: "test",
        max: 4096,
        actual: 5000,
    };
    let fallback_type = Router::infer_fallback_type(&error);
    assert_eq!(fallback_type, FallbackType::ContextWindow);
}

#[test]
fn test_infer_fallback_type_content_policy() {
    let error = ProviderError::ContentFiltered {
        provider: "test",
        reason: "Unsafe content detected".to_string(),
        policy_violations: Some(vec!["violence".to_string()]),
        potentially_retryable: Some(false),
    };
    let fallback_type = Router::infer_fallback_type(&error);
    assert_eq!(fallback_type, FallbackType::ContentPolicy);
}

#[test]
fn test_infer_fallback_type_rate_limit() {
    let error = ProviderError::rate_limit("test", Some(60));
    let fallback_type = Router::infer_fallback_type(&error);
    assert_eq!(fallback_type, FallbackType::RateLimit);
}

#[test]
fn test_infer_fallback_type_general() {
    let error = ProviderError::network("test", "Connection failed");
    assert_eq!(Router::infer_fallback_type(&error), FallbackType::General);

    let error = ProviderError::authentication("test", "Invalid API key");
    assert_eq!(Router::infer_fallback_type(&error), FallbackType::General);

    let error = ProviderError::timeout("test", "Request timed out");
    assert_eq!(Router::infer_fallback_type(&error), FallbackType::General);
}

#[tokio::test]
async fn test_get_models_with_fallbacks() {
    let fallback_config = FallbackConfig::new().add_general(
        "gpt-4",
        vec!["gpt-4-turbo".to_string(), "gpt-3.5-turbo".to_string()],
    );

    let router = Router::default().with_fallback_config(fallback_config);

    let models = router.get_models_with_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(models.len(), 3);
    assert_eq!(models[0], "gpt-4");
    assert_eq!(models[1], "gpt-4-turbo");
    assert_eq!(models[2], "gpt-3.5-turbo");
}

#[tokio::test]
async fn test_get_models_with_fallbacks_no_fallbacks() {
    let router = Router::default();

    let models = router.get_models_with_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(models.len(), 1);
    assert_eq!(models[0], "gpt-4");
}

#[tokio::test]
async fn test_set_fallback_config_runtime() {
    let router = Router::default();

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 0);

    let fallback_config =
        FallbackConfig::new().add_general("gpt-4", vec!["gpt-4-turbo".to_string()]);
    router.set_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "gpt-4-turbo");
}

#[test]
fn test_fallback_type_equality() {
    assert_eq!(FallbackType::General, FallbackType::General);
    assert_eq!(FallbackType::ContextWindow, FallbackType::ContextWindow);
    assert_eq!(FallbackType::ContentPolicy, FallbackType::ContentPolicy);
    assert_eq!(FallbackType::RateLimit, FallbackType::RateLimit);

    assert_ne!(FallbackType::General, FallbackType::ContextWindow);
    assert_ne!(FallbackType::ContentPolicy, FallbackType::RateLimit);
}

#[tokio::test]
async fn test_fallback_config_multiple_models() {
    let fallback_config = FallbackConfig::new()
        .add_general("gpt-4", vec!["gpt-4-turbo".to_string()])
        .add_general("gpt-3.5-turbo", vec!["gpt-3.5-turbo-16k".to_string()])
        .add_context_window("claude-3-opus", vec!["claude-3-opus-200k".to_string()]);

    let router = Router::default().with_fallback_config(fallback_config);

    let fallbacks = router.get_fallbacks("gpt-4", FallbackType::General);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "gpt-4-turbo");

    let fallbacks = router.get_fallbacks("gpt-3.5-turbo", FallbackType::General);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "gpt-3.5-turbo-16k");

    let fallbacks = router.get_fallbacks("claude-3-opus", FallbackType::ContextWindow);
    assert_eq!(fallbacks.len(), 1);
    assert_eq!(fallbacks[0], "claude-3-opus-200k");
}

// ==================== Fallback Cycle Detection Tests ====================

#[test]
fn test_fallback_validate_no_cycle() {
    let config = FallbackConfig::new()
        .add_general("gpt-4", vec!["gpt-3.5-turbo".to_string()])
        .add_general("gpt-3.5-turbo", vec!["claude-3".to_string()]);

    assert!(config.validate().is_ok());
}

#[test]
fn test_fallback_validate_direct_cycle() {
    // A -> B and B -> A
    let config = FallbackConfig::new()
        .add_general("a", vec!["b".to_string()])
        .add_general("b", vec!["a".to_string()]);

    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn test_fallback_validate_transitive_cycle() {
    // A -> B -> C -> A
    let config = FallbackConfig::new()
        .add_general("a", vec!["b".to_string()])
        .add_general("b", vec!["c".to_string()])
        .add_general("c", vec!["a".to_string()]);

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_fallback_validate_self_cycle() {
    let config = FallbackConfig::new().add_general("a", vec!["a".to_string()]);

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_fallback_validate_empty_config() {
    let config = FallbackConfig::new();
    assert!(config.validate().is_ok());
}

#[test]
fn test_fallback_validate_cycle_in_one_type_only() {
    // Cycle in context_window, but not in general
    let config = FallbackConfig::new()
        .add_general("a", vec!["b".to_string()])
        .add_context_window("x", vec!["y".to_string()])
        .add_context_window("y", vec!["x".to_string()]);

    let result = config.validate();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.contains("context_window")));
}
