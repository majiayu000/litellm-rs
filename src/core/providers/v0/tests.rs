use super::*;

// ==================== V0Config Tests ====================

#[test]
fn test_v0_config_default() {
    let config = V0Config::default();
    assert_eq!(config.api_base, "https://api.v0.dev/v1");
    assert!(config.api_key.is_empty());
    assert_eq!(config.timeout_seconds, 60);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_v0_config_clone() {
    let config = V0Config {
        api_base: "https://custom.api.v0.dev".to_string(),
        api_key: "test-key".to_string(),
        timeout_seconds: 120,
        max_retries: 5,
    };
    let cloned = config.clone();
    assert_eq!(config.api_base, cloned.api_base);
    assert_eq!(config.api_key, cloned.api_key);
    assert_eq!(config.timeout_seconds, cloned.timeout_seconds);
}

#[test]
fn test_v0_config_validate_success() {
    let config = V0Config {
        api_base: "https://api.v0.dev/v1".to_string(),
        api_key: "valid-api-key".to_string(),
        timeout_seconds: 60,
        max_retries: 3,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_v0_config_validate_empty_api_key() {
    let config = V0Config {
        api_base: "https://api.v0.dev/v1".to_string(),
        api_key: String::new(),
        timeout_seconds: 60,
        max_retries: 3,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("API key"));
}

#[test]
fn test_v0_config_validate_empty_api_base() {
    let config = V0Config {
        api_base: String::new(),
        api_key: "valid-key".to_string(),
        timeout_seconds: 60,
        max_retries: 3,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("API base"));
}

#[test]
fn test_v0_config_serialization() {
    let config = V0Config {
        api_base: "https://api.v0.dev/v1".to_string(),
        api_key: "test-key".to_string(),
        timeout_seconds: 60,
        max_retries: 3,
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"api_base\""));
    assert!(json.contains("\"api_key\""));
    assert!(json.contains("\"timeout_seconds\":60"));
}

#[test]
fn test_v0_config_deserialization() {
    let json = r#"{
        "api_base": "https://api.v0.dev/v1",
        "api_key": "test-key",
        "timeout_seconds": 90,
        "max_retries": 5
    }"#;
    let config: V0Config = serde_json::from_str(json).unwrap();
    assert_eq!(config.api_base, "https://api.v0.dev/v1");
    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.timeout_seconds, 90);
    assert_eq!(config.max_retries, 5);
}

// ==================== ProviderConfig Trait Tests ====================

#[test]
fn test_provider_config_api_key() {
    let config = V0Config {
        api_key: "my-key".to_string(),
        ..Default::default()
    };
    assert_eq!(config.api_key(), Some("my-key"));
}

#[test]
fn test_provider_config_api_key_empty() {
    let config = V0Config::default();
    assert_eq!(config.api_key(), None);
}

#[test]
fn test_provider_config_api_base() {
    let config = V0Config {
        api_base: "https://custom.api.com".to_string(),
        ..Default::default()
    };
    assert_eq!(config.api_base(), Some("https://custom.api.com"));
}

#[test]
fn test_provider_config_timeout() {
    let config = V0Config {
        timeout_seconds: 120,
        ..Default::default()
    };
    assert_eq!(config.timeout(), std::time::Duration::from_secs(120));
}

#[test]
fn test_provider_config_max_retries() {
    let config = V0Config {
        max_retries: 5,
        ..Default::default()
    };
    assert_eq!(config.max_retries(), 5);
}

// ==================== V0Model Tests ====================

#[test]
fn test_v0_model_default_id() {
    let model = V0Model::V0Default;
    assert_eq!(model.model_id(), "v0-default");
}

#[test]
fn test_v0_model_custom_id() {
    let model = V0Model::Custom("my-custom-model".to_string());
    assert_eq!(model.model_id(), "my-custom-model");
}

#[test]
fn test_v0_model_supports_function_calling() {
    assert!(V0Model::V0Default.supports_function_calling());
    assert!(V0Model::Custom("test".to_string()).supports_function_calling());
}

#[test]
fn test_v0_model_supports_streaming() {
    assert!(V0Model::V0Default.supports_streaming());
    assert!(V0Model::Custom("test".to_string()).supports_streaming());
}

#[test]
fn test_v0_model_max_context_tokens() {
    assert_eq!(V0Model::V0Default.max_context_tokens(), 32768);
    assert_eq!(
        V0Model::Custom("test".to_string()).max_context_tokens(),
        32768
    );
}

