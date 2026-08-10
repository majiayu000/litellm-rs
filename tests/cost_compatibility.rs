use litellm_rs::core::cost::calculator::{CostCalculator, generic_cost_per_token};
use litellm_rs::core::cost::types::{CostResult, UsageTokens};
use litellm_rs::core::pricing_service::{PricingService, PricingUsage};

#[test]
fn compatibility_modules_remain_source_compatible() {
    let result = CostResult::new(0.1, 0.2).with_additional_cost("cache".into(), 0.05);
    assert!((result.total_cost - 0.35).abs() < f64::EPSILON);

    let calculator = litellm_rs::core::cost::providers::openai::OpenAICostCalculator::new();
    assert_eq!(calculator.provider_name(), "openai");
    let _: &dyn CostCalculator<Error = litellm_rs::core::cost::CostError> = &calculator;
}

#[test]
fn legacy_adapter_matches_pricing_authority_and_unknowns_fail_closed() {
    let usage = UsageTokens::new(1_000, 500);
    let legacy = generic_cost_per_token("gpt-4o-mini", &usage, "openai").unwrap();
    let authority = PricingService::with_embedded_default().unwrap();
    let canonical = authority
        .calculate_loaded_usage_cost_for_provider(
            "openai",
            "gpt-4o-mini",
            &PricingUsage::new(1_000, 500),
        )
        .unwrap();
    assert!((legacy.total_cost - canonical.total_cost).abs() < 1e-12);

    assert!(generic_cost_per_token("unknown-cost-model", &usage, "openai").is_err());
    assert!(
        authority
            .calculate_loaded_usage_cost_for_provider(
                "openai",
                "unknown-cost-model",
                &PricingUsage::new(1_000, 500),
            )
            .is_err()
    );
}
