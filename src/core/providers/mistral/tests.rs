use super::*;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

fn create_test_config() -> MistralConfig {
    MistralConfig {
        api_key: "test_api_key".to_string(),
        ..Default::default()
    }
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_mistral_provider_creation() {
    let config = MistralConfig {
        api_key: "test_key".to_string(),
        ..Default::default()
    };

    let provider = MistralProvider::new(config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(LLMProvider::name(&provider), "mistral");
    assert!(
        provider
            .capabilities()
            .contains(&ProviderCapability::ChatCompletionStream)
    );
}

#[tokio::test]
async fn test_mistral_provider_creation_custom_base() {
    let config = MistralConfig {
        api_key: "test_key".to_string(),
        api_base: "https://custom.mistral.ai/v1".to_string(),
        ..Default::default()
    };

    let provider = MistralProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_mistral_provider_creation_no_api_key() {
    let config = MistralConfig::default();
    let provider = MistralProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_mistral_provider_creation_empty_api_key() {
    let config = MistralConfig {
        api_key: "".to_string(),
        ..Default::default()
    };

    let provider = MistralProvider::new(config).await;
    assert!(provider.is_err());
}

// ==================== Config Validation Tests ====================

#[test]
fn test_mistral_config_validation() {
    let mut config = MistralConfig::default();
    assert!(config.validate().is_err()); // No API key

    config.api_key = "test_key".to_string();
    assert!(config.validate().is_ok());

    config.timeout_seconds = 0;
    assert!(config.validate().is_err()); // Invalid timeout

    config.timeout_seconds = 30;
    config.max_retries = 11;
    assert!(config.validate().is_err()); // Too many retries
}

#[test]
fn test_mistral_config_default() {
    let config = MistralConfig::default();

    assert_eq!(config.api_key, "");
    assert_eq!(config.api_base, "https://api.mistral.ai/v1");
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_mistral_config_provider_config_trait() {
    let config = create_test_config();

    assert_eq!(config.api_key(), Some("test_api_key"));
    assert_eq!(config.api_base(), Some("https://api.mistral.ai/v1"));
    assert_eq!(config.timeout(), std::time::Duration::from_secs(30));
    assert_eq!(config.max_retries(), 3);
}

#[test]
fn test_mistral_config_validation_max_retries_boundary() {
    let mut config = create_test_config();

    config.max_retries = 10;
    assert!(config.validate().is_ok());

    config.max_retries = 11;
    assert!(config.validate().is_err());
}

// ==================== Provider Capabilities Tests ====================

#[tokio::test]
async fn test_provider_name() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    assert_eq!(provider.name(), "mistral");
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
    assert!(caps.contains(&ProviderCapability::Embeddings));
    assert_eq!(caps.len(), 4);
}

#[tokio::test]
async fn test_provider_models() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "mistral-large-latest"));
    assert!(models.iter().any(|m| m.id == "mistral-small-latest"));
    assert!(models.iter().any(|m| m.id == "mistral-medium-latest"));
    assert!(models.iter().any(|m| m.id == "mistral-large-2512"));
    assert!(models.iter().any(|m| m.id == "mistral-small-2603"));
    assert!(models.iter().any(|m| m.id == "mistral-small-2506"));
    assert!(models.iter().any(|m| m.id == "mistral-medium-3-5"));
    assert!(models.iter().any(|m| m.id == "mistral-medium-2508"));
    assert!(models.iter().any(|m| m.id == "mistral-large"));
    assert!(models.iter().any(|m| m.id == "mistral-small"));
    assert!(models.iter().any(|m| m.id == "mistral-medium"));
    assert!(models.iter().any(|m| m.id == "mistral-embed"));
    assert!(models.iter().any(|m| m.id == "magistral-medium-2509"));
    assert!(models.iter().any(|m| m.id == "magistral-medium-latest"));
    assert!(models.iter().any(|m| m.id == "magistral-small-latest"));
    assert!(models.iter().any(|m| m.id == "magistral-medium-1-2"));
    assert!(models.iter().any(|m| m.id == "pixtral-large-2411"));
    assert!(models.iter().any(|m| m.id == "devstral-medium-latest"));
    assert!(models.iter().any(|m| m.id == "devstral-small-latest"));
    assert!(models.iter().any(|m| m.id == "devstral-2512"));
    assert!(models.iter().any(|m| m.id == "devstral-2-2512"));
    assert!(models.iter().any(|m| m.id == "ministral-3b-latest"));
    assert!(models.iter().any(|m| m.id == "ministral-8b-latest"));
    assert!(models.iter().any(|m| m.id == "ministral-14b-latest"));
}

#[tokio::test]
async fn test_current_mistral_alias_metadata() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    let Some(alias) = models.iter().find(|m| m.id == "mistral-small") else {
        panic!("mistral-small alias should be present");
    };
    assert_eq!(
        alias.metadata.get("alias_for"),
        Some(&serde_json::json!("mistral-small-latest"))
    );

    let Some(small_4) = models.iter().find(|m| m.id == "mistral-small-latest") else {
        panic!("mistral-small-latest should be present");
    };
    assert_eq!(small_4.max_context_length, 256000);
    assert!(small_4.supports_multimodal);
    assert_eq!(small_4.input_cost_per_1k_tokens, Some(0.00015));
    assert_eq!(small_4.output_cost_per_1k_tokens, Some(0.0006));

    let Some(medium_alias) = models.iter().find(|m| m.id == "mistral-medium") else {
        panic!("mistral-medium alias should be present");
    };
    assert_eq!(
        medium_alias.metadata.get("alias_for"),
        Some(&serde_json::json!("mistral-medium-latest"))
    );

    let Some(medium_3_5) = models.iter().find(|m| m.id == "mistral-medium-latest") else {
        panic!("mistral-medium-latest should be present");
    };
    assert_eq!(medium_3_5.max_context_length, 256000);
    assert!(medium_3_5.supports_multimodal);
    assert_eq!(medium_3_5.input_cost_per_1k_tokens, Some(0.0015));
    assert_eq!(medium_3_5.output_cost_per_1k_tokens, Some(0.0075));

    let Some(magistral) = models.iter().find(|m| m.id == "magistral-medium-latest") else {
        panic!("magistral-medium-latest should be present");
    };
    assert!(magistral.supports_tools);
    assert!(magistral.supports_multimodal);
}

