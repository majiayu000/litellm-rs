//! Cost Calculation for Bedrock Models
//!
//! Provides accurate pricing information and cost calculation
//! for all supported Bedrock models.

use std::collections::HashMap;
use std::sync::LazyLock;

pub use crate::core::cost::types::ModelPricing;

fn currency(pricing: &ModelPricing) -> &'static str {
    if pricing.currency == "USD" {
        "USD"
    } else {
        "UNKNOWN"
    }
}

/// Comprehensive pricing database for all Bedrock models
static MODEL_PRICING: LazyLock<HashMap<&'static str, ModelPricing>> = LazyLock::new(|| {
    super::super::catalog::all_entries()
        .iter()
        .filter_map(|entry| {
            entry
                .to_model_pricing()
                .map(|pricing| (entry.model_id, pricing))
        })
        .collect()
});

/// Cost calculator for Bedrock models
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate cost for a specific model and token usage
    pub fn calculate_cost(model_id: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
        MODEL_PRICING.get(model_id).map(|pricing| {
            let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
            let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;
            input_cost + output_cost
        })
    }

    /// Get pricing information for a model
    pub fn get_model_pricing(model_id: &str) -> Option<&'static ModelPricing> {
        MODEL_PRICING.get(model_id)
    }

    /// Get pricing information in the shared core cost model shape.
    pub fn get_core_model_pricing(model_id: &str) -> Option<ModelPricing> {
        MODEL_PRICING.get(model_id).cloned()
    }

    /// Get all available models with pricing
    pub fn get_all_models() -> Vec<&'static str> {
        MODEL_PRICING.keys().copied().collect()
    }

    /// Calculate cost with breakdown
    pub fn calculate_detailed_cost(
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Option<CostBreakdown> {
        MODEL_PRICING.get(model_id).map(|pricing| {
            let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
            let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

            CostBreakdown {
                input_tokens,
                output_tokens,
                input_cost,
                output_cost,
                total_cost: input_cost + output_cost,
                currency: currency(pricing),
            }
        })
    }
}

/// Detailed cost breakdown
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: &'static str,
}

#[cfg(test)]
#[path = "cost_tests.rs"]
mod tests;
