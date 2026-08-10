use super::*;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

// Helper function to create a basic valid request
fn create_valid_request(model: &str) -> ChatRequest {
    ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: None,
        max_tokens: None,
        max_completion_tokens: None,
        top_p: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: None,
        stream: false,
        stream_options: None,
        tools: None,
        tool_choice: None,
        parallel_tool_calls: None,
        response_format: None,
        user: None,
        seed: None,
        n: None,
        logit_bias: None,
        functions: None,
        function_call: None,
        logprobs: None,
        top_logprobs: None,
        reasoning_effort: None,
        store: None,
        metadata: None,
        service_tier: None,
        thinking: None,
        extra_params: std::collections::HashMap::new(),
    }
}

// ==================== Provider Creation Tests ====================

#[test]
fn test_provider_creation() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config);
    assert!(provider.is_ok());
}

#[test]
fn test_provider_creation_with_short_key() {
    let config = GeminiConfig::new_google_ai("short-key");
    let provider = GeminiProvider::new(config);
    // Should fail validation for short API key
    assert!(provider.is_err());
}

// ==================== Provider Capabilities Tests ====================

#[test]
fn test_provider_capabilities() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert_eq!(provider.name(), "gemini");
    assert!(provider.supports_streaming());
    assert!(provider.supports_tools());
    assert!(provider.supports_vision());
    assert!(!provider.supports_embeddings());
    assert!(!provider.supports_image_generation());
}

#[test]
fn test_capabilities_array() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let caps = provider.capabilities();
    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
}

// ==================== Model Support Tests ====================

#[test]
fn test_model_support() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert!(provider.supports_model("gemini-1.0-pro"));
    assert!(provider.supports_model("gemini-1.5-flash"));
    assert!(!provider.supports_model("gpt-4"));
}

#[test]
fn test_model_support_gemini_1_0_pro() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert!(provider.supports_model("gemini-1.0-pro"));
}

#[test]
fn test_model_support_unsupported() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert!(!provider.supports_model("claude-3"));
    assert!(!provider.supports_model("llama-2"));
    assert!(!provider.supports_model("unknown-model"));
}

#[test]
fn test_models_list() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let models = provider.models();
    assert!(!models.is_empty());
}

// ==================== Request Validation Tests ====================

#[test]
fn test_request_validation_empty_messages() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let empty_request = ChatRequest {
        model: "gemini-1.0-pro".to_string(),
        messages: vec![],
        ..Default::default()
    };

    assert!(provider.validate_request(&empty_request).is_err());
}

#[test]
fn test_request_validation_invalid_temperature_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.temperature = Some(3.0); // Out of range

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_invalid_temperature_negative() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.temperature = Some(-0.5);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_valid_temperature() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.temperature = Some(1.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_temperature_edge_low() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.temperature = Some(0.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_temperature_edge_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.temperature = Some(2.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_invalid_top_p_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.top_p = Some(1.5);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_invalid_top_p_negative() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.top_p = Some(-0.1);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_valid_top_p() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-1.0-pro");
    request.top_p = Some(0.9);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_unsupported_model() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let request = create_valid_request("unsupported-model");
    assert!(provider.validate_request(&request).is_err());
}

// ==================== Supported Params Tests ====================

#[test]
fn test_supported_openai_params() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let params = provider.get_supported_openai_params("gemini-1.0-pro");
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

#[tokio::test]
async fn test_map_openai_params_max_tokens() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut params = HashMap::new();
    params.insert("max_tokens".to_string(), serde_json::json!(100));

    let mapped = provider
        .map_openai_params(params, "gemini-1.0-pro")
        .await
        .unwrap();
    assert!(mapped.contains_key("max_output_tokens"));
    assert!(!mapped.contains_key("max_tokens"));
}

#[tokio::test]
async fn test_map_openai_params_temperature() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));

    let mapped = provider
        .map_openai_params(params, "gemini-1.0-pro")
        .await
        .unwrap();
    assert!(mapped.contains_key("temperature"));
}

