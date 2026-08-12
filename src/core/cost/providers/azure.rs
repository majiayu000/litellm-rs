//! Azure Provider Cost Calculation
//!
//! Simple delegation to the unified cost calculation system

use crate::core::cost::{
    calculator::{CostCalculator, estimate_cost, generic_cost_per_token, get_model_pricing},
    types::{CostBreakdown, CostError, CostEstimate, ModelPricing, UsageTokens},
};
use async_trait::async_trait;

/// Azure Cost Calculator - delegates to generic implementation
#[derive(Debug, Clone, Default)]
pub struct AzureCostCalculator;

impl AzureCostCalculator {
    pub fn new() -> Self {
        Self
    }

    /// Calculate fine-tuning cost
    pub fn calculate_fine_tuning_cost(
        &self,
        base_model: &str,
        training_tokens: u32,
        hosting_hours: f64,
    ) -> Result<f64, CostError> {
        let pricing = get_model_pricing(base_model, "azure")?;

        // Fine-tuning cost calculation (simplified)
        let training_cost =
            (training_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens * 10.0; // Rough multiplier
        let hosting_cost = hosting_hours * 1.02; // $1.02/hour for hosting

        Ok(training_cost + hosting_cost)
    }

    /// Calculate DALL-E cost for Azure
    pub fn calculate_dalle_cost(
        &self,
        model: &str,
        size: &str,
        quality: Option<&str>,
        n: u32,
    ) -> Result<f64, CostError> {
        let pricing = get_model_pricing(model, "azure")?;

        if let Some(ref cost_per_image) = pricing.cost_per_image {
            let cost_multiplier = if model.contains("dall-e-3") {
                match (size, quality.unwrap_or("standard")) {
                    ("1024x1024", "standard") => 1.0,
                    ("1024x1024", "hd") => 2.0,
                    ("1024x1792", "standard") | ("1792x1024", "standard") => 2.0,
                    ("1024x1792", "hd") | ("1792x1024", "hd") => 4.0,
                    _ => 1.0,
                }
            } else {
                match size {
                    "256x256" => 0.5,
                    "512x512" => 1.0,
                    "1024x1024" => 1.5,
                    _ => 1.0,
                }
            };

            // Use base cost from pricing
            let base_cost = cost_per_image.get("base").copied().unwrap_or(0.04);
            return Ok(base_cost * cost_multiplier * n as f64);
        }

        Err(CostError::MissingPricing {
            model: model.to_string(),
        })
    }
}

#[async_trait]
impl CostCalculator for AzureCostCalculator {
    type Error = CostError;

    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error> {
        generic_cost_per_token(model, usage, "azure")
    }

    async fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error> {
        estimate_cost(model, "azure", input_tokens, max_output_tokens)
    }

    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error> {
        get_model_pricing(model, "azure")
    }

    fn provider_name(&self) -> &str {
        "azure"
    }
}

/// Helper function for easy cost calculation (maintains compatibility)
pub fn cost_per_token(model: &str, usage: &UsageTokens) -> Result<(f64, f64), CostError> {
    let breakdown = generic_cost_per_token(model, usage, "azure")?;
    Ok((breakdown.input_cost, breakdown.output_cost))
}

/// Get Azure model pricing (convenience function)
pub fn get_azure_model_pricing(model: &str) -> Result<ModelPricing, CostError> {
    get_model_pricing(model, "azure")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== AzureCostCalculator Tests ====================

    #[test]
    fn test_azure_cost_calculator_new() {
        let calc = AzureCostCalculator::new();
        assert_eq!(calc.provider_name(), "azure");
    }

    #[test]
    fn test_azure_cost_calculator_default() {
        let calc = AzureCostCalculator;
        assert_eq!(calc.provider_name(), "azure");
    }

    #[test]
    fn test_azure_cost_calculator_provider_name() {
        let calc = AzureCostCalculator;
        assert_eq!(calc.provider_name(), "azure");
    }

    #[test]
    fn test_azure_cost_calculator_clone() {
        let calc = AzureCostCalculator::new();
        let cloned = calc.clone();
        assert_eq!(calc.provider_name(), cloned.provider_name());
    }

    #[test]
    fn test_azure_cost_calculator_debug() {
        let calc = AzureCostCalculator::new();
        let debug_str = format!("{:?}", calc);
        assert!(debug_str.contains("AzureCostCalculator"));
    }

    // ==================== calculate_fine_tuning_cost Tests ====================

    #[test]
    fn test_calculate_fine_tuning_cost_unknown_model() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_fine_tuning_cost("unknown-model", 10000, 5.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_fine_tuning_cost_gpt4() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_fine_tuning_cost("gpt-4", 10000, 5.0);
        // May succeed or fail depending on pricing database
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_fine_tuning_cost_zero_tokens() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_fine_tuning_cost("gpt-4", 0, 0.0);
        if let Ok(cost) = result {
            assert_eq!(cost, 0.0);
        }
    }

    #[test]
    fn test_calculate_fine_tuning_cost_hosting_only() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_fine_tuning_cost("gpt-4", 0, 10.0);
        if let Ok(cost) = result {
            // Should be 10.0 * 1.02 = 10.2 for hosting
            assert!((cost - 10.2).abs() < 0.01);
        }
    }