#[tokio::test]
async fn test_provider_models_have_pricing() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    for model in models {
        assert_eq!(model.provider, "mistral");
        assert!(model.input_cost_per_1k_tokens.is_some());
        assert!(model.output_cost_per_1k_tokens.is_some());
    }
}

#[tokio::test]
async fn test_mistral_small_4_static_pricing_and_alias_boundaries() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    for model_id in [
        "mistral-small-latest",
        "mistral-small-2603",
        "mistral-small-4",
        "mistral-small",
    ] {
        let Some(model) = models.iter().find(|model| model.id == model_id) else {
            panic!("{model_id} should be present in the static Mistral catalog");
        };
        assert_eq!(model.input_cost_per_1k_tokens, Some(0.00015));
        assert_eq!(model.output_cost_per_1k_tokens, Some(0.0006));
    }

    let Some(alias) = models.iter().find(|model| model.id == "mistral-small-4") else {
        panic!("mistral-small-4 alias should be present");
    };
    assert_eq!(
        alias.metadata.get("alias_for"),
        Some(&serde_json::json!("mistral-small-latest"))
    );
    assert_eq!(
        provider.canonical_model_id("mistral/mistral-small-2603"),
        "mistral-small-2603"
    );
    assert_eq!(
        provider.canonical_model_id("mistral/mistral-small-4"),
        "mistral-small-latest"
    );

    let Some(previous_generation) = models.iter().find(|model| model.id == "mistral-small-2506")
    else {
        panic!("mistral-small-2506 should remain in the static Mistral catalog");
    };
    assert_eq!(previous_generation.input_cost_per_1k_tokens, Some(0.0001));
    assert_eq!(previous_generation.output_cost_per_1k_tokens, Some(0.0003));
    assert!(
        !models
            .iter()
            .any(|model| model.id == "mistral-small-2603-preview")
    );
    assert_eq!(
        provider.canonical_model_id("mistral/mistral-small-2603-preview"),
        "mistral-small-2603-preview"
    );
    for (candidate, canonical) in [
        ("mistral/Mistral-Small-2603", "Mistral-Small-2603"),
        ("MISTRAL/mistral-small-2603", "MISTRAL/mistral-small-2603"),
        ("openai/mistral-small-2603", "openai/mistral-small-2603"),
        (
            "mistral/mistral/mistral-small-2603",
            "mistral/mistral-small-2603",
        ),
    ] {
        assert_eq!(provider.canonical_model_id(candidate), canonical);
    }
}

// ==================== Supported Params Tests ====================

#[tokio::test]
async fn test_get_supported_openai_params() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let params = provider.get_supported_openai_params("mistral-large");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"random_seed"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
    assert!(params.contains(&"response_format"));
}

// ==================== Map OpenAI Params Tests ====================

#[tokio::test]
async fn test_map_openai_params_seed_to_random_seed() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("seed".to_string(), serde_json::json!(42));

    let mapped = provider
        .map_openai_params(params, "mistral-large")
        .await
        .unwrap();

    assert!(!mapped.contains_key("seed"));
    assert!(mapped.contains_key("random_seed"));
    assert_eq!(mapped.get("random_seed").unwrap(), &serde_json::json!(42));
}

