use super::*;

// Tests for estimate_cost
#[test]
fn test_estimate_cost_basic() {
    let result = estimate_cost("gpt-4o-mini", "openai", 1000, Some(500));
    assert!(result.is_ok());
    let estimate = result.unwrap();

    let expected_input = (1000.0 / 1000.0) * 0.00015;
    let expected_output = (500.0 / 1000.0) * 0.0006;

    assert!((estimate.input_cost - expected_input).abs() < 1e-6);
    assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
    assert_eq!(estimate.min_cost, expected_input);
    assert!((estimate.max_cost - (expected_input + expected_output)).abs() < 1e-6);
    assert_eq!(estimate.currency, "USD");
}

#[test]
fn test_estimate_cost_no_max_output() {
    let result = estimate_cost("gpt-4o", "openai", 1000, None);
    assert!(result.is_ok());
    let estimate = result.unwrap();

    // Should use default 100 tokens
    let expected_output = (100.0 / 1000.0) * 0.01;
    assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
}

#[test]
fn test_estimate_cost_unsupported_model() {
    let result = estimate_cost("unknown-model", "openai", 1000, Some(500));
    assert!(result.is_err());
}

// Tests for compare_model_costs
#[test]
fn test_compare_model_costs_single_model() {
    let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
    let comparisons = compare_model_costs(&models, 1000, 500);

    assert_eq!(comparisons.len(), 1);
    assert_eq!(comparisons[0].model, "gpt-4o-mini");
    assert_eq!(comparisons[0].provider, "openai");
    assert!(comparisons[0].total_cost > 0.0);
    assert!(comparisons[0].cost_per_token > 0.0);
    assert!(comparisons[0].efficiency_score > 0.0);
}

#[test]
fn test_compare_model_costs_multiple_models() {
    let models = vec![
        ("gpt-4o".to_string(), "openai".to_string()),
        ("gpt-4o-mini".to_string(), "openai".to_string()),
        ("claude-3-haiku".to_string(), "anthropic".to_string()),
    ];
    let comparisons = compare_model_costs(&models, 1000, 500);

    assert_eq!(comparisons.len(), 3);

    // Should be sorted by cost (lowest first)
    for i in 1..comparisons.len() {
        assert!(comparisons[i - 1].total_cost <= comparisons[i].total_cost);
    }

    // Verify efficiency score calculation
    for comparison in &comparisons {
        let expected_efficiency = 1500.0 / comparison.total_cost;
        assert!((comparison.efficiency_score - expected_efficiency).abs() < 1e-6);
    }
}

#[test]
fn test_compare_model_costs_with_invalid_model() {
    let models = vec![
        ("gpt-4o-mini".to_string(), "openai".to_string()),
        ("invalid-model".to_string(), "openai".to_string()),
        ("claude-3-haiku".to_string(), "anthropic".to_string()),
    ];
    let comparisons = compare_model_costs(&models, 1000, 500);

    // Should only include valid models
    assert_eq!(comparisons.len(), 2);
}

#[test]
fn test_compare_model_costs_empty_list() {
    let models: Vec<(String, String)> = vec![];
    let comparisons = compare_model_costs(&models, 1000, 500);
    assert_eq!(comparisons.len(), 0);
}

#[test]
fn test_compare_model_costs_zero_tokens() {
    let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
    let comparisons = compare_model_costs(&models, 0, 0);

    // Should handle zero tokens gracefully
    assert_eq!(comparisons.len(), 1);
    assert_eq!(comparisons[0].total_cost, 0.0);
}
