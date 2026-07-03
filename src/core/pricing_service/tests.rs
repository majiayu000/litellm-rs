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

fn flat_image_model_info(output_cost_per_image: Option<f64>) -> LiteLLMModelInfo {
    let mut extra = HashMap::new();
    if let Some(price) = output_cost_per_image {
        extra.insert(
            "output_cost_per_image".to_string(),
            serde_json::Value::from(price),
        );
    }
    LiteLLMModelInfo {
        max_tokens: None,
        max_input_tokens: None,
        max_output_tokens: None,
        input_cost_per_token: None,
        output_cost_per_token: None,
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "bedrock".to_string(),
        mode: "image_generation".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra,
    }
}

#[test]
fn provider_pricing_dry_run_charges_audio_tokens_with_audio_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.mode = "audio_transcription".to_string();
    model_info.extra.insert(
        "input_cost_per_audio_token".to_string(),
        serde_json::Value::from(0.002),
    );
    service.add_custom_model("audio-priced-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.audio_tokens = Some(250);

    let cost = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "audio-priced-model", &usage)
        .unwrap();

    assert_eq!(cost.audio_cost, 0.5);
    assert_eq!(cost.total_cost, 0.5);
}

#[test]
fn provider_pricing_dry_run_charges_audio_tokens_with_output_audio_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.mode = "audio_transcription".to_string();
    model_info.extra.insert(
        "output_cost_per_audio_token".to_string(),
        serde_json::Value::from(0.003),
    );
    service.add_custom_model("output-audio-priced-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.audio_tokens = Some(200);

    let cost = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "output-audio-priced-model", &usage)
        .unwrap();

    assert_eq!(cost.audio_cost, 0.6);
    assert_eq!(cost.total_cost, 0.6);
}

#[test]
fn provider_pricing_dry_run_fails_closed_for_missing_audio_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.mode = "audio_transcription".to_string();
    service.add_custom_model("missing-audio-price".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.audio_tokens = Some(250);

    let error = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "missing-audio-price", &usage)
        .unwrap_err();

    assert!(error.to_string().contains("input_cost_per_audio_token"));
    assert!(error.to_string().contains("output_cost_per_audio_token"));
}

#[test]
fn provider_pricing_charges_flat_output_image_cost_without_token_prices() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "flat-image-model".to_string(),
        flat_image_model_info(Some(0.06)),
    );
    let mut usage = PricingUsage::new(25, 0);
    usage.image_tokens = Some(300);
    usage.output_image_count = Some(3);

    let cost = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "flat-image-model", &usage)
        .unwrap();

    assert_eq!(cost.input_cost, 0.0);
    assert!((cost.image_cost - 0.18).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.18).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_fails_closed_for_missing_flat_output_image_price() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "missing-image-price".to_string(),
        flat_image_model_info(None),
    );
    let mut usage = PricingUsage::new(0, 0);
    usage.output_image_count = Some(1);

    let error = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "missing-image-price", &usage)
        .unwrap_err();

    assert!(error.to_string().contains("output_cost_per_image"));
}

#[test]
fn provider_pricing_fails_closed_for_invalid_flat_output_image_price() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "invalid-image-price".to_string(),
        flat_image_model_info(Some(-0.1)),
    );
    let mut usage = PricingUsage::new(0, 0);
    usage.output_image_count = Some(1);

    let error = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "invalid-image-price", &usage)
        .unwrap_err();

    assert!(error.to_string().contains("Invalid image pricing"));
}

#[test]
fn provider_pricing_prefers_image_token_price_over_flat_output_image_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(Some(0.06));
    model_info.extra.insert(
        "image_cost_per_token".to_string(),
        serde_json::Value::from(0.01),
    );
    service.add_custom_model("image-token-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.image_tokens = Some(100);
    usage.output_image_count = Some(3);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "image-token-model", &usage)
        .unwrap();

    assert!((cost.image_cost - 1.0).abs() < f64::EPSILON);
    assert!((cost.total_cost - 1.0).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_treats_explicit_zero_image_token_price_as_present() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.extra.insert(
        "image_cost_per_token".to_string(),
        serde_json::Value::from(0.0),
    );
    service.add_custom_model("zero-image-token-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.image_tokens = Some(100);
    usage.output_image_count = Some(3);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "zero-image-token-model", &usage)
        .unwrap();

    assert_eq!(cost.image_cost, 0.0);
    assert_eq!(cost.total_cost, 0.0);
}

