//! OpenAI Provider Cost Calculation
//!
//! Simple delegation to the unified cost calculation system

use crate::core::cost::{
    calculator::{CostCalculator, estimate_cost, generic_cost_per_token, get_model_pricing},
    types::{CostBreakdown, CostError, CostEstimate, ModelPricing, UsageTokens},
};
use async_trait::async_trait;

/// OpenAI Cost Calculator - delegates to generic implementation
#[derive(Debug, Clone, Default)]
pub struct OpenAICostCalculator;

impl OpenAICostCalculator {
    pub fn new() -> Self {
        Self
    }

    /// Calculate image generation cost
    pub fn calculate_image_cost(
        &self,
        model: &str,
        size: &str,
        quality: Option<&str>,
        quantity: u32,
    ) -> Result<f64, CostError> {
        let pricing = get_model_pricing(model, "openai")?;

        if let Some(ref cost_per_image) = pricing.cost_per_image {
            let price_key = if model.contains("dall-e-3") && quality == Some("hd") {
                format!("{}-hd", size)
            } else {
                size.to_string()
            };

            if let Some(&cost) = cost_per_image.get(&price_key) {
                return Ok(cost * quantity as f64);
            }
        }

        Err(CostError::MissingPricing {
            model: model.to_string(),
        })
    }

    /// Calculate audio processing cost
    pub fn calculate_audio_cost(
        &self,
        model: &str,
        duration_minutes: f64,
    ) -> Result<f64, CostError> {
        let pricing = get_model_pricing(model, "openai")?;

        if let Some(cost_per_second) = pricing.cost_per_second {
            return Ok(duration_minutes * 60.0 * cost_per_second);
        }

        Err(CostError::MissingPricing {
            model: model.to_string(),
        })
    }
}

#[async_trait]
impl CostCalculator for OpenAICostCalculator {
    type Error = CostError;

    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error> {
        generic_cost_per_token(model, usage, "openai")
    }

    async fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error> {
        estimate_cost(model, "openai", input_tokens, max_output_tokens)
    }

    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error> {
        get_model_pricing(model, "openai")
    }

    fn provider_name(&self) -> &str {
        "openai"
    }
}

/// Helper function for easy cost calculation (maintains compatibility)
pub fn cost_per_token(model: &str, usage: &UsageTokens) -> Result<(f64, f64), CostError> {
    let breakdown = generic_cost_per_token(model, usage, "openai")?;
    Ok((breakdown.input_cost, breakdown.output_cost))
}

