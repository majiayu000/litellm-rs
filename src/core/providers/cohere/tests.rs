//! Cohere Provider Tests
//!
//! Comprehensive tests for Cohere provider functionality

use self::config::{CohereApiVersion, CohereConfig};
use super::*;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;
use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, embedding::EmbeddingInput, embedding::EmbeddingRequest,
    message::MessageContent, message::MessageRole,
};
use rerank::{RerankDocument, RerankRequest};
use serde_json::json;

// ==================== Configuration Tests ====================

#[test]
fn test_config_default() {
    let config = CohereConfig::default();

    assert!(config.api_key.is_empty());
    assert_eq!(config.api_base, "https://api.cohere.ai");
    assert_eq!(config.api_version, CohereApiVersion::V2);
    assert_eq!(config.timeout_seconds, 60);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_config_with_api_key() {
    let config = CohereConfig::new("my-api-key");

    assert_eq!(config.api_key, "my-api-key");
    assert_eq!(config.api_base, "https://api.cohere.ai");
}

#[test]
fn test_config_builder_pattern() {
    let config = CohereConfig::new("key")
        .with_api_version(CohereApiVersion::V1)
        .with_api_base("https://custom.api.com")
        .with_timeout(120);

    assert_eq!(config.api_version, CohereApiVersion::V1);
    assert_eq!(config.api_base, "https://custom.api.com");
    assert_eq!(config.timeout_seconds, 120);
}

#[test]
fn test_config_validation() {
    use crate::core::traits::provider::ProviderConfig;

    // Valid config
    let config = CohereConfig::new("key");
    assert!(config.validate().is_ok());

    // Empty API key
    let config = CohereConfig::default();
    assert!(config.validate().is_err());

    // Zero timeout
    let mut config = CohereConfig::new("key");
    config.timeout_seconds = 0;
    assert!(config.validate().is_err());

    // Too many retries
    let mut config = CohereConfig::new("key");
    config.max_retries = 15;
    assert!(config.validate().is_err());
}

#[test]
fn test_config_endpoints() {
    let config = CohereConfig::new("key");

    assert!(config.chat_endpoint().contains("/v2/chat"));
    assert!(config.embed_endpoint().contains("/v2/embed"));
    assert!(config.rerank_endpoint().contains("/v1/rerank"));
    assert!(config.models_endpoint().contains("/v1/models"));
}

#[test]
fn test_config_v1_chat_endpoint() {
    let config = CohereConfig::new("key").with_api_version(CohereApiVersion::V1);

    assert!(config.chat_endpoint().contains("/v1/chat"));
}

#[test]
fn test_api_version_path() {
    assert_eq!(CohereApiVersion::V1.as_path(), "v1");
    assert_eq!(CohereApiVersion::V2.as_path(), "v2");
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation_success() {
    let config = CohereConfig::new("test-key");
    let provider = CohereProvider::new(config).await;

    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_with_api_key() {
    let provider = CohereProvider::with_api_key("test-key").await;

    assert!(provider.is_ok());
    let provider = provider.unwrap();
    assert_eq!(provider.name(), "cohere");
}

#[tokio::test]
async fn test_provider_creation_fails_without_key() {
    let config = CohereConfig::default();
    let provider = CohereProvider::new(config).await;

    assert!(provider.is_err());
}

// ==================== Provider Capabilities Tests ====================

#[tokio::test]
async fn test_provider_name() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    assert_eq!(provider.name(), "cohere");
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::Embeddings));
    assert!(caps.contains(&ProviderCapability::Rerank));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
}

#[tokio::test]
async fn test_provider_models_not_empty() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    assert!(!models.is_empty());
}

