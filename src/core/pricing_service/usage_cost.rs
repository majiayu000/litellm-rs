//! Pure token and modality usage-cost calculation for loaded catalog rows.

use super::types::{CostType, LiteLLMModelInfo, PricingCostBreakdown, PricingUsage};
use crate::utils::error::gateway_error::{GatewayError, Result};
use chrono::{DateTime, Utc};

pub(super) fn calculate_usage_cost_with_pricing_at(
    requested_provider: &str,
    model: &str,
    model_info: &LiteLLMModelInfo,
    usage: &PricingUsage,
    pricing_time: DateTime<Utc>,
) -> Result<PricingCostBreakdown> {
    let peak_rates =
        crate::core::pricing::time_of_use::peak_token_rates_at(model_info, pricing_time).map_err(
            |message| {
                GatewayError::Config(format!(
                    "Invalid time-of-use pricing for model {model}: {message}"
                ))
            },
        )?;
    calculate_usage_cost_with_rates(requested_provider, model, model_info, usage, peak_rates)
}

pub(super) fn calculate_usage_cost_with_maximum_rates(
    requested_provider: &str,
    model: &str,
    model_info: &LiteLLMModelInfo,
    usage: &PricingUsage,
) -> Result<PricingCostBreakdown> {
    let peak_rates = crate::core::pricing::time_of_use::configured_peak_token_rates(model_info)
        .map_err(|message| {
            GatewayError::Config(format!(
                "Invalid time-of-use pricing for model {model}: {message}"
            ))
        })?;
    calculate_usage_cost_with_rates(requested_provider, model, model_info, usage, peak_rates)
}

fn calculate_usage_cost_with_rates(
    requested_provider: &str,
    model: &str,
    model_info: &LiteLLMModelInfo,
    usage: &PricingUsage,
    peak_rates: Option<crate::core::pricing::time_of_use::TokenRates>,
) -> Result<PricingCostBreakdown> {
    let (mut input_cost_per_token, mut output_cost_per_token) =
        super::image_pricing::token_unit_prices(model, model_info, usage)?;
    if let Some(rates) = peak_rates {
        input_cost_per_token = rates.input_cost_per_token;
        output_cost_per_token = rates.output_cost_per_token;
    }

    let input_cost_per_token = tiered_cost_per_token(
        model_info,
        input_cost_per_token,
        "input_cost_per_token_above_",
        usage.prompt_tokens,
    );
    let output_cost_per_token = tiered_cost_per_token(
        model_info,
        output_cost_per_token,
        "output_cost_per_token_above_",
        usage.prompt_tokens,
    );
    let base_cache_read_cost = peak_rates
        .map(|rates| rates.cache_read_input_token_cost)
        .unwrap_or_else(|| {
            model_info
                .extra
                .get("cache_read_input_token_cost")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(input_cost_per_token)
        });
    let cache_read_cost_per_token = tiered_cost_per_token(
        model_info,
        base_cache_read_cost,
        "cache_read_input_token_cost_above_",
        usage.prompt_tokens,
    );
    let cache_creation_cost_per_token = tiered_cost_per_token(
        model_info,
        model_info
            .extra
            .get("cache_creation_input_token_cost")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(input_cost_per_token),
        "cache_creation_input_token_cost_above_",
        usage.prompt_tokens,
    );
    let cache_creation_tokens = usage.cache_creation_token_count();
    let cache_read_tokens = usage.cache_read_token_count();
    let non_cached_tokens = usage.non_cached_prompt_tokens();
    let input_cost = non_cached_tokens as f64 * input_cost_per_token;
    let output_cost = usage.completion_tokens as f64 * output_cost_per_token;
    let cache_cost = cache_creation_tokens as f64 * cache_creation_cost_per_token
        + cache_read_tokens as f64 * cache_read_cost_per_token;
    let audio_cost = priced_extra_units(
        model_info,
        model,
        usage.audio_tokens,
        &["input_cost_per_audio_token"],
        "audio pricing",
    )? + priced_extra_units(
        model_info,
        model,
        usage.output_audio_tokens,
        &["output_cost_per_audio_token"],
        "output audio pricing",
    )?;
    let image_cost_per_token =
        super::image_pricing::image_token_unit_price(model_info, usage).unwrap_or(0.0);
    let image_cost = usage.image_tokens.unwrap_or(0) as f64 * image_cost_per_token
        + super::image_pricing::output_image_cost(model, model_info, usage)?;
    let reasoning_cost = usage.reasoning_tokens.unwrap_or(0) as f64
        * extra_f64(model_info, "output_cost_per_reasoning_token");
    let total_cost =
        input_cost + output_cost + cache_cost + audio_cost + image_cost + reasoning_cost;

    Ok(PricingCostBreakdown {
        total_cost,
        input_cost,
        output_cost,
        cache_cost,
        audio_cost,
        image_cost,
        reasoning_cost,
        usage: usage.clone(),
        currency: "USD".to_string(),
        model: model.to_string(),
        provider: requested_provider.to_string(),
        cost_type: CostType::TokenBased,
    })
}

