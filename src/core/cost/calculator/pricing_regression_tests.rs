use super::*;

fn model_info_from_json(value: serde_json::Value) -> crate::core::pricing::LiteLLMModelInfo {
    serde_json::from_value(value).expect("valid LiteLLMModelInfo json")
}

#[test]
fn test_litellm_pricing_errors_when_both_token_costs_missing() {
    // A catalog entry with neither input nor output cost must not bill at $0.
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat"
    }));
    let result = litellm_to_cost_pricing("mystery-model", &info);
    assert!(matches!(
        result,
        Err(CostError::MissingPricing { ref model }) if model == "mystery-model"
    ));
}

#[test]
fn test_litellm_pricing_errors_when_chat_has_single_missing_side() {
    // Chat completions use prompt and completion tokens, so a missing side
    // must not be billed at $0.
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat",
        "input_cost_per_token": 0.000_01
    }));
    let result = litellm_to_cost_pricing("half-priced-chat", &info);
    assert!(matches!(
        result,
        Err(CostError::MissingPricing { ref model }) if model == "half-priced-chat"
    ));
}

#[test]
fn test_litellm_pricing_allows_single_missing_side_for_embedding() {
    // Embeddings can have only input-side token pricing.
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "embedding",
        "input_cost_per_token": 0.000_01
    }));
    let pricing = litellm_to_cost_pricing("half-priced", &info).expect("should be priced");
    assert!(pricing.input_cost_per_1k_tokens > 0.0);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0);
}

#[test]
fn test_litellm_pricing_ok_when_both_present() {
    let info = model_info_from_json(serde_json::json!({
        "litellm_provider": "openai",
        "mode": "chat",
        "input_cost_per_token": 0.000_01,
        "output_cost_per_token": 0.000_03
    }));
    let pricing = litellm_to_cost_pricing("full", &info).expect("should be priced");
    assert!(pricing.input_cost_per_1k_tokens > 0.0);
    assert!(pricing.output_cost_per_1k_tokens > 0.0);
}