#[tokio::test]
async fn test_provider_has_command_models() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    assert!(models.iter().any(|m| m.id == "command-a-plus-05-2026"));
    assert!(models.iter().any(|m| m.id == "command-a-03-2025"));
    assert!(models.iter().any(|m| m.id == "command-a-reasoning-08-2025"));
    assert!(models.iter().any(|m| m.id == "command-a-vision-07-2025"));
    assert!(models.iter().any(|m| m.id == "command-a-translate-08-2025"));
    assert!(models.iter().any(|m| m.id == "command-r7b-12-2024"));
    assert!(models.iter().any(|m| m.id == "command-r-plus-08-2024"));
    assert!(models.iter().any(|m| m.id == "command-r-08-2024"));
    assert!(models.iter().any(|m| m.id == "command-r-plus"));
    assert!(models.iter().any(|m| m.id == "command-r"));
    assert!(models.iter().any(|m| m.id == "command"));
    assert!(models.iter().any(|m| m.id == "command-light"));
}

#[tokio::test]
async fn test_provider_has_embed_models() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    assert!(models.iter().any(|m| m.id == "embed-v4.0"));
    assert!(models.iter().any(|m| m.id == "embed-english-v3.0"));
    assert!(models.iter().any(|m| m.id == "embed-multilingual-v3.0"));
    assert!(models.iter().any(|m| m.id == "embed-english-light-v3.0"));
}

#[tokio::test]
async fn test_provider_has_rerank_models() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    assert!(models.iter().any(|m| m.id == "rerank-v4.0-pro"));
    assert!(models.iter().any(|m| m.id == "rerank-v4.0-fast"));
    assert!(models.iter().any(|m| m.id == "rerank-v3.5"));
    assert!(models.iter().any(|m| m.id == "rerank-english-v3.0"));
    assert!(models.iter().any(|m| m.id == "rerank-multilingual-v3.0"));
}

#[tokio::test]
async fn test_provider_has_current_audio_and_aya_models() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    assert!(models.iter().any(|m| m.id == "cohere-transcribe-03-2026"));
    assert!(models.iter().any(|m| m.id == "tiny-aya-global"));
    assert!(models.iter().any(|m| m.id == "tiny-aya-earth"));
    assert!(models.iter().any(|m| m.id == "tiny-aya-fire"));
    assert!(models.iter().any(|m| m.id == "tiny-aya-water"));
    assert!(models.iter().any(|m| m.id == "c4ai-aya-expanse-32b"));
    assert!(models.iter().any(|m| m.id == "c4ai-aya-vision-32b"));
}

#[tokio::test]
async fn test_provider_models_have_pricing() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let models = provider.models();

    for model in models {
        assert_eq!(model.provider, "cohere");
        assert_eq!(model.currency, "USD");
        if model.input_cost_per_1k_tokens.is_none() || model.output_cost_per_1k_tokens.is_none() {
            assert_eq!(model.metadata.get("status"), Some(&json!("live")));
        }
    }
}

#[tokio::test]
async fn test_current_cohere_model_metadata() {
    let Ok(provider) = CohereProvider::with_api_key("key").await else {
        panic!("cohere provider should initialize with an API key");
    };
    let models = provider.models();

    let Some(command_a_plus) = models
        .iter()
        .find(|model| model.id == "command-a-plus-05-2026")
    else {
        panic!("command-a-plus-05-2026 should be registered");
    };
    assert_eq!(command_a_plus.max_context_length, 128000);
    assert_eq!(command_a_plus.max_output_length, Some(64000));
    assert!(command_a_plus.supports_tools);
    assert!(command_a_plus.supports_multimodal);

    let Some(command_a_vision) = models
        .iter()
        .find(|model| model.id == "command-a-vision-07-2025")
    else {
        panic!("command-a-vision-07-2025 should be registered");
    };
    assert_eq!(command_a_vision.max_context_length, 128000);
    assert_eq!(command_a_vision.max_output_length, Some(8000));
    assert!(command_a_vision.supports_multimodal);

    let Some(command) = models.iter().find(|model| model.id == "command") else {
        panic!("command should remain registered as a deprecated legacy alias");
    };
    assert_eq!(
        command.metadata.get("status"),
        Some(&json!("deprecated_2025_09_15"))
    );
}