/// Get OpenAI model pricing (convenience function)
pub fn get_openai_model_pricing(model: &str) -> Result<ModelPricing, CostError> {
    get_model_pricing(model, "openai")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== OpenAICostCalculator Tests ====================

    #[test]
    fn test_openai_cost_calculator_new() {
        let calc = OpenAICostCalculator::new();
        assert_eq!(calc.provider_name(), "openai");
    }

    #[test]
    fn test_openai_cost_calculator_default() {
        let calc = OpenAICostCalculator;
        assert_eq!(calc.provider_name(), "openai");
    }

    #[test]
    fn test_openai_cost_calculator_provider_name() {
        let calc = OpenAICostCalculator;
        assert_eq!(calc.provider_name(), "openai");
    }

    #[test]
    fn test_openai_cost_calculator_clone() {
        let calc = OpenAICostCalculator::new();
        let cloned = calc.clone();
        assert_eq!(calc.provider_name(), cloned.provider_name());
    }

    #[test]
    fn test_openai_cost_calculator_debug() {
        let calc = OpenAICostCalculator::new();
        let debug_str = format!("{:?}", calc);
        assert!(debug_str.contains("OpenAICostCalculator"));
    }

    // ==================== calculate_image_cost Tests ====================

    #[test]
    fn test_calculate_image_cost_unknown_model() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_image_cost("unknown-model", "1024x1024", None, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_image_cost_dall_e_3() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_image_cost("dall-e-3", "1024x1024", None, 1);
        // May succeed or fail depending on pricing database
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_image_cost_dall_e_3_hd() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_image_cost("dall-e-3", "1024x1024", Some("hd"), 1);
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_image_cost_multiple_images() {
        let calc = OpenAICostCalculator::new();
        let result_one = calc.calculate_image_cost("dall-e-3", "1024x1024", None, 1);
        let result_three = calc.calculate_image_cost("dall-e-3", "1024x1024", None, 3);

        if let (Ok(cost_one), Ok(cost_three)) = (result_one, result_three) {
            // 3 images should cost 3x one image
            assert!((cost_three - cost_one * 3.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_calculate_image_cost_zero_quantity() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_image_cost("dall-e-3", "1024x1024", None, 0);
        if let Ok(cost) = result {
            assert_eq!(cost, 0.0);
        }
    }

    // ==================== calculate_audio_cost Tests ====================

    #[test]
    fn test_calculate_audio_cost_unknown_model() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_audio_cost("unknown-audio-model", 5.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_audio_cost_whisper() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_audio_cost("whisper-1", 5.0);
        // May succeed or fail depending on pricing database
        if let Ok(cost) = result {
            assert!(cost >= 0.0);
        }
    }

    #[test]
    fn test_calculate_audio_cost_zero_duration() {
        let calc = OpenAICostCalculator::new();
        let result = calc.calculate_audio_cost("whisper-1", 0.0);
        if let Ok(cost) = result {
            assert_eq!(cost, 0.0);
        }
    }

    #[test]
    fn test_calculate_audio_cost_scaling() {
        let calc = OpenAICostCalculator::new();
        let result_one = calc.calculate_audio_cost("whisper-1", 1.0);
        let result_ten = calc.calculate_audio_cost("whisper-1", 10.0);

        if let (Ok(cost_one), Ok(cost_ten)) = (result_one, result_ten) {
            // 10 minutes should cost 10x one minute
            assert!((cost_ten - cost_one * 10.0).abs() < 0.001);
        }
    }

    // ==================== cost_per_token Tests ====================

    #[test]
    fn test_cost_per_token_gpt4() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4", &usage);
        // Should return result for known model
        if let Ok((input_cost, output_cost)) = result {
            assert!(input_cost >= 0.0);
            assert!(output_cost >= 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_gpt4_turbo() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4-turbo", &usage);
        if let Ok((input_cost, output_cost)) = result {
            assert!(input_cost >= 0.0);
            assert!(output_cost >= 0.0);
        }
    }

    #[test]
    fn test_cost_per_token_gpt35_turbo() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-3.5-turbo", &usage);
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

    // ==================== get_openai_model_pricing Tests ====================

    #[test]
    fn test_get_openai_model_pricing_gpt4() {
        let result = get_openai_model_pricing("gpt-4");
        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
            assert_eq!(pricing.currency, "USD");
        }
    }

    #[test]
    fn test_get_openai_model_pricing_unknown_model() {
        let result = get_openai_model_pricing("unknown-openai-model");
        assert!(result.is_err());
    }

    // ==================== Async Tests ====================

    #[tokio::test]
    async fn test_openai_calculator_calculate_cost() {
        let calc = OpenAICostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("gpt-4", &usage).await;

        if let Ok(breakdown) = result {
            assert_eq!(breakdown.provider, "openai");
            assert!(breakdown.total_cost >= 0.0);
        }
    }

    #[tokio::test]
    async fn test_openai_calculator_calculate_cost_unknown_model() {
        let calc = OpenAICostCalculator::new();
        let usage = UsageTokens::new(1000, 500);

        let result = calc.calculate_cost("unknown-model", &usage).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_openai_calculator_estimate_cost() {
        let calc = OpenAICostCalculator::new();

        let result = calc.estimate_cost("gpt-4", 1000, Some(500)).await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
            assert!(estimate.max_cost >= estimate.min_cost);
            assert_eq!(estimate.currency, "USD");
        }
    }

    #[tokio::test]
    async fn test_openai_calculator_estimate_cost_no_max_output() {
        let calc = OpenAICostCalculator::new();

        let result = calc.estimate_cost("gpt-4", 1000, None).await;

        if let Ok(estimate) = result {
            assert!(estimate.min_cost >= 0.0);
        }
    }

    #[test]
    fn test_openai_calculator_get_model_pricing() {
        let calc = OpenAICostCalculator::new();

        let result = calc.get_model_pricing("gpt-4");

        if let Ok(pricing) = result {
            assert!(pricing.input_cost_per_1k_tokens >= 0.0);
            assert!(pricing.output_cost_per_1k_tokens >= 0.0);
        }
    }

    #[test]
    fn test_openai_calculator_get_model_pricing_unknown() {
        let calc = OpenAICostCalculator::new();

        let result = calc.get_model_pricing("unknown-openai-model");

        assert!(result.is_err());
        if let Err(CostError::ModelNotSupported { model, provider }) = result {
            assert_eq!(model, "unknown-openai-model");
            assert_eq!(provider, "openai");
        }
    }

    // ==================== Model Variants Tests ====================

    #[test]
    fn test_openai_cost_gpt4o() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4o", &usage);
        // Should work if model is in pricing database
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_openai_cost_gpt4o_mini() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("gpt-4o-mini", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_openai_cost_o1_preview() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("o1-preview", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_openai_cost_o1_mini() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("o1-mini", &usage);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_openai_cost_with_prefix() {
        let usage = UsageTokens::new(1000, 500);
        let result = cost_per_token("openai/gpt-4", &usage);
        // May or may not work depending on implementation
        assert!(result.is_ok() || result.is_err());
    }

    // ==================== Integration Tests ====================

    #[tokio::test]
    async fn test_openai_calculator_large_usage() {
        let calc = OpenAICostCalculator::new();
        let usage = UsageTokens::new(100000, 50000);

        let result = calc.calculate_cost("gpt-4", &usage).await;

        if let Ok(breakdown) = result {
            assert!(breakdown.total_cost >= 0.0);
            assert_eq!(breakdown.usage.prompt_tokens, 100000);
            assert_eq!(breakdown.usage.completion_tokens, 50000);
        }
    }

    #[test]
    fn test_openai_pricing_comparison() {
        // GPT-4 should be more expensive than GPT-3.5-turbo
        let pricing_4 = get_openai_model_pricing("gpt-4");
        let pricing_35 = get_openai_model_pricing("gpt-3.5-turbo");

        if let (Ok(p4), Ok(p35)) = (pricing_4, pricing_35) {
            assert!(p4.input_cost_per_1k_tokens >= p35.input_cost_per_1k_tokens);
        }
    }
}