#[tokio::test]
async fn test_map_openai_params_passthrough() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("max_tokens".to_string(), serde_json::json!(100));
    params.insert("top_p".to_string(), serde_json::json!(0.9));

    let mapped = provider
        .map_openai_params(params, "mistral-large")
        .await
        .unwrap();

    assert_eq!(mapped.get("temperature").unwrap(), &serde_json::json!(0.7));
    assert_eq!(mapped.get("max_tokens").unwrap(), &serde_json::json!(100));
    assert_eq!(mapped.get("top_p").unwrap(), &serde_json::json!(0.9));
}

#[tokio::test]
async fn test_map_openai_params_unsupported_filtered() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("unsupported_param".to_string(), serde_json::json!("value"));
    params.insert("temperature".to_string(), serde_json::json!(0.5));

    let mapped = provider
        .map_openai_params(params, "mistral-large")
        .await
        .unwrap();

    assert!(!mapped.contains_key("unsupported_param"));
    assert!(mapped.contains_key("temperature"));
}

// ==================== Transform Request Tests ====================

#[tokio::test]
async fn test_transform_request_basic() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let request = ChatRequest {
        model: "mistral-large".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let transformed = result.unwrap();
    assert_eq!(transformed["model"], "mistral-large-latest");
    assert!(transformed["messages"].is_array());
}

#[tokio::test]
async fn test_transform_request_rewrites_current_alias() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    let request = ChatRequest {
        model: "mistral-small-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    let Ok(transformed) = result else {
        panic!("transform_request should succeed for mistral-small-4");
    };
    assert_eq!(transformed["model"], "mistral-small-latest");
}

#[tokio::test]
async fn test_transform_request_preserves_versioned_snapshot_model() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    for model in ["mistral-medium-2508", "devstral-2512", "devstral-2-2512"] {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };

        let context = RequestContext::default();
        let result = provider.transform_request(request, context).await;

        let Ok(transformed) = result else {
            panic!("transform_request should succeed for {model}");
        };
        assert_eq!(transformed["model"], model);
    }
}

#[tokio::test]
async fn test_transform_request_strips_mistral_prefix() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    let request = ChatRequest {
        model: "mistral/mistral-small-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    let Ok(transformed) = result else {
        panic!("transform_request should succeed for mistral-prefixed model");
    };
    assert_eq!(transformed["model"], "mistral-small-latest");
}

#[tokio::test]
async fn test_transform_request_with_seed() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let request = ChatRequest {
        model: "mistral-large".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        seed: Some(42),
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let transformed = result.unwrap();
    // Seed should be transformed to random_seed
    assert!(transformed.get("seed").is_none() || transformed["random_seed"].is_number());
}

// ==================== Is Embedding Model Tests ====================

#[tokio::test]
async fn test_is_embedding_model() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    assert!(provider.is_embedding_model("mistral-embed"));
    assert!(provider.is_embedding_model("text-embedding-model"));
    assert!(!provider.is_embedding_model("mistral-large"));
    assert!(!provider.is_embedding_model("mistral-small"));
}

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost_known_model() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("mistral-large", 1000, 500).await;
    assert!(matches!(cost, Ok(v) if v >= 0.0));
}

#[tokio::test]
async fn test_calculate_cost_current_small_model() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    for model in [
        "mistral-small-latest",
        "mistral-small-2603",
        "mistral-small-4",
        "mistral-small",
        "mistral/mistral-small-2603",
        "mistral/mistral-small-4",
    ] {
        let cost = provider.calculate_cost(model, 1000, 500).await;
        assert!(
            matches!(cost, Ok(value) if (value - 0.00045).abs() < f64::EPSILON),
            "{model} should use the central Mistral Small 4 runtime price"
        );
    }

    let central = crate::core::pricing::get_pricing_db()
        .get_model_info("mistral/mistral-small-2603")
        .unwrap_or_else(|| panic!("official Mistral Small 4 row should be centrally priced"));
    assert_eq!(central.input_cost_per_token, Some(0.000_000_15));
    assert_eq!(central.output_cost_per_token, Some(0.000_000_6));
    assert_eq!(
        central
            .extra
            .get("cache_read_input_token_cost")
            .and_then(serde_json::Value::as_f64),
        Some(0.000_000_015)
    );
}

#[tokio::test]
async fn test_calculate_cost_versioned_small_2506_keeps_catalog_rate() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    let cost = provider
        .calculate_cost("mistral-small-2506", 1000, 500)
        .await;

    assert!(matches!(cost, Ok(v) if (v - 0.00025).abs() < f64::EPSILON));
}

#[tokio::test]
async fn test_calculate_cost_current_alias_prices_are_deterministic() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    let large = provider.calculate_cost("mistral-large", 1000, 500).await;
    let small = provider.calculate_cost("mistral-small", 1000, 500).await;

    assert!(matches!(large, Ok(v) if (v - 0.00125).abs() < f64::EPSILON));
    assert!(matches!(small, Ok(v) if (v - 0.00045).abs() < f64::EPSILON));
}

