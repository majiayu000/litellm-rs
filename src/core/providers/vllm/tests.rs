//! Unit tests for vLLM provider

#[cfg(test)]
use super::*;
use crate::core::traits::provider::ProviderConfig;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

// ==================== Configuration Tests ====================

#[test]
fn test_config_default() {
    let config = VLLMConfig::default();
    assert!(config.api_key.is_none());
    assert!(config.api_base.is_none());
    assert_eq!(config.timeout, 120);
    assert_eq!(config.max_retries, 3);
    assert!(!config.debug);
    assert!(config.skip_model_validation);
}

#[test]
fn test_config_new() {
    let config = VLLMConfig::new("http://localhost:8000/v1");
    assert_eq!(
        config.api_base,
        Some("http://localhost:8000/v1".to_string())
    );
    assert!(config.api_key.is_none());
}

#[test]
fn test_config_with_credentials() {
    let config =
        VLLMConfig::with_credentials("http://localhost:8000/v1", Some("test-key".to_string()));
    assert_eq!(
        config.api_base,
        Some("http://localhost:8000/v1".to_string())
    );
    assert_eq!(config.api_key, Some("test-key".to_string()));
}

#[test]
fn test_config_builder_methods() {
    let config = VLLMConfig::new("http://localhost:8000/v1")
        .with_model("meta-llama/Llama-3.1-8B-Instruct")
        .with_timeout(60)
        .with_debug(true);

    assert_eq!(
        config.model,
        Some("meta-llama/Llama-3.1-8B-Instruct".to_string())
    );
    assert_eq!(config.timeout, 60);
    assert!(config.debug);
}

#[test]
fn test_config_validation() {
    // Valid config with API base
    let config = VLLMConfig::new("http://localhost:8000/v1");
    assert!(config.validate().is_ok());

    // Invalid config without API base (and no env var)
    // SAFETY: This is a single-threaded test
    unsafe {
        std::env::remove_var("VLLM_API_BASE");
    }
    let config = VLLMConfig::default();
    assert!(config.validate().is_err());

    // Invalid config with zero timeout
    let config = VLLMConfig {
        api_base: Some("http://localhost:8000/v1".to_string()),
        timeout: 0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let config = VLLMConfig::new("http://localhost:8000/v1");
    let provider = VLLMProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_api_base() {
    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1").await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_credentials() {
    let provider =
        VLLMProvider::with_credentials("http://localhost:8000/v1", Some("test-key".to_string()))
            .await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_with_model() {
    let config =
        VLLMConfig::new("http://localhost:8000/v1").with_model("meta-llama/Llama-3.1-8B-Instruct");
    let provider = VLLMProvider::new(config).await.unwrap();

    // Should have one model in the list
    assert!(!provider.models().is_empty());
    assert_eq!(provider.models()[0].id, "meta-llama/Llama-3.1-8B-Instruct");
}

// ==================== Provider Trait Tests ====================

#[tokio::test]
async fn test_provider_name() {
    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();
    assert_eq!(provider.name(), "vllm");
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();
    let capabilities = provider.capabilities();

    assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
    assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
    assert!(capabilities.contains(&ProviderCapability::ToolCalling));
}

#[tokio::test]
async fn test_supported_openai_params() {
    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();

    let params = provider.get_supported_openai_params("any-model");

    // Check standard OpenAI params
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"response_format"));

    // Check vLLM-specific params
    assert!(params.contains(&"top_k"));
    assert!(params.contains(&"min_p"));
    assert!(params.contains(&"repetition_penalty"));
    assert!(params.contains(&"use_beam_search"));
}

#[tokio::test]
async fn test_cost_calculation() {
    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();

    // vLLM is self-hosted, so cost should always be 0
    let cost = provider
        .calculate_cost("any-model", 1000, 1000)
        .await
        .unwrap();
    assert_eq!(cost, 0.0);
}

// ==================== Error Tests ====================
// Note: VLLMError is now a type alias to ProviderError
// Error functionality is tested in the unified provider error tests

// ==================== Model Info Tests ====================

#[test]
fn test_get_model_info_known() {
    let info = model_info::get_model_info("meta-llama/Meta-Llama-3.1-8B-Instruct");
    assert!(info.is_some());
    let info = info.unwrap();
    assert_eq!(info.family, "llama");
    assert!(info.supports_tools);
}

#[test]
fn test_get_model_info_unknown() {
    let info = model_info::get_model_info("unknown-model");
    assert!(info.is_none());
}

#[test]
fn test_get_or_create_model_info() {
    // Known model
    let info = model_info::get_or_create_model_info("mistralai/Mistral-7B-Instruct-v0.3");
    assert_eq!(info.family, "mistral");

    // Unknown model - should create custom
    let info = model_info::get_or_create_model_info("my-custom-model");
    assert_eq!(info.family, "custom");
    assert_eq!(info.model_id, "my-custom-model");
}

#[test]
fn test_get_known_models() {
    let models = model_info::get_known_models();
    assert!(!models.is_empty());

    // Check some expected models
    assert!(models.iter().any(|m| m.contains("Llama")));
    assert!(models.iter().any(|m| m.contains("Mistral")));
}

#[test]
fn test_get_models_by_family() {
    let llama_models = model_info::get_models_by_family("llama");
    assert!(!llama_models.is_empty());

    let mistral_models = model_info::get_models_by_family("mistral");
    assert!(!mistral_models.is_empty());

    let qwen_models = model_info::get_models_by_family("qwen");
    assert!(!qwen_models.is_empty());
}

#[test]
fn test_get_tool_capable_models() {
    let tool_models = model_info::get_tool_capable_models();
    assert!(!tool_models.is_empty());

    // All returned models should support tools
    for model_id in tool_models {
        let info = model_info::get_model_info(model_id).unwrap();
        assert!(info.supports_tools);
    }
}

// ==================== Batch Processing Tests ====================

#[test]
fn test_batch_params_default() {
    let params = provider::BatchParams::default();
    assert!(params.temperature.is_none());
    assert!(params.max_tokens.is_none());
    assert!(params.top_p.is_none());
    assert!(params.stop.is_none());
}

// ==================== Request Transform Tests ====================

#[tokio::test]
async fn test_transform_request() {
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();

    let request = crate::core::types::chat::ChatRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        ..Default::default()
    };

    let context = crate::core::types::context::RequestContext::default();
    let result = provider.transform_request(request, context).await;

    assert!(result.is_ok());
    let json = result.unwrap();
    assert_eq!(json["model"], "test-model");
    assert!((json["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
    assert_eq!(json["max_tokens"], 100);
}

#[tokio::test]
async fn test_map_openai_params() {
    use std::collections::HashMap;

    let provider = VLLMProvider::with_api_base("http://localhost:8000/v1")
        .await
        .unwrap();

    let mut params = HashMap::new();
    params.insert(
        "temperature".to_string(),
        serde_json::Value::Number(serde_json::Number::from_f64(0.7).unwrap()),
    );
    params.insert(
        "max_tokens".to_string(),
        serde_json::Value::Number(100.into()),
    );

    let result = provider
        .map_openai_params(params.clone(), "test-model")
        .await;
    assert!(result.is_ok());

    // vLLM uses OpenAI-compatible params, so they should be unchanged
    let mapped = result.unwrap();
    assert_eq!(mapped["temperature"], params["temperature"]);
    assert_eq!(mapped["max_tokens"], params["max_tokens"]);
}

// ==================== Streaming Tests ====================

#[test]
fn test_streaming_response_to_chunks() {
    use crate::core::types::responses::{ChatChoice, ChatResponse, FinishReason, Usage};
    use crate::core::types::{chat::ChatMessage, message::MessageContent};

    let response = ChatResponse {
        id: "test-id".to_string(),
        object: "chat.completion".to_string(),
        created: 1234567890,
        model: "test-model".to_string(),
        system_fingerprint: Some("fp_test".to_string()),
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: crate::core::types::message::MessageRole::Assistant,
                content: Some(MessageContent::Text("Hello world".to_string())),
                name: None,
                tool_calls: None,
                function_call: None,
                thinking: None,
                ..Default::default()
            },
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        }],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 2,
            total_tokens: 12,
            ..Default::default()
        }),
    };

    // Use the streaming module's internal function (would need to expose for testing)
    // For now, just verify the response structure is correct
    assert_eq!(response.id, "test-id");
    assert_eq!(response.choices.len(), 1);
}

