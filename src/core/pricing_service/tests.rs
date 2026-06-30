//! Tests for the pricing service

#[cfg(test)]
use crate::core::pricing_service::{LiteLLMModelInfo, PricingService, PricingUsage};
#[cfg(test)]
use crate::core::types::responses::{PromptTokensDetails, Usage};
use std::collections::HashMap;

#[test]
fn test_model_info_deserialization() {
    let json = r#"{
        "max_tokens": 4096,
        "input_cost_per_token": 0.00001,
        "output_cost_per_token": 0.00003,
        "litellm_provider": "openai",
        "mode": "chat",
        "supports_function_calling": true
    }"#;

    let model_info: LiteLLMModelInfo = serde_json::from_str(json).unwrap();
    assert_eq!(model_info.max_tokens, Some(4096));
    assert_eq!(model_info.input_cost_per_token, Some(0.00001));
    assert_eq!(model_info.litellm_provider, "openai");
}

#[tokio::test]
async fn test_token_based_cost_calculation() {
    let service = PricingService::new(None);

    let model_info = LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: Some(0.001),
        output_cost_per_token: Some(0.002),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "openai".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(true),
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    };

    let result = service
        .calculate_token_based_cost("gpt-4", &model_info, 1000, 500)
        .unwrap();

    // 1000 * 0.001 + 500 * 0.002 = 1 + 1 = 2
    assert!((result.total_cost - 2.0).abs() < f64::EPSILON);
    assert_eq!(result.input_tokens, 1000);
    assert_eq!(result.output_tokens, 500);
}

#[test]
fn pricing_usage_preserves_cache_creation_and_read_tokens() {
    let usage = Usage {
        prompt_tokens: 1000,
        completion_tokens: 100,
        total_tokens: 1100,
        prompt_tokens_details: Some(PromptTokensDetails {
            cached_tokens: Some(700),
            cache_creation_tokens: Some(200),
            cache_read_tokens: Some(500),
            audio_tokens: None,
        }),
        completion_tokens_details: None,
        thinking_usage: None,
    };

    let pricing_usage = PricingUsage::from(&usage);

    assert_eq!(pricing_usage.cached_tokens, Some(700));
    assert_eq!(pricing_usage.cache_creation_tokens, Some(200));
    assert_eq!(pricing_usage.cache_read_tokens, Some(500));
    assert_eq!(pricing_usage.non_cached_prompt_tokens(), 300);
}

#[test]
fn pricing_usage_treats_cached_tokens_as_read_fallback() {
    let mut pricing_usage = PricingUsage::new(1000, 100);
    pricing_usage.cached_tokens = Some(500);
    pricing_usage.cache_creation_tokens = Some(200);

    assert_eq!(pricing_usage.cache_read_token_count(), 500);
    assert_eq!(pricing_usage.non_cached_prompt_tokens(), 300);
}

#[test]
fn provider_pricing_charges_cache_creation_and_read_separately() {
    let service = PricingService::new(None);
    let mut model_info = LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: Some(0.00001),
        output_cost_per_token: Some(0.00003),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "openai".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    };
    model_info.extra.insert(
        "cache_creation_input_token_cost".to_string(),
        serde_json::Value::from(0.000012),
    );
    model_info.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::Value::from(0.000002),
    );
    service.add_custom_model("cache-priced-model".to_string(), model_info);
    let mut usage = PricingUsage::new(1000, 100);
    usage.cached_tokens = Some(700);
    usage.cache_creation_tokens = Some(200);
    usage.cache_read_tokens = Some(500);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("openai", "cache-priced-model", &usage)
        .unwrap();

    assert!((cost.input_cost - 0.003).abs() < f64::EPSILON);
    assert!((cost.output_cost - 0.003).abs() < f64::EPSILON);
    assert!((cost.cache_cost - 0.0034).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.0094).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_honors_explicit_zero_cache_prices() {
    let service = PricingService::new(None);
    let mut model_info = LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: Some(0.00001),
        output_cost_per_token: Some(0.00003),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "openai".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    };
    model_info.extra.insert(
        "cache_creation_input_token_cost".to_string(),
        serde_json::Value::from(0.0),
    );
    model_info.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::Value::from(0.0),
    );
    service.add_custom_model("zero-cache-priced-model".to_string(), model_info);
    let mut usage = PricingUsage::new(1000, 100);
    usage.cached_tokens = Some(700);
    usage.cache_creation_tokens = Some(200);
    usage.cache_read_tokens = Some(500);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("openai", "zero-cache-priced-model", &usage)
        .unwrap();

    assert!((cost.input_cost - 0.003).abs() < f64::EPSILON);
    assert!((cost.output_cost - 0.003).abs() < f64::EPSILON);
    assert_eq!(cost.cache_cost, 0.0);
    assert!((cost.total_cost - 0.006).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_uses_input_price_when_cache_prices_are_missing() {
    let service = PricingService::new(None);
    let model_info = LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: Some(0.00001),
        output_cost_per_token: Some(0.00003),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "openai".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    };
    service.add_custom_model("cache-unpriced-model".to_string(), model_info);
    let mut usage = PricingUsage::new(1000, 100);
    usage.cached_tokens = Some(700);
    usage.cache_creation_tokens = Some(200);
    usage.cache_read_tokens = Some(500);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("openai", "cache-unpriced-model", &usage)
        .unwrap();

    assert!((cost.input_cost - 0.003).abs() < f64::EPSILON);
    assert!((cost.output_cost - 0.003).abs() < f64::EPSILON);
    assert!((cost.cache_cost - 0.007).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.013).abs() < f64::EPSILON);
}
