use super::*;

// Integration tests
#[test]
fn test_cost_calculation_workflow() {
    // Simulate a complete workflow
    let usage = create_usage(2000, 1000);

    // 1. Get pricing
    let pricing = get_model_pricing("gpt-4o-mini", "openai");
    assert!(pricing.is_ok());

    // 2. Calculate cost
    let breakdown = generic_cost_per_token("gpt-4o-mini", &usage, "openai");
    assert!(breakdown.is_ok());
    let breakdown = breakdown.unwrap();

    // 3. Verify breakdown structure
    assert_eq!(breakdown.model, "gpt-4o-mini");
    assert_eq!(breakdown.provider, "openai");
    assert_eq!(breakdown.currency, "USD");
    assert!(breakdown.total_cost > 0.0);
    assert_eq!(breakdown.usage.total_tokens, 3000);
}

#[test]
fn test_estimate_and_actual_cost_consistency() {
    let input_tokens = 1000;
    let output_tokens = 500;

    // Estimate cost
    let estimate = estimate_cost("gpt-4o", "openai", input_tokens, Some(output_tokens));
    assert!(estimate.is_ok());
    let estimate = estimate.unwrap();

    // Calculate actual cost
    let usage = create_usage(input_tokens, output_tokens);
    let breakdown = generic_cost_per_token("gpt-4o", &usage, "openai");
    assert!(breakdown.is_ok());
    let breakdown = breakdown.unwrap();

    // Actual cost should match estimate max_cost
    assert!((breakdown.total_cost - estimate.max_cost).abs() < 1e-10);
    assert!((breakdown.input_cost - estimate.input_cost).abs() < 1e-10);
}