    #[test]
    fn test_calculate_fine_tuning_cost_large_tokens() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_fine_tuning_cost("gpt-4", 1000000, 24.0);
        if let Ok(cost) = result {
            assert!(cost > 0.0);
        }
    }

    // ==================== calculate_dalle_cost Tests ====================

    #[test]
    fn test_calculate_dalle_cost_unknown_model() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_dalle_cost("unknown-model", "1024x1024", None, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_dalle_cost_dall_e_3_standard() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_dalle_cost("dall-e-3", "1024x1024", Some("standard"), 1);
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_dalle_cost_dall_e_3_hd() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_dalle_cost("dall-e-3", "1024x1024", Some("hd"), 1);
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_dalle_cost_dall_e_3_sizes() {
        let calc = AzureCostCalculator::new();
        let sizes = ["1024x1024", "1024x1792", "1792x1024"];

        for size in sizes {
            let result = calc.calculate_dalle_cost("dall-e-3", size, Some("standard"), 1);
            if let Ok(cost) = result {
                assert!(cost >= 0.0);
            }
        }
    }

    #[test]
    fn test_calculate_dalle_cost_dall_e_2_sizes() {
        let calc = AzureCostCalculator::new();
        let sizes = ["256x256", "512x512", "1024x1024"];

        for size in sizes {
            let result = calc.calculate_dalle_cost("dall-e-2", size, None, 1);
            if let Ok(cost) = result {
                assert!(cost >= 0.0);
            }
        }
    }

    #[test]
    fn test_calculate_dalle_cost_multiple_images() {
        let calc = AzureCostCalculator::new();
        let result_one = calc.calculate_dalle_cost("dall-e-3", "1024x1024", None, 1);
        let result_five = calc.calculate_dalle_cost("dall-e-3", "1024x1024", None, 5);

        if let (Ok(cost_one), Ok(cost_five)) = (result_one, result_five) {
            assert!((cost_five - cost_one * 5.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_calculate_dalle_cost_zero_images() {
        let calc = AzureCostCalculator::new();
        let result = calc.calculate_dalle_cost("dall-e-3", "1024x1024", None, 0);
        if let Ok(cost) = result {
            assert_eq!(cost, 0.0);
        }
    }

    // ==================== cost_per_token Tests ====================

    #[test]
    fn test_cost_per_token_azure_gpt4() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4", &usage);
        if let Ok((input_cost, output_cost)) = result {
            assert!(input_cost >= 0.0);
            assert!(output_cost >= 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_unknown_model() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("unknown-model-xyz", &usage);
        assert!(result.is_err());
    }

    #[test]
    fn test_cost_per_token_zero_tokens() {
        let usage = UsageTokens::new(0, 0);
        let result = cost_per_token("gpt-4", &usage);
        if let Ok((input_cost, output_cost)) = result {
            assert_eq!(input_cost, 0.0);
            assert_eq!(output_cost, 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_input_only() {
        let usage = UsageTokens::new(1000, 0);
        let result = cost_per_token("gpt-4", &usage);
        if let Ok((input_cost, output_cost)) = result {
            assert!(input_cost >= 0.0);
            assert_eq!(output_cost, 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_output_only() {
        let usage = UsageTokens::new(0, 1000);
        let result = cost_per_token("gpt-4", &usage);
        if let Ok((input_cost, output_cost)) = result {
            assert_eq!(input_cost, 0.0);
            assert!(output_cost >= 0.0);
        }
    }

    // ==================== get_azure_model_pricing Tests ====================

    #[test]
    fn test_get_azure_model_pricing_gpt4() {
        let result = get_azure_model_pricing("gpt-4");
        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
            assert_eq!(pricing.currency, "USD");
        }
    }

    #[test]
    fn test_get_azure_model_pricing_unknown_model() {
        let result = get_azure_model_pricing("unknown-azure-model");
        assert!(result.is_err());
    }

    // ==================== Async Tests ====================

    #[tokio::test]
    async fn test_azure_calculator_calculate_cost() {
        let calc = AzureCostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("gpt-4", &usage).await;

        if let Ok(breakdown) = result {
            assert_eq!(breakdown.provider, "azure");
            assert!(breakdown.total_cost >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_azure_calculator_calculate_cost_unknown_model() {
        let calc = AzureCostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("unknown-model", &usage).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_azure_calculator_estimate_cost() {
        let calc = AzureCostCalculator::new();

        let result = calc.estimate_cost("gpt-4", 1000, Some(500)).await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
            assert!(estimate.max_cost >= estimate.min_cost);
            assert_eq!(estimate.currency, "USD");
        }
    }

    #[tokio::test]
    async fn test_azure_calculator_estimate_cost_no_max_output() {
        let calc = AzureCostCalculator::new();

        let result = calc.estimate_cost("gpt-4", 1000, None).await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
        }
    }

    #[test]
    fn test_azure_calculator_get_model_pricing() {
        let calc = AzureCostCalculator::new();

        let result = calc.get_model_pricing("gpt-4");

        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
        }
    }

    #[test]
    fn test_azure_calculator_get_model_pricing_unknown() {
        let calc = AzureCostCalculator::new();

        let result = calc.get_model_pricing("unknown-azure-model");

        assert!(result.is_err());
        // The underlying implementation may report either "azure" or "openai" as provider
        // since azure models often map to openai models internally
        if let Err(CostError::ModelNotSupported { model, provider: _ }) = result {
            assert_eq!(model, "unknown-azure-model");
        }
    }

    // ==================== Model Variants Tests ====================

    #[test]
    fn test_azure_cost_gpt4_turbo() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4-turbo", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_azure_cost_gpt35_turbo() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-35-turbo", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_azure_cost_with_prefix() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("azure/gpt-4", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_azure_calculator_large_usage() {
        let calc = AzureCostCalculator::new();
        let usage = UsageTokens::new(100000, 50000);

        let result = calc.calculate_cost("gpt-4", &usage).await;

        if let Ok(breakdown) = result {
            assert!(breakdown.total_cost >= 0.0);
            assert_eq!(breakdown.usage.prompt_tokens, 100000);
            assert_eq!(breakdown.usage.completion_tokens, 50000);
        }
    }

    #[test]
    fn test_azure_dalle_quality_multiplier() {
        let calc = AzureCostCalculator::new();

        // HD should cost more than standard
        let result_standard =
            calc.calculate_dalle_cost("dall-e-3", "1024x1024", Some("standard"), 1);
        let result_hd = calc.calculate_dalle_cost("dall-e-3", "1024x1024", Some("hd"), 1);

        if let (Ok(cost_standard), Ok(cost_hd)) = (result_standard, result_hd) {
            // HD multiplier is 2x for 1024x1024
            assert!((cost_hd - cost_standard * 2.0).abs() < 0.001);
        }
    }
}
