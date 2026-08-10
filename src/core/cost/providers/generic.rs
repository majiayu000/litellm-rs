//! Generic Provider Cost Calculation
//!
//! Default implementation for providers without specific cost calculation needs

use crate::core::cost::{
    calculator::{CostCalculator, estimate_cost, generic_cost_per_token, get_model_pricing},
    types::{CostBreakdown, CostError, CostEstimate, ModelPricing, UsageTokens},
};
use async_trait::async_trait;

/// Generic Cost Calculator that can be used by any provider
#[derive(Debug, Clone)]
pub struct GenericCostCalculator {
    provider_name: String,
}

impl GenericCostCalculator {
    pub fn new(provider_name: String) -> Self {
        Self { provider_name }
    }
}

#[async_trait]
impl CostCalculator for GenericCostCalculator {
    type Error = CostError;

    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error> {
        generic_cost_per_token(model, usage, &self.provider_name)
    }

    async fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error> {
        estimate_cost(model, &self.provider_name, input_tokens, max_output_tokens)
    }

    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error> {
        get_model_pricing(model, &self.provider_name)
    }

    fn provider_name(&self) -> &str {
        &self.provider_name
    }
}

/// Creates a generic cost calculator for any provider
pub fn create_generic_calculator(provider: &str) -> GenericCostCalculator {
    GenericCostCalculator::new(provider.to_string())
}

/// Simple stub cost calculator that returns zero costs
/// Useful for providers without pricing information
#[derive(Debug, Clone)]
pub struct StubCostCalculator {
    provider_name: String,
}

impl StubCostCalculator {
    pub fn new(provider_name: String) -> Self {
        Self { provider_name }
    }
}

#[async_trait]
impl CostCalculator for StubCostCalculator {
    type Error = CostError;

    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error> {
        let mut breakdown =
            CostBreakdown::new(model.to_string(), self.provider_name.clone(), usage.clone());
        // Return zero costs for stub
        breakdown.calculate_total();
        Ok(breakdown)
    }

