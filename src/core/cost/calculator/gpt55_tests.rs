use super::{estimate_cost, generic_cost_per_token, get_model_pricing};
use crate::core::cost::types::UsageTokens;

#[test]
fn gpt55_pricing_includes_cache_and_long_context_tiers() {
    let pricing = get_model_pricing("gpt-5.5", "openai")
        .expect("gpt-5.5 should have OpenAI fallback pricing");

    assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
    assert_eq!(pricing.output_cost_per_1k_tokens, 0.030);
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.0005));

    let tiered = pricing
        .tiered_pricing
        .as_ref()
        .expect("gpt-5.5 should include long-context tiered pricing");
    assert_eq!(
        tiered.get("input_cost_per_token_above_272k_tokens"),
        Some(&0.010)
    );
    assert_eq!(
        tiered.get("output_cost_per_token_above_272k_tokens"),
        Some(&0.045)
    );
    assert_eq!(
        tiered.get("cache_read_input_token_cost_above_272k_tokens"),
        Some(&0.001)
    );
}

#[test]
fn gpt55_cached_and_long_context_costs_are_charged() {
    let mut cached_usage = UsageTokens::new(1_000, 100);
    cached_usage.cached_tokens = Some(400);

    let cached_breakdown = generic_cost_per_token("gpt-5.5", &cached_usage, "openai").unwrap();
    assert!((cached_breakdown.input_cost - 0.003).abs() < 1e-12);
    assert!((cached_breakdown.cache_cost - 0.0002).abs() < 1e-12);
    assert!((cached_breakdown.output_cost - 0.003).abs() < 1e-12);
    assert!((cached_breakdown.total_cost - 0.0062).abs() < 1e-12);

    let mut long_context_usage = UsageTokens::new(300_000, 2_000);
    long_context_usage.cached_tokens = Some(50_000);

    let long_context_breakdown =
        generic_cost_per_token("gpt-5.5", &long_context_usage, "openai").unwrap();
    assert!((long_context_breakdown.input_cost - 2.5).abs() < 1e-12);
    assert!((long_context_breakdown.cache_cost - 0.05).abs() < 1e-12);
    assert!((long_context_breakdown.output_cost - 0.09).abs() < 1e-12);
    assert!((long_context_breakdown.total_cost - 2.64).abs() < 1e-12);
}

#[test]
fn gpt55_long_context_estimates_use_tiered_rates() {
    let Ok(estimate) = estimate_cost("gpt-5.5", "openai", 300_000, Some(2_000)) else {
        panic!("gpt-5.5 long-context estimate should succeed");
    };

    assert!((estimate.input_cost - 3.0).abs() < 1e-12);
    assert!((estimate.estimated_output_cost - 0.09).abs() < 1e-12);
    assert!((estimate.max_cost - 3.09).abs() < 1e-12);
}

#[test]
fn gpt55_pro_cached_tokens_are_not_free() {
    let pricing = get_model_pricing("gpt-5.5-pro", "openai")
        .expect("gpt-5.5-pro should have OpenAI fallback pricing");
    assert_eq!(pricing.cache_read_input_token_cost, Some(0.030));

    let mut usage = UsageTokens::new(1_000, 100);
    usage.cached_tokens = Some(400);

    let breakdown = generic_cost_per_token("gpt-5.5-pro", &usage, "openai").unwrap();
    assert!((breakdown.input_cost - 0.018).abs() < 1e-12);
    assert!((breakdown.cache_cost - 0.012).abs() < 1e-12);
    assert!((breakdown.output_cost - 0.018).abs() < 1e-12);
    assert!((breakdown.total_cost - 0.048).abs() < 1e-12);
}
