//! Anthropic Provider Cost Calculation
//!
//! Simple delegation to the unified cost calculation system

use crate::core::cost::{
    CostCalculator,
    calculator::{estimate_cost, generic_cost_per_token, get_model_pricing},
    types::{CostBreakdown, CostError, CostEstimate, ModelPricing, UsageTokens},
};
use async_trait::async_trait;

/// Anthropic Cost Calculator - delegates to generic implementation
#[derive(Debug, Clone, Default)]
pub struct AnthropicCostCalculator;

impl AnthropicCostCalculator {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CostCalculator for AnthropicCostCalculator {
    type Error = CostError;

    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error> {
        generic_cost_per_token(model, usage, "anthropic")
    }

    async fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error> {
        estimate_cost(model, "anthropic", input_tokens, max_output_tokens)
    }

    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error> {
        get_model_pricing(model, "anthropic")
    }

    fn provider_name(&self) -> &str {
        "anthropic"
    }
}

/// Helper function for easy cost calculation (maintains compatibility)
pub fn cost_per_token(model: &str, usage: &UsageTokens) -> Result<(f64, f64), CostError> {
    let breakdown = generic_cost_per_token(model, usage, "anthropic")?;
    Ok((breakdown.input_cost, breakdown.output_cost))
}

/// Get Anthropic model pricing (convenience function)
pub fn get_anthropic_model_pricing(model: &str) -> Result<ModelPricing, CostError> {
    get_model_pricing(model, "anthropic")
}

/// Helper function compatible with old API (returns Option)
pub fn calculate_anthropic_cost(model: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
    let usage = UsageTokens::new(input_tokens, output_tokens);
    generic_cost_per_token(model, &usage, "anthropic")
        .ok()
        .map(|b| b.total_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== AnthropicCostCalculator Tests ====================

    #[test]
    fn test_anthropic_cost_calculator_new() {
        let calc = AnthropicCostCalculator::new();
        assert_eq!(calc.provider_name(), "anthropic");
    }

    #[test]
    fn test_anthropic_cost_calculator_default() {
        let calc = AnthropicCostCalculator;
        assert_eq!(calc.provider_name(), "anthropic");
    }

    #[test]
    fn test_anthropic_cost_calculator_provider_name() {
        let calc = AnthropicCostCalculator;
        assert_eq!(calc.provider_name(), "anthropic");
    }

    #[test]
    fn test_anthropic_cost_calculator_clone() {
        let calc = AnthropicCostCalculator::new();
        let cloned = calc.clone();
        assert_eq!(calc.provider_name(), cloned.provider_name());
    }

    #[test]
    fn test_anthropic_cost_calculator_debug() {
        let calc = AnthropicCostCalculator::new();
        let debug_str = format!("{:?}", calc);
        assert!(debug_str.contains("AnthropicCostCalculator"));
    }

    // ==================== cost_per_token Tests ====================

    #[test]
    fn test_cost_per_token_claude_3_opus() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("claude-3-opus-20240229", &usage);

        let (input_cost, output_cost) = result.expect("known Anthropic model should be priced");
        assert!(input_cost > 0.0);
        assert!(output_cost > 0.0);
    }

    #[test]
    fn test_cost_per_token_claude_3_sonnet() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("claude-3-sonnet-20240229", &usage);

        let (input_cost, output_cost) = result.expect("known Anthropic model should be priced");
        assert!(input_cost > 0.0);
        assert!(output_cost > 0.0);
    }

    #[test]
    fn test_cost_per_token_claude_3_haiku() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("claude-3-haiku-20240307", &usage);

