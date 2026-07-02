use super::*;

// Helper function to create basic usage
fn create_usage(prompt_tokens: u32, completion_tokens: u32) -> UsageTokens {
    UsageTokens::new(prompt_tokens, completion_tokens)
}

fn assert_cost_eq(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

mod component_cost_tests;
mod edge_case_tests;
mod estimation_comparison_tests;
mod pricing_lookup_tests;
mod workflow_tests;