#[test]
fn test_v0_model_clone() {
    let model = V0Model::V0Default;
    let cloned = model.clone();
    assert!(matches!(cloned, V0Model::V0Default));

    let custom = V0Model::Custom("test".to_string());
    let custom_cloned = custom.clone();
    assert!(matches!(custom_cloned, V0Model::Custom(s) if s == "test"));
}

#[test]
fn test_v0_model_serialization() {
    let model = V0Model::V0Default;
    let json = serde_json::to_string(&model).unwrap();
    assert_eq!(json, "\"V0Default\"");

    let custom = V0Model::Custom("my-model".to_string());
    let json = serde_json::to_string(&custom).unwrap();
    assert!(json.contains("Custom"));
    assert!(json.contains("my-model"));
}

// ==================== parse_v0_model Tests ====================

#[test]
fn test_parse_v0_model_default() {
    let model = parse_v0_model("v0");
    assert!(matches!(model, V0Model::V0Default));

    let model = parse_v0_model("v0-default");
    assert!(matches!(model, V0Model::V0Default));
}

#[test]
fn test_parse_v0_model_custom() {
    let model = parse_v0_model("custom-model-123");
    assert!(matches!(model, V0Model::Custom(s) if s == "custom-model-123"));
}

// ==================== V0Provider Tests ====================

#[test]
fn test_v0_provider_new() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new(config);
    assert!(provider.is_ok());
}

#[test]
fn test_v0_provider_new_or_default() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    assert_eq!(provider.config.api_key, "test-key");
}

#[test]
fn test_v0_provider_clone() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let cloned = provider.clone();
    assert_eq!(provider.config.api_key, cloned.config.api_key);
}

#[test]
fn test_v0_provider_get_endpoint() {
    let config = V0Config {
        api_base: "https://api.v0.dev/v1".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);

    assert_eq!(
        provider.get_endpoint("chat/completions"),
        "https://api.v0.dev/v1/chat/completions"
    );
    assert_eq!(
        provider.get_endpoint("/models"),
        "https://api.v0.dev/v1/models"
    );
}

#[test]
fn test_v0_provider_get_endpoint_trailing_slash() {
    let config = V0Config {
        api_base: "https://api.v0.dev/v1/".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);

    assert_eq!(
        provider.get_endpoint("chat/completions"),
        "https://api.v0.dev/v1/chat/completions"
    );
}

#[test]
fn test_v0_provider_create_headers() {
    let config = V0Config {
        api_key: "test-key-123".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let headers = provider.create_headers();

    assert!(headers.contains_key(reqwest::header::AUTHORIZATION));
    assert!(headers.contains_key(reqwest::header::CONTENT_TYPE));
}

// ==================== LLMProvider Trait Tests ====================

#[test]
fn test_v0_provider_name() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    assert_eq!(provider.name(), "v0");
}

#[test]
fn test_v0_provider_capabilities() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let capabilities = provider.capabilities();

    assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
    assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
    assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    assert!(capabilities.contains(&ProviderCapability::FunctionCalling));
}

#[test]
fn test_v0_provider_models() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let models = provider.models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "v0-default"));
}

#[test]
fn test_v0_provider_supported_params() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let params = provider.get_supported_openai_params("v0-default");

    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"tools"));
}

#[test]
fn test_v0_provider_get_error_mapper() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);
    let _mapper = provider.get_error_mapper();
    // Just verify it compiles and returns
}

#[tokio::test]
async fn test_v0_provider_calculate_cost() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);

    // 1000 input tokens at $0.1/1K = $0.1
    // 1000 output tokens at $0.2/1K = $0.2
    // Total = $0.3
    let cost = provider
        .calculate_cost("v0-default", 1000, 1000)
        .await
        .unwrap();
    assert!((cost - 0.3).abs() < 0.001);
}

#[tokio::test]
async fn test_v0_provider_calculate_cost_zero_tokens() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);

    let cost = provider.calculate_cost("v0-default", 0, 0).await.unwrap();
    assert_eq!(cost, 0.0);
}

#[tokio::test]
async fn test_v0_provider_map_openai_params() {
    let config = V0Config {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = V0Provider::new_or_default(config);

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("stream".to_string(), serde_json::json!(true));

    let mapped = provider
        .map_openai_params(params, "v0-default")
        .await
        .unwrap();

    assert_eq!(mapped.get("temperature"), Some(&serde_json::json!(0.7)));
    assert_eq!(mapped.get("stream"), Some(&serde_json::json!(true)));
}