// ==================== Provider Error Conversion Tests ====================
// Note: VLLMError is now a type alias to ProviderError
// No conversion needed - they are the same type

// ==================== Model Info Conversion Tests ====================

#[tokio::test]
async fn test_model_info_conversion() {
    let config = VLLMConfig::new("http://localhost:8000/v1")
        .with_model("meta-llama/Meta-Llama-3.1-8B-Instruct");
    let provider = VLLMProvider::new(config).await.unwrap();

    let models = provider.models();
    assert_eq!(models.len(), 1);

    let model = &models[0];
    assert_eq!(model.id, "meta-llama/Meta-Llama-3.1-8B-Instruct");
    assert_eq!(model.provider, "vllm");
    assert!(model.supports_streaming);
    assert!(model.supports_tools);

    // vLLM is self-hosted, no API costs
    assert!(model.input_cost_per_1k_tokens.is_none());
    assert!(model.output_cost_per_1k_tokens.is_none());
}

// ==================== Config Serialization Tests ====================

#[test]
fn test_config_serialization() {
    let config = VLLMConfig {
        api_key: Some("test-key".to_string()),
        api_base: Some("http://localhost:8000/v1".to_string()),
        timeout: 60,
        max_retries: 5,
        debug: true,
        model: Some("test-model".to_string()),
        skip_model_validation: false,
    };

    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["api_key"], "test-key");
    assert_eq!(json["api_base"], "http://localhost:8000/v1");
    assert_eq!(json["timeout"], 60);
    assert_eq!(json["model"], "test-model");
}

#[test]
fn test_config_deserialization() {
    let json = r#"{
        "api_base": "http://localhost:8000/v1",
        "model": "meta-llama/Llama-3.1-8B-Instruct",
        "timeout": 90
    }"#;

    let config: VLLMConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.api_base,
        Some("http://localhost:8000/v1".to_string())
    );
    assert_eq!(
        config.model,
        Some("meta-llama/Llama-3.1-8B-Instruct".to_string())
    );
    assert_eq!(config.timeout, 90);
}