#[tokio::test]
async fn test_calculate_cost_new_aliases_use_canonical_pricing() {
    let Ok(provider) = MistralProvider::new(create_test_config()).await else {
        panic!("mistral test provider should initialize");
    };

    let cases = [
        ("magistral-medium-1-2", "magistral-medium-latest", 0.0045),
        ("magistral-small-1-2", "magistral-small-latest", 0.00125),
    ];

    for (alias, canonical, expected) in cases {
        let Ok(alias_cost) = provider.calculate_cost(alias, 1000, 500).await else {
            panic!("alias cost should calculate for {alias}");
        };
        let Ok(canonical_cost) = provider.calculate_cost(canonical, 1000, 500).await else {
            panic!("canonical cost should calculate for {canonical}");
        };

        assert!((alias_cost - expected).abs() < f64::EPSILON);
        assert_eq!(alias_cost, canonical_cost);
    }
}

#[tokio::test]
async fn test_calculate_cost_embed_model() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("mistral-embed", 1000, 0).await;
    assert!(matches!(cost, Ok(v) if v >= 0.0));
}

#[tokio::test]
async fn test_calculate_cost_unknown_model() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
    assert!(matches!(cost, Ok(v) if v >= 0.0));
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("mistral-large", 0, 0).await;
    assert!(cost.is_ok());
    assert!((cost.unwrap() - 0.0).abs() < 0.0001);
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_authentication() {
    let mapper = MistralErrorMapper;
    let error = mapper.map_http_error(401, "Unauthorized");

    match error {
        ProviderError::Authentication { provider, .. } => {
            assert_eq!(provider, "mistral");
        }
        _ => panic!("Expected Authentication error"),
    }
}

#[test]
fn test_error_mapper_rate_limit() {
    let mapper = MistralErrorMapper;
    let error = mapper.map_http_error(429, "Rate limit exceeded");

    match error {
        ProviderError::RateLimit { provider, .. } => {
            assert_eq!(provider, "mistral");
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_error_mapper_network_error() {
    let mapper = MistralErrorMapper;
    let error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
    let mapped = mapper.map_network_error(&error);

    match mapped {
        ProviderError::Network { provider, .. } => {
            assert_eq!(provider, "mistral");
        }
        _ => panic!("Expected Network error"),
    }
}

#[test]
fn test_error_mapper_parsing_error() {
    let mapper = MistralErrorMapper;
    let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
    let mapped = mapper.map_parsing_error(&error);

    match mapped {
        ProviderError::ResponseParsing { provider, .. } => {
            assert_eq!(provider, "mistral");
        }
        _ => panic!("Expected ResponseParsing error"),
    }
}

#[test]
fn test_error_mapper_timeout_error() {
    let mapper = MistralErrorMapper;
    let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(30));

    match mapped {
        ProviderError::Timeout { provider, .. } => {
            assert_eq!(provider, "mistral");
        }
        _ => panic!("Expected Timeout error"),
    }
}

// ==================== Get Error Mapper Tests ====================

#[tokio::test]
async fn test_get_error_mapper() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let _mapper = provider.get_error_mapper();
    // Just verify it doesn't panic
}

// ==================== Clone/Debug Tests ====================

#[tokio::test]
async fn test_provider_clone() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.models().len(), cloned.models().len());
}

#[tokio::test]
async fn test_provider_debug() {
    let provider = MistralProvider::new(create_test_config()).await.unwrap();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("MistralProvider"));
}

#[test]
fn test_config_clone() {
    let config = create_test_config();
    let cloned = config.clone();

    assert_eq!(config.api_key, cloned.api_key);
    assert_eq!(config.api_base, cloned.api_base);
}

#[test]
fn test_config_debug() {
    let config = create_test_config();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("MistralConfig"));
}

// ==================== Serialization Tests ====================

#[test]
fn test_config_serialization() {
    let config = create_test_config();
    let json = serde_json::to_value(&config).unwrap();

    assert_eq!(json["api_key"], "test_api_key");
    assert_eq!(json["api_base"], "https://api.mistral.ai/v1");
    assert_eq!(json["timeout_seconds"], 30);
    assert_eq!(json["max_retries"], 3);
}

#[test]
fn test_config_deserialization() {
    let json = r#"{
            "api_key": "my_key",
            "api_base": "https://custom.api.com",
            "timeout_seconds": 60,
            "max_retries": 5
        }"#;

    let config: MistralConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.api_key, "my_key");
    assert_eq!(config.api_base, "https://custom.api.com");
    assert_eq!(config.timeout_seconds, 60);
    assert_eq!(config.max_retries, 5);
}