    async fn estimate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error> {
        Ok(CostEstimate {
            min_cost: 0.0,
            max_cost: 0.0,
            input_cost: 0.0,
            estimated_output_cost: 0.0,
            currency: "USD".to_string(),
        })
    }

    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error> {
        Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: self.provider_name.clone(),
        })
    }

    fn provider_name(&self) -> &str {
        &self.provider_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== GenericCostCalculator Tests ====================

    #[test]
    fn test_generic_cost_calculator_new() {
        let calc = GenericCostCalculator::new("openai".to_string());
        assert_eq!(calc.provider_name(), "openai");
    }

    #[test]
    fn test_generic_cost_calculator_provider_name() {
        let calc = GenericCostCalculator::new("anthropic".to_string());
        assert_eq!(calc.provider_name(), "anthropic");
    }

    #[test]
    fn test_generic_cost_calculator_clone() {
        let calc = GenericCostCalculator::new("azure".to_string());
        let cloned = calc.clone();
        assert_eq!(calc.provider_name(), cloned.provider_name());
    }

    #[test]
    fn test_generic_cost_calculator_debug() {
        let calc = GenericCostCalculator::new("test".to_string());
        let debug_str = format!("{:?}", calc);
        assert!(debug_str.contains("GenericCostCalculator"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_create_generic_calculator() {
        let calc = create_generic_calculator("groq");
        assert_eq!(calc.provider_name(), "groq");
    }

    #[test]
    fn test_create_generic_calculator_different_providers() {
        let providers = ["openai", "anthropic", "azure", "groq", "deepseek"];
        for provider in providers {
            let calc = create_generic_calculator(provider);
            assert_eq!(calc.provider_name(), provider);
        }
    }

    // ==================== StubCostCalculator Tests ====================

    #[test]
    fn test_stub_cost_calculator_new() {
        let calc = StubCostCalculator::new("test".to_string());
        assert_eq!(calc.provider_name(), "test");
    }

    #[test]
    fn test_stub_cost_calculator_provider_name() {
        let calc = StubCostCalculator::new("custom".to_string());
        assert_eq!(calc.provider_name(), "custom");
    }

    #[test]
    fn test_stub_cost_calculator_clone() {
        let calc = StubCostCalculator::new("stub".to_string());
        let cloned = calc.clone();
        assert_eq!(calc.provider_name(), cloned.provider_name());
    }

    #[test]
    fn test_stub_cost_calculator_debug() {
        let calc = StubCostCalculator::new("debug_test".to_string());
        let debug_str = format!("{:?}", calc);
        assert!(debug_str.contains("StubCostCalculator"));
        assert!(debug_str.contains("debug_test"));
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_calculate_cost() {
        let calc = StubCostCalculator::new("stub".to_string());
        let usage = UsageTokens::new(100, 50);

        let result = calc.calculate_cost("gpt-4", &usage).await;
        assert!(result.is_ok());

        let breakdown = result.unwrap();
        assert_eq!(breakdown.model, "gpt-4");
        assert_eq!(breakdown.provider, "stub");
        // Stub returns zero cost
        assert_eq!(breakdown.total_cost, 0.0);
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_calculate_cost_any_model() {
        let calc = StubCostCalculator::new("stub".to_string());
        let usage = UsageTokens::new(1000, 500);

        // Should work with any model name
        let models = ["gpt-4", "claude-3", "unknown-model", "test/model"];
        for model in models {
            let result = calc.calculate_cost(model, &usage).await;
            assert!(result.is_ok());
            let breakdown = result.unwrap();
            assert_eq!(breakdown.model, model);
            assert_eq!(breakdown.total_cost, 0.0);
        }
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_estimate_cost() {
        let calc = StubCostCalculator::new("stub".to_string());

        let result = calc.estimate_cost("gpt-4", 100, Some(500)).await;
        assert!(result.is_ok());

        let estimate = result.unwrap();
        assert_eq!(estimate.min_cost, 0.0);
        assert_eq!(estimate.max_cost, 0.0);
        assert_eq!(estimate.input_cost, 0.0);
        assert_eq!(estimate.estimated_output_cost, 0.0);
        assert_eq!(estimate.currency, "USD");
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_estimate_cost_no_max_output() {
        let calc = StubCostCalculator::new("stub".to_string());

        let result = calc.estimate_cost("claude-3", 200, None).await;
        assert!(result.is_ok());

        let estimate = result.unwrap();
        assert_eq!(estimate.min_cost, 0.0);
        assert_eq!(estimate.max_cost, 0.0);
    }

    #[test]
    fn test_stub_cost_calculator_get_model_pricing_error() {
        let calc = StubCostCalculator::new("stub".to_string());

        let result = calc.get_model_pricing("gpt-4");
        assert!(result.is_err());

        let error = result.unwrap_err();
        match error {
            CostError::ModelNotSupported { model, provider } => {
                assert_eq!(model, "gpt-4");
                assert_eq!(provider, "stub");
            }
            _ => panic!("Expected ModelNotSupported error"),
        }
    }

    #[test]
    fn test_stub_cost_calculator_get_model_pricing_different_models() {
        let calc = StubCostCalculator::new("custom_provider".to_string());

        let models = ["model-a", "model-b", "unknown"];
        for model in models {
            let result = calc.get_model_pricing(model);
            assert!(result.is_err());

            if let CostError::ModelNotSupported {
                model: m,
                provider: p,
            } = result.unwrap_err()
            {
                assert_eq!(m, model);
                assert_eq!(p, "custom_provider");
            }
        }
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_stub_vs_generic_zero_cost() {
        let stub = StubCostCalculator::new("stub".to_string());
        let usage = UsageTokens::new(100, 50);

        let stub_result = stub.calculate_cost("unknown-model", &usage).await;
        assert!(stub_result.is_ok());
        let stub_breakdown = stub_result.unwrap();

        // Stub should always return zero cost
        assert_eq!(stub_breakdown.total_cost, 0.0);
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_large_usage() {
        let calc = StubCostCalculator::new("stub".to_string());
        let usage = UsageTokens::new(1_000_000, 500_000);

        let result = calc.calculate_cost("expensive-model", &usage).await;
        assert!(result.is_ok());

        let breakdown = result.unwrap();
        // Should still be zero for stub
        assert_eq!(breakdown.total_cost, 0.0);
    }

    #[tokio::test]
    async fn test_stub_cost_calculator_preserves_usage() {
        let calc = StubCostCalculator::new("stub".to_string());
        let usage = UsageTokens::new(250, 150);

        let result = calc.calculate_cost("model", &usage).await;
        let breakdown = result.unwrap();

        assert_eq!(breakdown.usage.prompt_tokens, 250);
        assert_eq!(breakdown.usage.completion_tokens, 150);
        assert_eq!(breakdown.usage.total_tokens, 400);
    }
}