        let (input_cost, output_cost) = result.expect("known Anthropic model should be priced");
        assert!(input_cost > 0.0);
        assert!(output_cost > 0.0);
    }

    #[test]
    fn test_cost_per_token_unknown_model() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("unknown-model-xyz", &usage);

        // Should return error for unknown model
        assert!(result.is_err());
    }

    #[test]
    fn test_cost_per_token_zero_tokens() {
        let usage = UsageTokens::new(0, 0);
        let result = cost_per_token("claude-3-opus-20240229", &usage);

        if let Ok((input_cost, output_cost)) = result {
            assert_eq!(input_cost, 0.0);
            assert_eq!(output_cost, 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_input_only() {
        let usage = UsageTokens::new(1000, 0);
        let result = cost_per_token("claude-3-opus-20240229", &usage);

        if let Ok((input_cost, output_cost)) = result {
            assert!(input_cost >= 0.0);
            assert_eq!(output_cost, 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_output_only() {
        let usage = UsageTokens::new(0, 1000);
        let result = cost_per_token("claude-3-opus-20240229", &usage);

        if let Ok((input_cost, output_cost)) = result {
            assert_eq!(input_cost, 0.0);
            assert!(output_cost >= 0.0);
        }
    }

    // ==================== get_anthropic_model_pricing Tests ====================

    #[test]
    fn test_get_anthropic_model_pricing_claude_3_opus() {
        let result = get_anthropic_model_pricing("claude-3-opus-20240229");

        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
            assert_eq!(pricing.currency, "USD");
        }
    }

    #[test]
    fn test_get_anthropic_model_pricing_unknown_model() {
        let result = get_anthropic_model_pricing("unknown-anthropic-model");

        assert!(result.is_err());
    }

    // ==================== calculate_anthropic_cost Tests ====================

    #[test]
    fn test_calculate_anthropic_cost_valid_model() {
        let result = calculate_anthropic_cost("claude-3-opus-20240229", 1000, 500);

        // Returns Some if model is found, None otherwise
        if let Some(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_anthropic_cost_invalid_model() {
        let result = calculate_anthropic_cost("invalid-model", 1000, 500);

        // Should return None for invalid model
        assert!(result.is_none());
    }

    #[test]
    fn test_calculate_anthropic_cost_zero_tokens() {
        let result = calculate_anthropic_cost("claude-3-opus-20240229", 0, 0);

        if let Some(cost) = result {
            assert_eq!(cost, 0.0);
        }
    }

    #[test]
    fn test_calculate_anthropic_cost_large_tokens() {
        let result = calculate_anthropic_cost("claude-3-opus-20240229", 100000, 50000);

        // Should handle large token counts
        if let Some(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    // ==================== Async Tests ====================

    #[tokio::test]
    async fn test_anthropic_calculator_calculate_cost() {
        let calc = AnthropicCostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("claude-3-opus-20240229", &usage).await;

        if let Ok(breakdown) = result {
            assert_eq!(breakdown.provider, "anthropic");
            assert!(breakdown.total_cost >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_anthropic_calculator_calculate_cost_unknown_model() {
        let calc = AnthropicCostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("unknown-model", &usage).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_anthropic_calculator_estimate_cost() {
        let calc = AnthropicCostCalculator::new();

        let result = calc
            .estimate_cost("claude-3-opus-20240229", 1000, Some(500))
            .await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
            assert!(estimate.max_cost >= estimate.min_cost);
            assert_eq!(estimate.currency, "USD");
        }
    }

    #[tokio::test]
    async fn test_anthropic_calculator_estimate_cost_no_max_output() {
        let calc = AnthropicCostCalculator::new();

        let result = calc
            .estimate_cost("claude-3-opus-20240229", 1000, None)
            .await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
        }
    }

    #[test]
    fn test_anthropic_calculator_get_model_pricing() {
        let calc = AnthropicCostCalculator::new();

        let result = calc.get_model_pricing("claude-3-opus-20240229");

        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
        }
    }

    #[test]
    fn test_anthropic_calculator_get_model_pricing_unknown() {
        let calc = AnthropicCostCalculator::new();

        let result = calc.get_model_pricing("unknown-anthropic-model");

        assert!(result.is_err());
        if let Err(CostError::ModelNotSupported { model, provider }) = result {
            assert_eq!(model, "unknown-anthropic-model");
            assert_eq!(provider, "anthropic");
        }
    }

    // ==================== Model Variants Tests ====================

    #[test]
    fn test_anthropic_cost_with_prefix() {
        // Test model with anthropic/ prefix
        let prefixed = calculate_anthropic_cost("anthropic/claude-3-opus-20240229", 1000, 500);
        let canonical = calculate_anthropic_cost("claude-3-opus-20240229", 1000, 500);

        assert_eq!(prefixed, canonical);
        assert!(prefixed.is_some());
    }

    #[test]
    fn test_anthropic_cost_claude_2() {
        let result = calculate_anthropic_cost("claude-2.1", 1000, 500);
        let cost = result.expect("claude-2.1 should have anthropic pricing");
        assert!(cost > 0.0);
    }

    #[test]
    fn test_anthropic_cost_claude_instant() {
        let result = calculate_anthropic_cost("claude-instant-1.2", 1000, 500);
        let cost = result.expect("claude-instant-1.2 should have anthropic pricing");
        assert!(cost > 0.0);
    }
}
