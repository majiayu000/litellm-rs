use super::*;

fn create_test_config() -> JinaConfig {
    JinaConfig {
        api_key: "test_api_key".to_string(),
        ..Default::default()
    }
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_jina_provider_creation() {
    let config = JinaConfig {
        api_key: "test_key".to_string(),
        ..Default::default()
    };

    let provider = JinaProvider::new(config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(LLMProvider::name(&provider), "jina");
    assert!(
        provider
            .capabilities()
            .contains(&ProviderCapability::Embeddings)
    );
}

#[tokio::test]
async fn test_jina_provider_creation_custom_base() {
    let config = JinaConfig {
        api_key: "test_key".to_string(),
        api_base: "https://custom.jina.ai/v1".to_string(),
        ..Default::default()
    };

    let provider = JinaProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_jina_provider_creation_no_api_key() {
    let config = JinaConfig::default();
    let provider = JinaProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_jina_provider_creation_empty_api_key() {
    let config = JinaConfig {
        api_key: "".to_string(),
        ..Default::default()
    };

    let provider = JinaProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_jina_with_api_key() {
    let provider = JinaProvider::with_api_key("test_key").await;
    assert!(provider.is_ok());
}

// ==================== Config Validation Tests ====================

#[test]
fn test_jina_config_validation() {
    let mut config = JinaConfig::default();
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
fn test_jina_config_default() {
    let config = JinaConfig::default();

    assert_eq!(config.api_key, "");
    assert_eq!(config.api_base, "https://api.jina.ai/v1");
    assert_eq!(config.timeout_seconds, 30);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_jina_config_provider_config_trait() {
    let config = create_test_config();

    assert_eq!(config.api_key(), Some("test_api_key"));
    assert_eq!(config.api_base(), Some("https://api.jina.ai/v1"));
    assert_eq!(config.timeout(), std::time::Duration::from_secs(30));
    assert_eq!(config.max_retries(), 3);
}

// ==================== Provider Capabilities Tests ====================

#[tokio::test]
async fn test_provider_name() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    assert_eq!(provider.name(), "jina");
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::Embeddings));
    assert_eq!(caps.len(), 1);
}

#[tokio::test]
async fn test_provider_models() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "jina-embeddings-v3"));
    assert!(models.iter().any(|m| m.id == "jina-embeddings-v2-base-en"));
    assert!(
        models
            .iter()
            .any(|m| m.id == "jina-reranker-v2-base-multilingual")
    );
}

#[tokio::test]
async fn test_provider_models_have_pricing() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let models = provider.models();

    for model in models {
        assert_eq!(model.provider, "jina");
        assert!(model.input_cost_per_1k_tokens.is_some());
        assert!(model.output_cost_per_1k_tokens.is_some());
    }
}

// ==================== Model Type Detection Tests ====================

#[tokio::test]
async fn test_is_reranker_model() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    assert!(provider.is_reranker_model("jina-reranker-v2-base-multilingual"));
    assert!(provider.is_reranker_model("jina-colbert-v2"));
    assert!(!provider.is_reranker_model("jina-embeddings-v3"));
}

#[tokio::test]
async fn test_is_embedding_model() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    assert!(provider.is_embedding_model("jina-embeddings-v3"));
    assert!(provider.is_embedding_model("jina-embeddings-v2-base-en"));
    assert!(!provider.is_embedding_model("jina-reranker-v2-base-multilingual"));
}

// ==================== Base64 Detection Tests ====================

#[test]
fn test_is_base64_encoded() {
    // Data URL format
    assert!(JinaProvider::is_base64_encoded(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="
    ));

    // Regular text should not be detected as base64
    assert!(!JinaProvider::is_base64_encoded("Hello, world!"));
    assert!(!JinaProvider::is_base64_encoded(
        "This is a normal text string"
    ));
}

// ==================== Supported Params Tests ====================

#[tokio::test]
async fn test_get_supported_openai_params() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let params = provider.get_supported_openai_params("jina-embeddings-v3");

    assert!(params.contains(&"dimensions"));
    assert_eq!(params.len(), 1);
}

// ==================== Map OpenAI Params Tests ====================

#[tokio::test]
async fn test_map_openai_params() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("dimensions".to_string(), serde_json::json!(512));
    params.insert("unsupported".to_string(), serde_json::json!("value"));

    let mapped = provider
        .map_openai_params(params, "jina-embeddings-v3")
        .await
        .unwrap();

    assert!(mapped.contains_key("dimensions"));
    assert!(!mapped.contains_key("unsupported"));
    assert_eq!(mapped.get("dimensions").unwrap(), &serde_json::json!(512));
}

// ==================== Chat Not Supported Tests ====================

#[tokio::test]
async fn test_chat_completion_not_supported() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let request = ChatRequest {
        model: "jina-embeddings-v3".to_string(),
        messages: vec![],
        ..Default::default()
    };

    let result = provider
        .chat_completion(request, RequestContext::default())
        .await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ProviderError::NotSupported { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected NotSupported error"),
    }
}

