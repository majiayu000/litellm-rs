//! Public completion-cost entry points.

use super::service::{PricingService, require_pricing_field, require_total_time_seconds};
use super::types::{CostResult, CostType, LiteLLMModelInfo, PricingUsage};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{DateTime, Utc};
use tracing::warn;

impl PricingService {
    /// Calculate completion cost using the prices effective at call start.
    pub async fn calculate_completion_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
        total_time_seconds: Option<f64>,
    ) -> Result<CostResult> {
        self.calculate_completion_cost_at(
            model,
            input_tokens,
            output_tokens,
            prompt,
            completion,
            total_time_seconds,
            Utc::now(),
        )
        .await
    }

    /// Calculate completion cost at a specific UTC pricing instant.
    #[allow(clippy::too_many_arguments)]
    pub async fn calculate_completion_cost_at(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
        total_time_seconds: Option<f64>,
        pricing_time: DateTime<Utc>,
    ) -> Result<CostResult> {
        if self.needs_refresh()
            && let Err(error) = self.refresh_pricing_data().await
        {
            warn!("Failed to refresh pricing data: {error}");
        }

        let model_info = self
            .get_raw_model_info(model)
            .ok_or_else(|| GatewayError::not_found(format!("Model not found: {model}")))?;
        let model_info = super::google::effective_model_info_at(
            &model_info.litellm_provider,
            model,
            &model_info,
            pricing_time,
        )
        .into_owned();

        if model_info.cost_per_second.is_some() {
            let total_time_seconds = require_total_time_seconds(model, total_time_seconds)?;
            return self.calculate_time_based_cost(model, &model_info, total_time_seconds);
        }

        match model_info.litellm_provider.as_str() {
            "google" | "vertex_ai" => self.calculate_google_cost(
                model,
                &model_info,
                input_tokens,
                output_tokens,
                prompt,
                completion,
            ),
            _ => {
                let usage = PricingUsage::new(input_tokens, output_tokens);
                let breakdown = super::usage_cost::calculate_usage_cost_with_pricing_at(
                    &model_info.litellm_provider,
                    model,
                    &model_info,
                    &usage,
                    pricing_time,
                )?;
                Ok(CostResult {
                    input_cost: breakdown.input_cost,
                    output_cost: breakdown.output_cost,
                    total_cost: breakdown.total_cost,
                    input_tokens,
                    output_tokens,
                    model: model.to_string(),
                    provider: model_info.litellm_provider,
                    cost_type: CostType::TokenBased,
                })
            }
        }
    }

    /// Calculate scalar token-based cost for compatibility and Google fallback.
    pub(super) fn calculate_token_based_cost(
        &self,
        model: &str,
        model_info: &LiteLLMModelInfo,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<CostResult> {
        let input_cost_per_token = require_pricing_field(
            model_info.input_cost_per_token,
            model,
            "token pricing",
            "input_cost_per_token",
        )?;
        let output_cost_per_token = require_pricing_field(
            model_info.output_cost_per_token,
            model,
            "token pricing",
            "output_cost_per_token",
        )?;

        let input_cost = input_tokens as f64 * input_cost_per_token;
        let output_cost = output_tokens as f64 * output_cost_per_token;
        Ok(CostResult {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            input_tokens,
            output_tokens,
            model: model.to_string(),
            provider: model_info.litellm_provider.clone(),
            cost_type: CostType::TokenBased,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[tokio::test]
    async fn public_completion_cost_applies_time_of_use_pricing() {
        let service = PricingService::with_embedded_default().unwrap();
        let off_peak = Utc.with_ymd_and_hms(2026, 8, 24, 4, 0, 0).unwrap();
        let peak = Utc.with_ymd_and_hms(2026, 8, 24, 6, 0, 0).unwrap();
        let off_peak_cost = service
            .calculate_completion_cost_at(
                "deepseek-v4-flash",
                1_000,
                1_000,
                None,
                None,
                None,
                off_peak,
            )
            .await
            .unwrap();
        let peak_cost = service
            .calculate_completion_cost_at("deepseek-v4-flash", 1_000, 1_000, None, None, None, peak)
            .await
            .unwrap();

        assert!((peak_cost.total_cost - off_peak_cost.total_cost * 2.0).abs() < 1e-12);
    }
}
