use super::*;
use crate::core::providers::gemini::models::CostCalculator;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};
use chrono::{TimeZone, Utc};
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

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

    assert!(provider.supports_model("gemini-3.6-flash"));
    assert!(provider.supports_model("gemini-3.7-flash"));
    assert!(provider.supports_model("gemini-3.5-flash-lite"));
    assert!(provider.supports_model("gemini-2.5-flash"));
    assert!(!provider.supports_model("gemini-1.0-pro"));
    assert!(!provider.supports_model("gemini-3.1-flash"));
    assert!(!provider.supports_model("gpt-4"));
}

#[test]
fn vertex_provider_supports_only_documented_current_models() {
    let provider = GeminiProvider::new(GeminiConfig::new_vertex_ai("project", "location")).unwrap();

    assert!(!provider.supports_model("gemini-3.6-flash"));
    assert!(provider.supports_model("gemini-3.7-flash"));
    assert!(!provider.supports_model("gemini-3.5-flash-lite"));
    assert!(provider.supports_model("gemini-3.5-flash"));
}

#[test]
fn test_retired_gemini_1_0_pro_is_not_published() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    assert!(!provider.supports_model("gemini-1.0-pro"));
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

#[test]
fn provider_model_metadata_tracks_utc_pricing_boundary_without_restart() {
    let promotional_time = Utc.with_ymd_and_hms(2026, 12, 31, 23, 59, 59).unwrap();
    let standard_time = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
    let timestamp = Arc::new(AtomicI64::new(promotional_time.timestamp()));
    let clock_timestamp = Arc::clone(&timestamp);
    let clock = GeminiUtcClock::new(move || {
        chrono::DateTime::from_timestamp(clock_timestamp.load(Ordering::SeqCst), 0)
            .expect("test clock timestamp")
    });
    let provider = GeminiProvider::new_with_clock(
        GeminiConfig::new_google_ai("test-api-key-12345678901234567890"),
        clock,
    )
    .unwrap();

    let listed_prices = || {
        let model = provider
            .models()
            .iter()
            .find(|model| model.id == "gemini-3.7-flash")
            .expect("Gemini 3.7 Flash listing");
        (
            model.input_cost_per_1k_tokens,
            model.output_cost_per_1k_tokens,
        )
    };

    assert_eq!(listed_prices(), (Some(0.00075), Some(0.00375)));
    let promotional = get_gemini_registry()
        .get_core_model_pricing_at("gemini-3.7-flash", promotional_time)
        .unwrap();
    assert_eq!(promotional.cache_read_input_token_cost, Some(0.000075));
    assert_eq!(
        CostCalculator::calculate_multimodal_cost_at(
            "gemini-3.7-flash",
            1_000,
            1_000,
            Some(1_000),
            None,
            None,
            None,
            promotional_time,
        ),
        Some(0.003825)
    );

    timestamp.store(standard_time.timestamp(), Ordering::SeqCst);

    assert_eq!(listed_prices(), (Some(0.0015), Some(0.0075)));
    let standard = get_gemini_registry()
        .get_core_model_pricing_at("gemini-3.7-flash", standard_time)
        .unwrap();
    assert_eq!(standard.cache_read_input_token_cost, Some(0.00015));
    assert_eq!(
        CostCalculator::calculate_multimodal_cost_at(
            "gemini-3.7-flash",
            1_000,
            1_000,
            Some(1_000),
            None,
            None,
            None,
            standard_time,
        ),
        Some(0.00765)
    );
}

// ==================== Request Validation Tests ====================

#[test]
fn test_request_validation_empty_messages() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let empty_request = ChatRequest {
        model: "gemini-2.5-flash".to_string(),
        messages: vec![],
        ..Default::default()
    };

    assert!(provider.validate_request(&empty_request).is_err());
}