#[tokio::test]
async fn test_map_openai_params_unsupported_ignored() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut params = HashMap::new();
    params.insert("frequency_penalty".to_string(), serde_json::json!(0.5));
    params.insert("presence_penalty".to_string(), serde_json::json!(0.5));

    let mapped = provider
        .map_openai_params(params, "gemini-1.0-pro")
        .await
        .unwrap();
    assert!(!mapped.contains_key("frequency_penalty"));
    assert!(!mapped.contains_key("presence_penalty"));
}

#[tokio::test]
async fn test_map_openai_params_tools() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut params = HashMap::new();
    params.insert("tools".to_string(), serde_json::json!([]));
    params.insert("tool_choice".to_string(), serde_json::json!("auto"));

    let mapped = provider
        .map_openai_params(params, "gemini-1.0-pro")
        .await
        .unwrap();
    assert!(mapped.contains_key("tools"));
    assert!(mapped.contains_key("tool_choice"));
}

// ==================== Cost Calculation Tests ====================

#[test]
fn test_calculate_cost() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let cost = provider.calculate_cost("gemini-2.5-flash", 1000, 500);
    assert!(cost.is_ok());
}

#[test]
fn test_calculate_cost_unknown_model() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let cost = provider.calculate_cost("unknown-model", 1000, 500);
    assert!(matches!(cost, Err(ProviderError::ModelNotFound { .. })));
}

#[test]
fn test_calculate_cost_zero_tokens() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let cost = provider.calculate_cost("gemini-2.5-flash", 0, 0);
    assert_eq!(cost.expect("catalogued model should be priced"), 0.0);
}

#[tokio::test]
async fn test_async_calculate_cost() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let cost = LLMProvider::calculate_cost(&provider, "gemini-2.5-flash", 1000, 500).await;
    assert!(cost.is_ok());
}

#[tokio::test]
async fn async_calculate_cost_uses_shared_pricing_units() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let cost = LLMProvider::calculate_cost(&provider, "gemini-2.5-flash", 1_000, 500)
        .await
        .expect("catalogued Gemini model should be priced");

    assert!((cost - 0.00155).abs() < 1e-12);

    let preview = LLMProvider::calculate_cost(&provider, "gemini-3-flash-preview", 1_000, 500)
        .await
        .expect("provider-prefixed exact Gemini row should be priced");
    assert!((preview - 0.002).abs() < 1e-12);
}

#[tokio::test]
async fn async_calculate_cost_unknown_model_returns_typed_error() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    for model in [
        "unknown-google-model",
        "gemini-1.5-flash-9999",
        "gemini-1.5-flash",
    ] {
        let result = LLMProvider::calculate_cost(&provider, model, 1_000, 500).await;
        assert!(matches!(result, Err(ProviderError::ModelNotFound { .. })));
    }
}

// ==================== Unsupported Feature Tests ====================

#[tokio::test]
async fn test_embeddings_not_supported() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let request = EmbeddingRequest {
        model: "gemini-pro".to_string(),
        input: crate::core::types::embedding::EmbeddingInput::Text("test".to_string()),
        encoding_format: None,
        dimensions: None,
        user: None,
        task_type: None,
    };
    let context = RequestContext::default();

    let result = provider.embeddings(request, context).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_image_generation_not_supported() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let request = ImageGenerationRequest {
        model: Some("gemini-pro".to_string()),
        prompt: "test".to_string(),
        n: None,
        size: None,
        quality: None,
        response_format: None,
        style: None,
        user: None,
    };
    let context = RequestContext::default();

    let result = provider.image_generation(request, context).await;
    assert!(result.is_err());
}

// ==================== Provider Name and Identity Tests ====================

#[test]
fn test_provider_name() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert_eq!(provider.name(), "gemini");
}

#[test]
fn test_error_mapper() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let _mapper = provider.get_error_mapper();
    // If it compiles, the mapper exists
}
