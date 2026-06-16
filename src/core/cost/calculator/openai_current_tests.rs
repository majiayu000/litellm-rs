use super::{generic_cost_per_token, get_model_pricing};
use crate::core::cost::types::UsageTokens;

#[test]
fn openai_gpt54_pricing_includes_cache_and_long_context_tiers() {
    let pricing = get_model_pricing("gpt-5.4", "openai")
        .expect("gpt-5.4 should have OpenAI fallback pricing");

    assert_eq!(pricing.input_cost_per_1k_tokens, 0.0025);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.015);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00025));

    let tiered = pricing
        .tiered_pricing
        .as_ref()
        .expect("gpt-5.4 should include long-context tiered pricing");
    assert_eq!(
        tiered.get("input_cost_per_token_above_272k_tokens"),
        Some(&0.005)
    );
    assert_eq!(
        tiered.get("output_cost_per_token_above_272k_tokens"),
        Some(&0.0225)
    );
}

#[test]
fn openai_gpt_image_2_pricing_uses_current_image_rates() {
    let pricing = get_model_pricing("gpt-image-2", "openai")
        .expect("gpt-image-2 should have OpenAI fallback pricing");

    assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.030);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.00125));
    assert_eq!(pricing.image_cost_per_token, Some(0.000008));
}

#[test]
fn openai_embedding_v3_pricing_is_reachable_from_extended_catalog() {
    let pricing = get_model_pricing("text-embedding-3-large", "openai")
        .expect("text-embedding-3-large should be priced from catalog");

    assert_eq!(pricing.input_cost_per_1k_tokens, 0.00013);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.0);
}

#[test]
fn openai_gpt_image_2_image_tokens_are_charged_separately() {
    let mut usage = UsageTokens::new(1_000, 100);
    usage.image_tokens = Some(500);

    let breakdown = generic_cost_per_token("gpt-image-2", &usage, "openai")
        .expect("gpt-image-2 image cost should calculate");

    assert!((breakdown.input_cost - 0.005).abs() < 1e-12);
    assert!((breakdown.output_cost - 0.003).abs() < 1e-12);
    assert!((breakdown.image_cost - 0.004).abs() < 1e-12);
}