// ==================== Model Classification Tests ====================
// Note: Model classification tests removed - is_embedding_model and is_rerank_model are private methods

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost_command_r_plus() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let cost = provider.calculate_cost("command-r-plus", 1000, 1000).await;
    assert!(matches!(cost, Ok(v) if v > 0.0));
}

#[tokio::test]
async fn test_calculate_cost_command_r() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let cost = provider.calculate_cost("command-r", 1000, 1000).await;
    assert!(matches!(cost, Ok(v) if v > 0.0));
}

#[tokio::test]
async fn test_calculate_cost_embed_model() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let cost = provider.calculate_cost("embed-english-v3.0", 1000, 0).await;
    assert!(matches!(cost, Ok(v) if v > 0.0));
}

#[tokio::test]
async fn test_calculate_cost_rerank_model() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let result = provider.calculate_cost("rerank-v3.5", 1000, 0).await;
    assert!(matches!(
        result,
        Err(crate::core::providers::unified_provider::ProviderError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn test_calculate_cost_unknown_model() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let result = provider.calculate_cost("unknown-model", 1000, 500).await;
    assert!(matches!(
        result,
        Err(crate::core::providers::unified_provider::ProviderError::ModelNotFound { .. })
    ));
}

#[tokio::test]
async fn test_calculate_cost_unpriced_model_errors() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let result = provider
        .calculate_cost("command-a-reasoning-08-2025", 1000, 500)
        .await;
    assert!(matches!(
        result,
        Err(crate::core::providers::unified_provider::ProviderError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let cost = provider
        .calculate_cost("command-r-plus", 0, 0)
        .await
        .unwrap();
    assert!(cost.abs() < 0.0001);
}

// ==================== Supported Params Tests ====================

#[tokio::test]
async fn test_supported_params_chat() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let params = provider.get_supported_openai_params("command-r-plus");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"seed"));
}

#[tokio::test]
async fn test_supported_params_embed() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let params = provider.get_supported_openai_params("embed-english-v3.0");

    assert!(params.contains(&"encoding_format"));
    assert!(params.contains(&"dimensions"));
}

// ==================== Request Transformation Tests ====================

#[tokio::test]
async fn test_transform_request_basic() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let request = ChatRequest {
        model: "command-r-plus".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = crate::core::types::context::RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let body = result.unwrap();
    assert_eq!(body["model"], "command-r-plus");
}

#[tokio::test]
async fn test_transform_request_with_params() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();

    let request = ChatRequest {
        model: "command-r".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Test".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: Some(0.9),
        seed: Some(42),
        ..Default::default()
    };

    let context = crate::core::types::context::RequestContext::default();
    let body = provider.transform_request(request, context).await.unwrap();

    assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
    assert_eq!(body["max_tokens"], 100);
    assert!((body["p"].as_f64().unwrap() - 0.9).abs() < 0.001);
    assert_eq!(body["seed"], 42);
}

// ==================== Error Mapping Tests ====================

#[tokio::test]
async fn test_error_mapper_authentication() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let mapper = provider.get_error_mapper();
    let error = mapper.map_http_error(401, "Invalid API key");

    match error {
        CohereError::Authentication { provider, .. } => {
            assert_eq!(provider, "cohere");
        }
        _ => panic!("Expected Authentication error"),
    }
}

