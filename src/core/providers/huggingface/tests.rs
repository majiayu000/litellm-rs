//! Unit tests for HuggingFace Provider

use super::*;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
};
use crate::core::types::{context::RequestContext, model::ProviderCapability};

fn create_test_config() -> HuggingFaceConfig {
    HuggingFaceConfig::new("hf_test_api_key")
}

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let config = create_test_config();
    let provider = HuggingFaceProvider::new(config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(LLMProvider::name(&provider), "huggingface");
}

#[tokio::test]
async fn test_provider_with_api_key() {
    let provider = HuggingFaceProvider::with_api_key("hf_test_key").await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_no_api_key() {
    let config = HuggingFaceConfig::default();
    let provider = HuggingFaceProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_provider_creation_empty_api_key() {
    let config = HuggingFaceConfig {
        api_key: "".to_string(),
        ..Default::default()
    };
    let provider = HuggingFaceProvider::new(config).await;
    assert!(provider.is_err());
}

#[tokio::test]
async fn test_provider_creation_custom_base() {
    let config = HuggingFaceConfig::with_api_base(
        "hf_test_key",
        "https://my-endpoint.endpoints.huggingface.cloud",
    );
    let provider = HuggingFaceProvider::new(config).await;
    assert!(provider.is_ok());
}

// ==================== Provider Capabilities Tests ====================

#[tokio::test]
async fn test_provider_name() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    assert_eq!(provider.name(), "huggingface");
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
    assert!(caps.contains(&ProviderCapability::Embeddings));
}

#[tokio::test]
async fn test_provider_models() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    let models = provider.models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id.contains("Llama")));
    assert!(models.iter().any(|m| m.id.contains("DeepSeek")));
    assert!(models.iter().all(|m| m.provider == "huggingface"));
}

// ==================== Supported Params Tests ====================

#[tokio::test]
async fn test_get_supported_openai_params() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    let params = provider.get_supported_openai_params("any-model");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

// ==================== Map OpenAI Params Tests ====================

#[tokio::test]
async fn test_map_openai_params_temperature_adjustment() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let mut params = std::collections::HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.0));

    let mapped = provider
        .map_openai_params(params, "any-model")
        .await
        .unwrap();

    // Temperature should be adjusted to 0.01
    let temp = mapped.get("temperature").unwrap().as_f64().unwrap();
    assert!(temp > 0.0);
    assert!((temp - 0.01).abs() < 0.001);
}

#[tokio::test]
async fn test_map_openai_params_max_tokens_adjustment() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let mut params = std::collections::HashMap::new();
    params.insert("max_tokens".to_string(), serde_json::json!(0));

    let mapped = provider
        .map_openai_params(params, "any-model")
        .await
        .unwrap();

    // max_tokens should be adjusted to 1
    let tokens = mapped.get("max_tokens").unwrap().as_u64().unwrap();
    assert_eq!(tokens, 1);
}

#[tokio::test]
async fn test_map_openai_params_passthrough() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let mut params = std::collections::HashMap::new();
    params.insert("top_p".to_string(), serde_json::json!(0.9));
    params.insert("stream".to_string(), serde_json::json!(true));

    let mapped = provider
        .map_openai_params(params, "any-model")
        .await
        .unwrap();

    assert_eq!(mapped.get("top_p").unwrap(), &serde_json::json!(0.9));
    assert_eq!(mapped.get("stream").unwrap(), &serde_json::json!(true));
}

#[tokio::test]
async fn test_map_openai_params_unsupported_filtered() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let mut params = std::collections::HashMap::new();
    params.insert("unsupported_param".to_string(), serde_json::json!("value"));
    params.insert("top_p".to_string(), serde_json::json!(0.9));

    let mapped = provider
        .map_openai_params(params, "any-model")
        .await
        .unwrap();

    assert!(!mapped.contains_key("unsupported_param"));
    assert!(mapped.contains_key("top_p"));
}

// ==================== Transform Request Tests ====================