#[test]
fn provider_pricing_fails_closed_for_token_priced_image_usage_without_image_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.input_cost_per_token = Some(0.01);
    model_info.output_cost_per_token = Some(0.0);
    service.add_custom_model("token-priced-image-model".to_string(), model_info);
    let mut usage = PricingUsage::new(2, 0);
    usage.image_tokens = Some(100);
    usage.output_image_count = Some(1);

    let error = service
        .dry_run_loaded_usage_cost_for_provider("bedrock", "token-priced-image-model", &usage)
        .unwrap_err();

    let error = error.to_string();
    assert!(error.contains("image_cost_per_token"));
    assert!(error.contains("output_cost_per_image"));
}

#[test]
fn provider_pricing_fails_closed_for_mismatched_flat_image_variant() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "1024-x-1024/50-steps/flat-variant-model".to_string(),
        flat_image_model_info(Some(0.06)),
    );
    let mut usage = PricingUsage::new(0, 0);
    usage.output_image_count = Some(1);
    usage
        .output_image_pricing_keys
        .push("1024-x-1024/flat-variant-model".to_string());

    let error = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "flat-variant-model", &usage)
        .unwrap_err();

    assert!(error.to_string().contains("output_image_pricing_keys"));
}

#[test]
fn provider_pricing_charges_matching_flat_image_variant() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "1024-x-1024/flat-variant-model".to_string(),
        flat_image_model_info(Some(0.06)),
    );
    let mut usage = PricingUsage::new(0, 0);
    usage.output_image_count = Some(2);
    usage
        .output_image_pricing_keys
        .push("1024-x-1024/flat-variant-model".to_string());

    let cost = service
        .calculate_loaded_usage_cost_for_provider(
            "bedrock",
            "1024-x-1024/flat-variant-model",
            &usage,
        )
        .unwrap();

    assert!((cost.image_cost - 0.12).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_charges_output_image_token_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.extra.insert(
        "output_cost_per_image_token".to_string(),
        serde_json::Value::from(0.03),
    );
    service.add_custom_model("image-token-output-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.image_tokens = Some(4);
    usage.output_image_count = Some(1);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "image-token-output-model", &usage)
        .unwrap();

    assert!((cost.image_cost - 0.12).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.12).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_charges_input_image_token_price() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(None);
    model_info.extra.insert(
        "input_cost_per_image_token".to_string(),
        serde_json::Value::from(0.05),
    );
    service.add_custom_model("image-token-input-model".to_string(), model_info);
    let mut usage = PricingUsage::new(0, 0);
    usage.image_tokens = Some(4);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "image-token-input-model", &usage)
        .unwrap();

    assert!((cost.image_cost - 0.2).abs() < f64::EPSILON);
    assert!((cost.total_cost - 0.2).abs() < f64::EPSILON);
}

#[test]
fn provider_pricing_ignores_optional_flat_image_price_for_text_usage() {
    let service = PricingService::new(None);
    let mut model_info = flat_image_model_info(Some(0.06));
    model_info.input_cost_per_token = Some(0.01);
    model_info.output_cost_per_token = Some(0.02);
    service.add_custom_model("text-with-image-output-model".to_string(), model_info);
    let usage = PricingUsage::new(2, 3);

    let cost = service
        .calculate_loaded_usage_cost_for_provider("bedrock", "text-with-image-output-model", &usage)
        .unwrap();

    assert!((cost.input_cost - 0.02).abs() < f64::EPSILON);
    assert!((cost.output_cost - 0.06).abs() < f64::EPSILON);
    assert_eq!(cost.image_cost, 0.0);
    assert!((cost.total_cost - 0.08).abs() < f64::EPSILON);
}