#[tokio::test]
async fn test_chat_completion_stream_not_supported() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let request = ChatRequest {
        model: "jina-embeddings-v3".to_string(),
        messages: vec![],
        ..Default::default()
    };

    let result = provider
        .chat_completion_stream(request, RequestContext::default())
        .await;
    assert!(result.is_err());

    match result {
        Err(ProviderError::NotSupported { provider, .. }) => {
            assert_eq!(provider, "jina");
        }
        Err(_) => panic!("Expected NotSupported error"),
        Ok(_) => panic!("Expected error, got Ok"),
    }
}

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost_known_model() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("jina-embeddings-v3", 1000, 0).await;
    assert!(matches!(cost, Ok(v) if v >= 0.0));
}

#[tokio::test]
async fn test_calculate_cost_unknown_model() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("unknown-model", 1000, 500).await;
    assert!(matches!(cost, Ok(v) if v >= 0.0));
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let cost = provider.calculate_cost("jina-embeddings-v3", 0, 0).await;
    assert!(cost.is_ok());
    assert!((cost.unwrap() - 0.0).abs() < 0.0001);
}

#[tokio::test]
async fn test_calculate_rerank_cost() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();

    let cost = provider
        .calculate_rerank_cost("jina-reranker-v2-base-multilingual", 1000)
        .unwrap();
    // $0.000018 per 1k tokens
    assert!((cost - 0.000018).abs() < 0.0000001);
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_authentication() {
    let mapper = JinaErrorMapper;
    let error = mapper.map_http_error(401, "Unauthorized");

    match error {
        ProviderError::Authentication { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected Authentication error"),
    }
}

#[test]
fn test_error_mapper_rate_limit() {
    let mapper = JinaErrorMapper;
    let error = mapper.map_http_error(429, "Rate limit exceeded");

    match error {
        ProviderError::RateLimit { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_error_mapper_network_error() {
    let mapper = JinaErrorMapper;
    let error = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
    let mapped = mapper.map_network_error(&error);

    match mapped {
        ProviderError::Network { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected Network error"),
    }
}

#[test]
fn test_error_mapper_parsing_error() {
    let mapper = JinaErrorMapper;
    let error = std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid JSON");
    let mapped = mapper.map_parsing_error(&error);

    match mapped {
        ProviderError::ResponseParsing { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected ResponseParsing error"),
    }
}

#[test]
fn test_error_mapper_timeout_error() {
    let mapper = JinaErrorMapper;
    let mapped = mapper.map_timeout_error(std::time::Duration::from_secs(30));

    match mapped {
        ProviderError::Timeout { provider, .. } => {
            assert_eq!(provider, "jina");
        }
        _ => panic!("Expected Timeout error"),
    }
}

// ==================== Get Error Mapper Tests ====================

#[tokio::test]
async fn test_get_error_mapper() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let _mapper = provider.get_error_mapper();
    // Just verify it doesn't panic
}

// ==================== Clone/Debug Tests ====================

#[tokio::test]
async fn test_provider_clone() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.models().len(), cloned.models().len());
}

#[tokio::test]
async fn test_provider_debug() {
    let provider = JinaProvider::new(create_test_config()).await.unwrap();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("JinaProvider"));
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

    assert!(debug_str.contains("JinaConfig"));
}

// ==================== Serialization Tests ====================

#[test]
fn test_config_serialization() {
    let config = create_test_config();
    let json = serde_json::to_value(&config).unwrap();

    assert_eq!(json["api_key"], "test_api_key");
    assert_eq!(json["api_base"], "https://api.jina.ai/v1");
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

    let config: JinaConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.api_key, "my_key");
    assert_eq!(config.api_base, "https://custom.api.com");
    assert_eq!(config.timeout_seconds, 60);
    assert_eq!(config.max_retries, 5);
}

// ==================== Rerank Types Tests ====================

#[test]
fn test_rerank_request_serialization() {
    let request = RerankRequest {
        model: "jina-reranker-v2-base-multilingual".to_string(),
        query: "What is machine learning?".to_string(),
        documents: vec![
            "Machine learning is a subset of AI.".to_string(),
            "Deep learning uses neural networks.".to_string(),
        ],
        top_n: Some(2),
        return_documents: Some(true),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["model"], "jina-reranker-v2-base-multilingual");
    assert_eq!(json["query"], "What is machine learning?");
    assert_eq!(json["top_n"], 2);
}

#[test]
fn test_rerank_response_deserialization() {
    let json = r#"{
            "id": "test-id",
            "results": [
                {"index": 0, "relevance_score": 0.95, "document": {"text": "Machine learning is a subset of AI."}},
                {"index": 1, "relevance_score": 0.85, "document": {"text": "Deep learning uses neural networks."}}
            ],
            "usage": {"total_tokens": 100}
        }"#;

    let response: RerankResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "test-id");
    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].index, 0);
    assert!((response.results[0].relevance_score - 0.95).abs() < 0.01);
    assert_eq!(response.usage.unwrap().total_tokens, 100);
}
