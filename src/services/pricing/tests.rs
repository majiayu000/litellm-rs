//! Tests for the pricing service

#[cfg(test)]
use crate::services::pricing::{LiteLLMModelInfo, PricingService};
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