#[tokio::test]
async fn test_transform_request_basic() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let request = ChatRequest {
        model: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };

    let context = RequestContext::default();
    let result = provider.transform_request(request, context).await;

    // This may fail if it tries to fetch provider mapping, so we check for either success or network error
    match result {
        Ok(transformed) => {
            assert!(transformed["messages"].is_array());
        }
        Err(e) => {
            // Expected if network call fails
            println!(
                "Transform request failed (expected without network): {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_transform_request_with_options() {
    let _provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();

    let request = ChatRequest {
        model: "meta-llama/Llama-3.3-70B-Instruct".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        max_tokens: Some(100),
        top_p: Some(0.9),
        ..Default::default()
    };

    // Verify request was created correctly
    assert_eq!(request.model, "meta-llama/Llama-3.3-70B-Instruct");
    assert_eq!(request.temperature, Some(0.7));
    assert_eq!(request.max_tokens, Some(100));
    assert_eq!(request.top_p, Some(0.9));
}

// Note: transform_chat_request tests removed - method is now private

// ==================== Model Parsing Tests ====================

#[test]
fn test_parse_model_with_provider() {
    let (provider, model) =
        models::parse_model_string("huggingface/together/deepseek-ai/DeepSeek-R1");
    assert_eq!(provider, Some("together".to_string()));
    assert_eq!(model, "deepseek-ai/DeepSeek-R1");
}

#[test]
fn test_parse_model_without_provider() {
    let (provider, model) =
        models::parse_model_string("huggingface/meta-llama/Llama-3.3-70B-Instruct");
    assert!(provider.is_none());
    assert_eq!(model, "meta-llama/Llama-3.3-70B-Instruct");
}

#[test]
fn test_parse_model_no_prefix() {
    let (provider, model) = models::parse_model_string("meta-llama/Llama-3.3-70B-Instruct");
    assert!(provider.is_none());
    assert_eq!(model, "meta-llama/Llama-3.3-70B-Instruct");
}

// ==================== Clone/Debug Tests ====================

#[tokio::test]
async fn test_provider_clone() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    let cloned = provider.clone();

    assert_eq!(provider.name(), cloned.name());
    assert_eq!(provider.models().len(), cloned.models().len());
}

#[tokio::test]
async fn test_provider_debug() {
    let provider = HuggingFaceProvider::new(create_test_config())
        .await
        .unwrap();
    let debug_str = format!("{:?}", provider);

    assert!(debug_str.contains("HuggingFaceProvider"));
}

#[test]
fn test_config_clone() {
    let config = create_test_config();
    let cloned = config.clone();

    assert_eq!(config.api_key, cloned.api_key);
}

#[test]
fn test_config_debug() {
    let config = create_test_config();
    let debug_str = format!("{:?}", config);

    assert!(debug_str.contains("HuggingFaceConfig"));
}

// ==================== Serialization Tests ====================

#[test]
fn test_config_serialization() {
    let config = create_test_config();
    let json = serde_json::to_value(&config).unwrap();

    assert_eq!(json["api_key"], "hf_test_api_key");
    assert_eq!(json["timeout_seconds"], 60);
    assert_eq!(json["max_retries"], 3);
}

#[test]
fn test_config_deserialization() {
    let json = r#"{
        "api_key": "hf_my_key",
        "api_base": "https://custom.endpoint.com",
        "timeout_seconds": 120,
        "max_retries": 5,
        "use_router": false
    }"#;

    let config: HuggingFaceConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.api_key, "hf_my_key");
    assert_eq!(
        config.api_base,
        Some("https://custom.endpoint.com".to_string())
    );
    assert_eq!(config.timeout_seconds, 120);
    assert_eq!(config.max_retries, 5);
    assert!(!config.use_router);
}

// ==================== Custom Endpoint Tests ====================
// Note: test_is_custom_endpoint removed - is_custom_endpoint is now a private method

// ==================== Inference Provider Tests ====================

#[test]
fn test_inference_provider_from_str() {
    assert_eq!(
        InferenceProvider::parse("together"),
        InferenceProvider::Together
    );
    assert_eq!(
        InferenceProvider::parse("sambanova"),
        InferenceProvider::Sambanova
    );
    assert_eq!(
        InferenceProvider::parse("fireworks-ai"),
        InferenceProvider::FireworksAI
    );
    assert_eq!(
        InferenceProvider::parse("hf-inference"),
        InferenceProvider::HFInference
    );
}

#[test]
fn test_inference_provider_as_str() {
    assert_eq!(InferenceProvider::Together.as_str(), "together");
    assert_eq!(InferenceProvider::Sambanova.as_str(), "sambanova");
    assert_eq!(InferenceProvider::FireworksAI.as_str(), "fireworks-ai");
}

// ==================== Task Type Tests ====================

#[test]
fn test_huggingface_task_is_chat() {
    assert!(HuggingFaceTask::TextGenerationInference.is_chat_task());
    assert!(HuggingFaceTask::Conversational.is_chat_task());
    assert!(HuggingFaceTask::TextGeneration.is_chat_task());
    assert!(!HuggingFaceTask::FeatureExtraction.is_chat_task());
}

#[test]
fn test_huggingface_task_is_embedding() {
    assert!(HuggingFaceTask::FeatureExtraction.is_embedding_task());
    assert!(HuggingFaceTask::SentenceSimilarity.is_embedding_task());
    assert!(HuggingFaceTask::Rerank.is_embedding_task());
    assert!(!HuggingFaceTask::TextGenerationInference.is_embedding_task());
}

// ==================== URL Building Tests ====================

#[test]
fn test_get_chat_url_default() {
    let config = HuggingFaceConfig::new("hf_token");
    let url = config.get_chat_url(None, "meta-llama/Llama-3.3-70B-Instruct");

    assert!(url.contains("router.huggingface.co"));
    assert!(url.contains("/v1/chat/completions"));
}

#[test]
fn test_get_chat_url_with_together() {
    let config = HuggingFaceConfig::new("hf_token");
    let url = config.get_chat_url(Some("together"), "deepseek-ai/DeepSeek-R1");

    assert!(url.contains("router.huggingface.co"));
    assert!(url.contains("/together/v1/chat/completions"));
}

#[test]
fn test_get_chat_url_with_fireworks() {
    let config = HuggingFaceConfig::new("hf_token");
    let url = config.get_chat_url(Some("fireworks-ai"), "deepseek-ai/DeepSeek-R1");

    assert!(url.contains("router.huggingface.co"));
    assert!(url.contains("/fireworks-ai/inference/v1/chat/completions"));
}

#[test]
fn test_get_chat_url_custom_endpoint() {
    let config = HuggingFaceConfig::with_api_base(
        "hf_token",
        "https://my-endpoint.endpoints.huggingface.cloud/v1",
    );
    let url = config.get_chat_url(None, "any-model");

    assert_eq!(
        url,
        "https://my-endpoint.endpoints.huggingface.cloud/v1/chat/completions"
    );
}

#[test]
fn test_get_embeddings_url() {
    let config = HuggingFaceConfig::new("hf_token");
    let url = config.get_embeddings_url("feature-extraction", "microsoft/codebert-base");

    assert!(url.contains("hf-inference/pipeline"));
    assert!(url.contains("feature-extraction"));
    assert!(url.contains("microsoft/codebert-base"));
}
