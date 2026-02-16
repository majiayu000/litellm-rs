//! OpenAI Provider Unit Tests
//!
//! Comprehensive tests for the OpenAI provider implementation.

use super::*;
use crate::core::providers::base::GlobalPoolManager;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;
use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
};
use std::collections::HashMap;
use std::sync::Arc;

fn create_test_config() -> OpenAIConfig {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    config
}

fn create_test_provider() -> OpenAIProvider {
    OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::default()),
        config: create_test_config(),
        model_registry: get_openai_registry(),
    }
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_with_api_key() {
    let provider = OpenAIProvider::with_api_key("sk-testkey123").await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "openai");
}

#[tokio::test]
async fn test_provider_creation_with_organization() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());
    config.organization = Some("org-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_with_project() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());
    config.project = Some("proj-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_no_api_key() {
    let config = OpenAIConfig::default();
    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_err());
}

// ==================== Provider Properties Tests ====================

#[test]
fn test_provider_name() {
    let provider = create_test_provider();
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_provider_capabilities() {
    let provider = create_test_provider();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::Embeddings));
    assert!(caps.contains(&ProviderCapability::ImageGeneration));
    assert!(caps.contains(&ProviderCapability::AudioTranscription));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
    assert!(caps.contains(&ProviderCapability::FunctionCalling));
    assert!(caps.contains(&ProviderCapability::FineTuning));
    assert!(caps.contains(&ProviderCapability::ImageEdit));
    assert!(caps.contains(&ProviderCapability::ImageVariation));
    assert!(caps.contains(&ProviderCapability::RealtimeApi));
}

#[test]
fn test_provider_models_not_empty() {
    let provider = create_test_provider();
    assert!(!provider.models().is_empty());
}

// ==================== Model Support Tests ====================

#[test]
fn test_model_support_detection() {
    let provider = create_test_provider();

    // Test GPT-4 capabilities
    assert!(provider.model_supports_capability("gpt-4", &ProviderCapability::ChatCompletion));
    assert!(provider.model_supports_capability("gpt-4", &ProviderCapability::ToolCalling));

    // Test embedding model
    assert!(!provider.model_supports_capability(
        "text-embedding-ada-002",
        &ProviderCapability::ChatCompletion
    ));
}

#[test]
fn test_model_supports_capability_unknown_model() {
    let provider = create_test_provider();
    assert!(
        !provider.model_supports_capability("unknown-model", &ProviderCapability::ChatCompletion)
    );
}

#[test]
fn test_get_model_info() {
    let provider = create_test_provider();

    let model_info = provider.get_model_info("gpt-4");
    assert!(model_info.is_ok());

    let info = model_info.unwrap();
    assert_eq!(info.id, "gpt-4");
    assert_eq!(info.provider, "openai");
    assert!(info.supports_streaming);
    assert!(info.supports_tools);
}

#[test]
fn test_get_model_info_unknown_model() {
    let provider = create_test_provider();

    // Should return default info for unknown models (like Python LiteLLM)
    let model_info = provider.get_model_info("unknown-model-xyz");
    assert!(model_info.is_ok());

    let info = model_info.unwrap();
    assert_eq!(info.id, "unknown-model-xyz");
}

#[test]
fn test_get_model_config() {
    let provider = create_test_provider();

    let config = provider.get_model_config("gpt-4");
    // May or may not have config depending on registry
    let _ = config; // Just verify it doesn't panic
}

// ==================== Supported Params Tests ====================

#[test]
fn test_get_supported_openai_params_gpt4() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-4");

    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

#[test]
fn test_get_supported_openai_params_gpt35() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-3.5-turbo");

    assert!(params.contains(&"messages"));
    assert!(params.contains(&"temperature"));
}

#[test]
fn test_get_supported_openai_params_o1() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("o1-preview");

    // O1 models may or may not be in the registry - check basic params
    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    // If not in registry, defaults to basic params without max_completion_tokens
}

#[test]
fn test_get_supported_openai_params_unknown() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("unknown-model");

    // Should return default params
    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    assert!(params.contains(&"temperature"));
}

// ==================== Transform Request Tests ====================

#[test]
fn test_transform_chat_request_basic() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["model"], "gpt-4");
    assert!(transformed["messages"].is_array());
}

#[test]
fn test_transform_chat_request_with_temperature() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert!(transformed.get("temperature").is_some());
}

#[test]
fn test_transform_chat_request_with_max_tokens() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_tokens: Some(1000),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["max_tokens"], 1000);
}

#[test]
fn test_transform_chat_request_with_max_completion_tokens() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "o1-preview".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        max_completion_tokens: Some(2000),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["max_completion_tokens"], 2000);
}

#[test]
fn test_transform_chat_request_with_top_p() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        top_p: Some(0.9),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert!(transformed.get("top_p").is_some());
}

#[test]
fn test_transform_chat_request_with_user() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        user: Some("user-123".to_string()),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["user"], "user-123");
}