#[test]
fn test_request_validation_invalid_temperature_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.temperature = Some(3.0); // Out of range

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_invalid_temperature_negative() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.temperature = Some(-0.5);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_valid_temperature() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.temperature = Some(1.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_temperature_edge_low() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.temperature = Some(0.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_temperature_edge_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.temperature = Some(2.0);

    assert!(provider.validate_request(&request).is_ok());
}

#[test]
fn test_request_validation_invalid_top_p_high() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.top_p = Some(1.5);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_invalid_top_p_negative() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
    request.top_p = Some(-0.1);

    assert!(provider.validate_request(&request).is_err());
}

#[test]
fn test_request_validation_valid_top_p() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut request = create_valid_request("gemini-2.5-flash");
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

#[test]
fn fixed_sampling_models_reject_non_empty_assistant_prefill() {
    let provider = GeminiProvider::new(GeminiConfig::new_google_ai(
        "test-api-key-12345678901234567890",
    ))
    .unwrap();

    for model in [
        "gemini-3.7-flash",
        "gemini-3.6-flash",
        "gemini-3.5-flash-lite",
    ] {
        let mut request = create_valid_request(model);
        request.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("prefill".to_string())),
            ..Default::default()
        });
        assert!(provider.validate_request(&request).is_err(), "{model}");

        request.messages.last_mut().unwrap().content = Some(MessageContent::Text("  ".to_string()));
        assert!(provider.validate_request(&request).is_ok(), "{model}");
    }
}

// ==================== Supported Params Tests ====================

#[test]
fn test_supported_openai_params() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let params = provider.get_supported_openai_params("gemini-2.5-flash");
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

#[test]
fn fixed_sampling_models_do_not_advertise_sampling_parameters() {
    let provider = GeminiProvider::new(GeminiConfig::new_google_ai(
        "test-api-key-12345678901234567890",
    ))
    .unwrap();

    for model in [
        "gemini-3.7-flash",
        "gemini-3.6-flash",
        "gemini-3.5-flash-lite",
    ] {
        let params = provider.get_supported_openai_params(model);
        assert!(!params.contains(&"temperature"), "{model}");
        assert!(!params.contains(&"top_p"), "{model}");
        assert!(!params.contains(&"top_k"), "{model}");
        assert!(params.contains(&"max_tokens"), "{model}");
    }
}

#[tokio::test]
async fn fixed_sampling_models_drop_sampling_parameters() {
    let provider = GeminiProvider::new(GeminiConfig::new_google_ai(
        "test-api-key-12345678901234567890",
    ))
    .unwrap();
    let params = HashMap::from([
        ("temperature".to_string(), serde_json::json!(0.7)),
        ("top_p".to_string(), serde_json::json!(0.9)),
        ("top_k".to_string(), serde_json::json!(20)),
        ("max_tokens".to_string(), serde_json::json!(16)),
    ]);

    for model in ["gemini-3.7-flash", "gemini-3.6-flash"] {
        let mapped = provider
            .map_openai_params(params.clone(), model)
            .await
            .unwrap();
        assert!(!mapped.contains_key("temperature"), "{model}");
        assert!(!mapped.contains_key("top_p"), "{model}");
        assert!(!mapped.contains_key("top_k"), "{model}");
        assert_eq!(
            mapped["max_output_tokens"],
            serde_json::json!(16),
            "{model}"
        );
    }
}

#[tokio::test]
async fn test_map_openai_params_max_tokens() {
    let config = GeminiConfig::new_google_ai("test-api-key-12345678901234567890");
    let provider = GeminiProvider::new(config).unwrap();

    let mut params = HashMap::new();
    params.insert("max_tokens".to_string(), serde_json::json!(100));

    let mapped = provider
        .map_openai_params(params, "gemini-2.5-flash")
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
        .map_openai_params(params, "gemini-2.5-flash")
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
        .map_openai_params(params, "gemini-2.5-flash")
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
        .map_openai_params(params, "gemini-2.5-flash")
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

    let image = LLMProvider::calculate_cost(&provider, "gemini-3-pro-image-preview", 1_000, 500)
        .await
        .expect("chat-capable Gemini image row should be token priced");
    assert!((image - 0.008).abs() < 1e-12);
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