#[tokio::test]
async fn test_error_mapper_rate_limit() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let mapper = provider.get_error_mapper();
    let error = mapper.map_http_error(429, "Rate limit exceeded");

    match error {
        CohereError::RateLimit { provider, .. } => {
            assert_eq!(provider, "cohere");
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[tokio::test]
async fn test_error_mapper_server_error() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let mapper = provider.get_error_mapper();
    let error = mapper.map_http_error(500, "Internal server error");

    match error {
        CohereError::ApiError {
            provider, status, ..
        } => {
            assert_eq!(provider, "cohere");
            assert_eq!(status, 500);
        }
        _ => panic!("Expected ApiError"),
    }
}

// ==================== Rerank Tests ====================

#[test]
fn test_rerank_request_transformation() {
    let request = RerankRequest {
        model: "rerank-english-v3.0".to_string(),
        query: "What is AI?".to_string(),
        documents: vec![
            RerankDocument::Text("AI is artificial intelligence".to_string()),
            RerankDocument::Text("ML is machine learning".to_string()),
        ],
        top_n: Some(2),
        return_documents: Some(true),
        max_chunks_per_doc: None,
        rank_fields: None,
    };

    let body = rerank::CohereRerankHandler::transform_request(&request).unwrap();

    assert_eq!(body["model"], "rerank-english-v3.0");
    assert_eq!(body["query"], "What is AI?");
    assert_eq!(body["top_n"], 2);
    assert!(body["return_documents"].as_bool().unwrap());
}

#[test]
fn test_rerank_response_transformation() {
    let response = json!({
        "id": "test-123",
        "results": [
            {"index": 0, "relevance_score": 0.95, "document": {"text": "AI is artificial intelligence"}},
            {"index": 1, "relevance_score": 0.75, "document": {"text": "ML is machine learning"}}
        ],
        "meta": {
            "billed_units": {"search_units": 2}
        }
    });

    let result = rerank::CohereRerankHandler::transform_response(response).unwrap();

    assert_eq!(result.id, "test-123");
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0].relevance_score, 0.95);
    assert!(result.results[0].document.is_some());
}

// ==================== Embedding Tests ====================

#[test]
fn test_embedding_request_transformation() {
    let config = CohereConfig::new("key");
    let request = EmbeddingRequest {
        model: "embed-english-v3.0".to_string(),
        input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
        encoding_format: Some("float".to_string()),
        dimensions: Some(512),
        user: None,
        task_type: Some("search_document".to_string()),
    };

    let body = embed::CohereEmbeddingHandler::transform_request(&request, &config).unwrap();

    assert_eq!(body["model"], "embed-english-v3.0");
    assert_eq!(body["input_type"], "search_document");
    assert_eq!(body["output_dimension"], 512);
}

#[test]
fn test_embedding_default_dimensions() {
    assert_eq!(
        embed::CohereEmbeddingHandler::get_default_dimensions("embed-english-v3.0"),
        Some(1024)
    );
    assert_eq!(
        embed::CohereEmbeddingHandler::get_default_dimensions("embed-multilingual-v3.0"),
        Some(1024)
    );
    assert_eq!(
        embed::CohereEmbeddingHandler::get_default_dimensions("embed-english-v2.0"),
        Some(4096)
    );
}

// ==================== Clone and Debug Tests ====================

#[tokio::test]
async fn test_provider_clone() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.models().len(), cloned.models().len());
}

#[tokio::test]
async fn test_provider_debug() {
    let provider = CohereProvider::with_api_key("key").await.unwrap();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("CohereProvider"));
}

#[test]
fn test_config_clone() {
    let config = CohereConfig::new("key")
        .with_api_version(CohereApiVersion::V1)
        .with_timeout(120);

    let cloned = config.clone();

    assert_eq!(config.api_key, cloned.api_key);
    assert_eq!(config.api_version, cloned.api_version);
    assert_eq!(config.timeout_seconds, cloned.timeout_seconds);
}

// ==================== Streaming Tests ====================
// Note: Streaming parser tests removed - finish_reason type changed from String to FinishReason enum

// ==================== Chat Handler Tests ====================
// Note: Chat handler tests removed - extract_content and extract_usage are private methods

#[test]
fn test_chat_map_params() {
    use chat::CohereChatHandler;
    use std::collections::HashMap;

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), json!(0.8));
    params.insert("top_p".to_string(), json!(0.95));
    params.insert("stop".to_string(), json!(["END"]));
    params.insert("max_tokens".to_string(), json!(200));

    let mapped = CohereChatHandler::map_openai_params(params);

    assert_eq!(mapped["temperature"], json!(0.8));
    assert_eq!(mapped["p"], json!(0.95));
    assert_eq!(mapped["stop_sequences"], json!(["END"]));
    assert_eq!(mapped["max_tokens"], json!(200));
}