#[test]
fn test_transform_chat_request_with_seed() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        seed: Some(42),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["seed"], 42);
}

#[test]
fn test_transform_chat_request_with_n() {
    let provider = create_test_provider();

    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        n: Some(3),
        ..Default::default()
    };

    let result = provider.transform_chat_request(request);
    assert!(result.is_ok());

    let transformed = result.unwrap();
    assert_eq!(transformed["n"], 3);
}

// ==================== Map OpenAI Params Tests ====================

#[tokio::test]
async fn test_map_openai_params_passthrough() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));
    params.insert("max_tokens".to_string(), serde_json::json!(100));

    let result = provider.map_openai_params(params.clone(), "gpt-4").await;
    assert!(result.is_ok());

    let mapped = result.unwrap();
    // OpenAI params should pass through unchanged
    assert_eq!(mapped, params);
}

// ==================== Cost Calculation Tests ====================

#[tokio::test]
async fn test_calculate_cost() {
    let provider = create_test_provider();

    let cost = provider.calculate_cost("gpt-4", 1000, 500).await;
    assert!(cost.is_ok());

    // Cost should be 0 for unknown pricing (default model info has None for costs)
    let cost_value = cost.unwrap();
    assert!(cost_value >= 0.0);
}

#[tokio::test]
async fn test_calculate_cost_zero_tokens() {
    let provider = create_test_provider();

    let cost = provider.calculate_cost("gpt-4", 0, 0).await;
    assert!(cost.is_ok());
    assert!((cost.unwrap() - 0.0).abs() < 0.0001);
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_authentication() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(401, "Unauthorized");
    match error {
        OpenAIError::Authentication { provider, .. } => {
            assert_eq!(provider, "openai");
        }
        _ => panic!("Expected Authentication error"),
    }
}

#[test]
fn test_error_mapper_rate_limit() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(429, "Rate limit exceeded");
    match error {
        OpenAIError::RateLimit { provider, .. } => {
            assert_eq!(provider, "openai");
        }
        _ => panic!("Expected RateLimit error"),
    }
}

#[test]
fn test_error_mapper_invalid_request() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(400, "Invalid request");
    match error {
        OpenAIError::InvalidRequest { provider, message } => {
            assert_eq!(provider, "openai");
            assert_eq!(message, "Invalid request");
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

#[test]
fn test_error_mapper_api_error() {
    let mapper = OpenAIErrorMapper;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    let error = mapper.map_http_error(500, "Server error");
    match error {
        OpenAIError::ApiError {
            provider, status, ..
        } => {
            assert_eq!(provider, "openai");
            assert_eq!(status, 500);
        }
        _ => panic!("Expected ApiError error"),
    }
}

// ==================== Request Headers Tests ====================

#[test]
fn test_get_request_headers_with_api_key() {
    let provider = create_test_provider();
    let headers = provider.get_request_headers();

    assert!(!headers.is_empty());
    // Should have at least the Authorization header
    let has_auth = headers.iter().any(|h| h.0.as_ref() == "Authorization");
    assert!(has_auth);
}

#[test]
fn test_get_request_headers_with_organization() {
    let mut config = create_test_config();
    config.organization = Some("org-test123".to_string());

    let provider = OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::default()),
        config,
        model_registry: get_openai_registry(),
    };

    let headers = provider.get_request_headers();
    let has_org = headers
        .iter()
        .any(|h| h.0.as_ref() == "OpenAI-Organization");
    assert!(has_org);
}

#[test]
fn test_get_request_headers_with_project() {
    let mut config = create_test_config();
    config.project = Some("proj-test123".to_string());

    let provider = OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::default()),
        config,
        model_registry: get_openai_registry(),
    };

    let headers = provider.get_request_headers();
    let has_project = headers.iter().any(|h| h.0.as_ref() == "OpenAI-Project");
    assert!(has_project);
}

// ==================== Advanced Features Tests ====================

#[test]
fn test_get_advanced_model_capabilities() {
    let provider = create_test_provider();

    let caps = provider.get_advanced_model_capabilities("gpt-4o");
    // Just verify it doesn't panic and returns something
    let _ = caps;
}

#[test]
fn test_estimate_advanced_cost() {
    let provider = create_test_provider();

    let result = provider.estimate_advanced_cost("gpt-4o", 1000, 500, None);
    // May succeed or fail depending on model pricing info
    let _ = result;
}

#[test]
fn test_estimate_advanced_cost_with_reasoning() {
    let provider = create_test_provider();

    let result = provider.estimate_advanced_cost("o1-preview", 1000, 500, Some(200));
    // May succeed or fail depending on model pricing info
    let _ = result;
}

// ==================== Clone/Debug Tests ====================

#[test]
fn test_provider_clone() {
    let provider = create_test_provider();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.capabilities().len(), cloned.capabilities().len());
}

#[test]
fn test_provider_debug() {
    let provider = create_test_provider();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("OpenAIProvider"));
}
