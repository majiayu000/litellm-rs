use super::*;
use crate::core::providers::unified_provider::ProviderError;
use serde_json::json;

#[test]
fn test_require_config_str_success() {
    let config = json!({
        "api_key": "sk-test-key",
        "base_url": "https://api.example.com"
    });

    let result = require_config_str(&config, "api_key", "openai");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "sk-test-key");
}

#[test]
fn test_require_config_str_missing() {
    let config = json!({
        "base_url": "https://api.example.com"
    });

    let result = require_config_str(&config, "api_key", "openai");
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, ProviderError::Configuration { .. }));
}

#[test]
fn test_require_config_str_wrong_type() {
    let config = json!({
        "api_key": 12345
    });

    let result = require_config_str(&config, "api_key", "openai");
    assert!(result.is_err());
}

#[test]
fn test_get_config_str_some() {
    let config = json!({
        "api_key": "sk-test-key"
    });

    let result = get_config_str(&config, "api_key");
    assert_eq!(result, Some("sk-test-key"));
}

#[test]
fn test_get_config_str_none() {
    let config = json!({});

    let result = get_config_str(&config, "api_key");
    assert_eq!(result, None);
}

#[test]
fn test_require_config_u64_success() {
    let config = json!({
        "timeout": 30,
        "max_retries": 5
    });

    let result = require_config_u64(&config, "timeout", "openai");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 30);
}

#[test]
fn test_require_config_u64_missing() {
    let config = json!({});

    let result = require_config_u64(&config, "timeout", "openai");
    assert!(result.is_err());
}

#[test]
fn test_get_config_u64_or_present() {
    let config = json!({
        "timeout": 60
    });

    let result = get_config_u64_or(&config, "timeout", 30);
    assert_eq!(result, 60);
}

#[test]
fn test_get_config_u64_or_default() {
    let config = json!({});

    let result = get_config_u64_or(&config, "timeout", 30);
    assert_eq!(result, 30);
}

#[test]
fn test_require_config_bool_success() {
    let config = json!({
        "enable_streaming": true
    });

    let result = require_config_bool(&config, "enable_streaming", "openai");
    assert!(result.is_ok());
    assert!(result.unwrap());
}

#[test]
fn test_require_config_bool_missing() {
    let config = json!({});

    let result = require_config_bool(&config, "enable_streaming", "openai");
    assert!(result.is_err());
}

#[test]
fn test_get_config_bool_or_present() {
    let config = json!({
        "debug": true
    });

    let result = get_config_bool_or(&config, "debug", false);
    assert!(result);
}

#[test]
fn test_get_config_bool_or_default() {
    let config = json!({});

    let result = get_config_bool_or(&config, "debug", false);
    assert!(!result);
}

#[test]
fn test_nested_config_extraction() {
    let config = json!({
        "provider": {
            "api_key": "nested-key"
        }
    });

    // Direct extraction from nested won't work - need to get nested first
    let nested = config.get("provider").unwrap();
    let result = require_config_str(nested, "api_key", "openai");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "nested-key");
}

#[test]
fn test_null_value_treated_as_missing() {
    let config = json!({
        "api_key": null
    });

    let result = require_config_str(&config, "api_key", "openai");
    assert!(result.is_err());
}

#[test]
fn test_empty_string_is_valid() {
    let config = json!({
        "api_key": ""
    });

    let result = require_config_str(&config, "api_key", "openai");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[test]
fn test_error_message_contains_key() {
    let config = json!({});

    let result = require_config_str(&config, "my_special_key", "test_provider");
    let err = result.unwrap_err();

    match err {
        ProviderError::Configuration { message, provider } => {
            assert!(message.contains("my_special_key"));
            assert_eq!(provider, "test_provider");
        }
        _ => panic!("Expected Configuration error"),
    }
}