fn extra_f64(pricing: &LiteLLMModelInfo, key: &str) -> f64 {
    pricing
        .extra
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

fn priced_extra_units(
    pricing: &LiteLLMModelInfo,
    model: &str,
    units: Option<u32>,
    keys: &[&str],
    pricing_type: &str,
) -> Result<f64> {
    let units = units.unwrap_or(0);
    if units == 0 {
        return Ok(0.0);
    }

    let (key, unit_price) = keys
        .iter()
        .find_map(|key| {
            pricing
                .extra
                .get(*key)
                .and_then(serde_json::Value::as_f64)
                .map(|price| (*key, price))
        })
        .ok_or_else(|| {
            GatewayError::Config(format!(
                "Missing {pricing_type} for model {model}: {}",
                keys.join(", ")
            ))
        })?;
    if unit_price < 0.0 || unit_price.is_nan() {
        return Err(GatewayError::Config(format!(
            "Invalid {pricing_type} for model {model}: {key} ({unit_price})"
        )));
    }

    Ok(units as f64 * unit_price)
}

fn tiered_cost_per_token(
    pricing: &LiteLLMModelInfo,
    base_cost: f64,
    key_prefix: &str,
    prompt_tokens: u32,
) -> f64 {
    pricing
        .extra
        .iter()
        .filter_map(|(key, value)| {
            if !key.starts_with(key_prefix) {
                return None;
            }
            let threshold = extract_tier_threshold(key)?;
            if prompt_tokens > threshold {
                value.as_f64().map(|cost| (threshold, cost))
            } else {
                None
            }
        })
        .max_by_key(|(threshold, _)| *threshold)
        .map(|(_, cost)| cost)
        .unwrap_or(base_cost)
}

pub(super) fn extract_tier_threshold(key: &str) -> Option<u32> {
    let threshold = key.split("_above_").nth(1)?.strip_suffix("_tokens")?;
    if let Some(number) = threshold.strip_suffix('k') {
        number.parse::<u32>().ok().map(|value| value * 1000)
    } else {
        threshold.parse::<u32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn authority_applies_deepseek_peak_rates_to_text_and_cache_usage() {
        let service = super::super::PricingService::with_embedded_default().unwrap();
        let mut usage = PricingUsage::new(1_000, 1_000);
        usage.cached_tokens = Some(250);
        let off_peak = Utc.with_ymd_and_hms(2026, 8, 24, 4, 0, 0).unwrap();
        let peak = Utc.with_ymd_and_hms(2026, 8, 24, 6, 0, 0).unwrap();

        for model in [
            "deepseek-chat",
            "deepseek-reasoner",
            "deepseek-v4-flash",
            "deepseek-v4-flash-vision-exp",
            "deepseek-v4-pro",
            "deepseek/deepseek-chat",
            "deepseek/deepseek-reasoner",
            "deepseek/deepseek-v4-flash",
            "deepseek/deepseek-v4-flash-vision-exp",
            "deepseek/deepseek-v4-pro",
        ] {
            let off_peak_cost = service
                .calculate_loaded_usage_cost_for_provider_at("deepseek", model, &usage, off_peak)
                .unwrap();
            let peak_cost = service
                .calculate_loaded_usage_cost_for_provider_at("deepseek", model, &usage, peak)
                .unwrap();
            for (off_peak_component, peak_component) in [
                (off_peak_cost.input_cost, peak_cost.input_cost),
                (off_peak_cost.output_cost, peak_cost.output_cost),
                (off_peak_cost.cache_cost, peak_cost.cache_cost),
                (off_peak_cost.total_cost, peak_cost.total_cost),
            ] {
                assert!(
                    (peak_component - off_peak_component * 2.0).abs() < 1e-12,
                    "{model} peak component {peak_component} should be twice {off_peak_component}"
                );
            }
        }
    }

    #[test]
    fn authority_rejects_malformed_declared_time_of_use_pricing() {
        let service = super::super::PricingService::with_embedded_default().unwrap();
        let (_, mut info) = service
            .get_model_info_for_provider("deepseek", "deepseek-v4-flash")
            .unwrap();
        info.extra.insert(
            crate::core::pricing::time_of_use::TIME_OF_USE_PRICING_KEY.to_string(),
            serde_json::json!({"timezone": "UTC"}),
        );
        let at = Utc.with_ymd_and_hms(2026, 8, 24, 2, 0, 0).unwrap();
        let error = calculate_usage_cost_with_pricing_at(
            "deepseek",
            "deepseek-v4-flash",
            &info,
            &PricingUsage::new(1_000, 1_000),
            at,
        )
        .unwrap_err();
        assert!(error.to_string().contains("Invalid time-of-use pricing"));
    }

    #[test]
    fn reservation_estimate_covers_a_request_that_crosses_into_peak() {
        let service = super::super::PricingService::with_embedded_default().unwrap();
        let usage = PricingUsage::new(1_000, 1_000);
        let off_peak = Utc.with_ymd_and_hms(2026, 8, 24, 0, 59, 0).unwrap();
        let peak = Utc.with_ymd_and_hms(2026, 8, 24, 1, 0, 0).unwrap();
        let estimate = service
            .estimate_loaded_completion_cost_for_provider(
                "deepseek",
                "deepseek-v4-flash",
                1_000,
                Some(1_000),
            )
            .unwrap();
        let off_peak_cost = service
            .calculate_loaded_usage_cost_for_provider_at(
                "deepseek",
                "deepseek-v4-flash",
                &usage,
                off_peak,
            )
            .unwrap();
        let peak_cost = service
            .calculate_loaded_usage_cost_for_provider_at(
                "deepseek",
                "deepseek-v4-flash",
                &usage,
                peak,
            )
            .unwrap();

        assert!((estimate.max_cost - peak_cost.total_cost).abs() < 1e-12);
        assert!(off_peak_cost.total_cost <= estimate.max_cost);
        assert!(peak_cost.total_cost <= estimate.max_cost);
    }
}
