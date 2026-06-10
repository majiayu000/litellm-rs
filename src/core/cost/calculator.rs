//! Unified Cost Calculator
//!
//! Core cost calculation logic that all providers delegate to.
//! This eliminates code duplication and ensures consistent behavior.

use async_trait::async_trait;

use crate::core::cost::types::{
    CostBreakdown, CostError, CostEstimate, ModelCostComparison, ModelPricing, UsageTokens,
};
use crate::core::cost::utils::select_tiered_pricing;

mod pricing;

use self::pricing::{
    get_anthropic_pricing, get_azure_pricing, get_deepseek_pricing, get_minimax_pricing,
    get_moonshot_pricing, get_openai_pricing, get_vertex_ai_pricing, get_zhipu_pricing,
};

/// Unified Cost Calculator Trait
///
/// All providers should implement this trait by delegating to the generic functions
#[async_trait]
pub trait CostCalculator {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Calculate cost for a completed request
    async fn calculate_cost(
        &self,
        model: &str,
        usage: &UsageTokens,
    ) -> Result<CostBreakdown, Self::Error>;

    /// Estimate cost before making a request
    async fn estimate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<CostEstimate, Self::Error>;

    /// Get pricing information for a model
    fn get_model_pricing(&self, model: &str) -> Result<ModelPricing, Self::Error>;

    /// Get provider name
    fn provider_name(&self) -> &str;
}

/// Generic cost calculation function (like Python's generic_cost_per_token)
///
/// This is the core cost calculation logic that all providers delegate to
pub fn generic_cost_per_token(
    model: &str,
    usage: &UsageTokens,
    provider: &str,
) -> Result<CostBreakdown, CostError> {
    // Get model pricing information
    let pricing = get_model_pricing(model, provider)?;

    // Initialize cost breakdown
    let mut breakdown = CostBreakdown::new(model.to_string(), provider.to_string(), usage.clone());

    // Calculate tiered pricing if applicable
    let (input_cost_per_1k, output_cost_per_1k, cache_creation_cost_per_1k, cache_read_cost_per_1k) =
        select_tiered_pricing(&pricing, usage);

    // Calculate input cost
    breakdown.input_cost = calculate_input_cost(usage, input_cost_per_1k);

    // Calculate output cost
    breakdown.output_cost = calculate_output_cost(usage, output_cost_per_1k);

    // Calculate cache costs if applicable
    if let Some(cached_tokens) = usage.cached_tokens {
        breakdown.cache_cost = calculate_cache_cost(
            cached_tokens,
            cache_creation_cost_per_1k,
            cache_read_cost_per_1k,
        );
    }

    // Calculate audio costs if applicable
    if let Some(audio_tokens) = usage.audio_tokens {
        breakdown.audio_cost = calculate_audio_cost(&pricing, audio_tokens);
    }

    // Calculate image costs if applicable
    if let Some(image_tokens) = usage.image_tokens {
        breakdown.image_cost = calculate_image_cost(&pricing, image_tokens);
    }

    // Calculate reasoning tokens cost if applicable (for o1 models)
    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        breakdown.reasoning_cost = calculate_reasoning_cost(&pricing, reasoning_tokens);
    }

    // Calculate total
    breakdown.calculate_total();

    Ok(breakdown)
}

/// Get model pricing information
pub fn get_model_pricing(model: &str, provider: &str) -> Result<ModelPricing, CostError> {
    match provider.to_lowercase().as_str() {
        "openai" => get_pricing_with_shared_source(model, &["openai"], get_openai_pricing),
        "anthropic" => get_pricing_with_shared_source(model, &["anthropic"], get_anthropic_pricing),
        "azure" => get_azure_pricing(model),
        "vertex_ai" | "vertexai" => {
            get_pricing_with_shared_source(model, &["vertex_ai", "google"], get_vertex_ai_pricing)
        }
        "deepseek" => get_pricing_with_shared_source(model, &["deepseek"], get_deepseek_pricing),
        "moonshot" => get_pricing_with_shared_source(model, &["moonshot"], get_moonshot_pricing),
        "minimax" => get_pricing_with_shared_source(model, &["minimax"], get_minimax_pricing),
        "zhipu" | "zhipuai" | "glm" | "zai" => {
            get_pricing_with_shared_source(model, &["zhipuai", "glm", "zai"], get_zhipu_pricing)
        }
        _ => Err(CostError::ProviderNotSupported {
            provider: provider.to_string(),
        }),
    }
}

fn get_pricing_with_shared_source<F>(
    model: &str,
    provider_aliases: &[&str],
    fallback: F,
) -> Result<ModelPricing, CostError>
where
    F: FnOnce(&str) -> Result<ModelPricing, CostError>,
{
    // Some(..) means the shared catalog matched this model; an inner Err means
    // it matched but carried no usable pricing — that must not fall through to
    // hardcoded defaults, or an unpriced catalog entry would bill at $0.
    if let Some(pricing) = get_shared_model_pricing(model, provider_aliases) {
        return pricing;
    }

    fallback(model)
}

fn get_shared_model_pricing(
    model: &str,
    provider_aliases: &[&str],
) -> Option<Result<ModelPricing, CostError>> {
    let db = crate::core::pricing::get_pricing_db();

    if let Some(info) = db.get_model_info(model)
        && litellm_provider_matches(&info.litellm_provider, provider_aliases)
    {
        return Some(litellm_to_cost_pricing(model, info));
    }

    let model_lower = model.to_lowercase();
    for provider in provider_aliases {
        let mut candidates = db.get_provider_models(provider);
        candidates.sort();

        for model_id in candidates {
            let model_id_lower = model_id.to_lowercase();
            let matches = model_id_lower == model_lower || model_id_lower.contains(&model_lower);
            if matches
                && let Some(info) = db.get_model_info(&model_id)
                && litellm_provider_matches(&info.litellm_provider, provider_aliases)
            {
                return Some(litellm_to_cost_pricing(&model_id, info));
            }
        }
    }

    None
}

fn litellm_provider_matches(provider: &str, aliases: &[&str]) -> bool {
    let provider = normalize_pricing_provider(provider);
    aliases
        .iter()
        .any(|alias| normalize_pricing_provider(alias) == provider)
}

fn normalize_pricing_provider(provider: &str) -> String {
    match provider.to_lowercase().replace('-', "_").as_str() {
        "vertexai" | "google" => "vertex_ai".to_string(),
        "zhipu" | "glm" | "zai" => "zhipuai".to_string(),
        other => other.to_string(),
    }
}

fn litellm_to_cost_pricing(
    model: &str,
    info: &crate::core::pricing::LiteLLMModelInfo,
) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    // A catalog entry with neither token cost is unpriced data, not a free
    // model: charging $0 would silently under-bill, so surface it instead.
    if info.input_cost_per_token.is_none() && info.output_cost_per_token.is_none() {
        return Err(CostError::MissingPricing {
            model: model.to_string(),
        });
    }
    // Chat/completion requests consume both prompt and completion tokens; a
    // single missing side would under-bill real completions, so fail closed.
    if requires_bidirectional_token_pricing(info)
        && (info.input_cost_per_token.is_none() || info.output_cost_per_token.is_none())
    {
        return Err(CostError::MissingPricing {
            model: model.to_string(),
        });
    }
    // Non-chat modes such as embeddings may only price one token direction.
    // Keep allowing that shape, but flag the gap so catalog data can be fixed.
    if info.input_cost_per_token.is_none() || info.output_cost_per_token.is_none() {
        tracing::warn!(
            "model '{}' is missing {} token cost; billing that side at $0",
            model,
            if info.input_cost_per_token.is_none() {
                "input"
            } else {
                "output"
            }
        );
    }

    Ok(ModelPricing {
        model: model.to_string(),
        input_cost_per_1k_tokens: price_per_token_to_per_1k(
            info.input_cost_per_token.unwrap_or(0.0),
        ),
        output_cost_per_1k_tokens: price_per_token_to_per_1k(
            info.output_cost_per_token.unwrap_or(0.0),
        ),
        cache_read_input_token_cost: extra_token_cost_per_1k(info, "cache_read_input_token_cost"),
        cache_creation_input_token_cost: extra_token_cost_per_1k(
            info,
            "cache_creation_input_token_cost",
        ),
        input_cost_per_audio_token: extra_f64(info, "input_cost_per_audio_token"),
        output_cost_per_audio_token: extra_f64(info, "output_cost_per_audio_token"),
        image_cost_per_token: extra_f64(info, "image_cost_per_token"),
        reasoning_cost_per_token: extra_f64(info, "output_cost_per_reasoning_token"),
        cost_per_second: info.cost_per_second,
        video_cost_per_second: extra_f64(info, "video_cost_per_second"),
        audio_cost_per_second: extra_f64(info, "audio_cost_per_second"),
        cost_per_image: None,
        tiered_pricing: extra_tiered_pricing_per_1k(info),
        batch_discount: extra_f64(info, "batch_discount"),
        currency: "USD".to_string(),
        updated_at: Utc::now(),
    })
}

fn requires_bidirectional_token_pricing(info: &crate::core::pricing::LiteLLMModelInfo) -> bool {
    matches!(info.mode.as_str(), "chat" | "completion")
}

fn extra_f64(info: &crate::core::pricing::LiteLLMModelInfo, key: &str) -> Option<f64> {
    info.extra.get(key).and_then(serde_json::Value::as_f64)
}

fn extra_token_cost_per_1k(
    info: &crate::core::pricing::LiteLLMModelInfo,
    key: &str,
) -> Option<f64> {
    extra_f64(info, key).map(price_per_token_to_per_1k)
}

fn extra_tiered_pricing_per_1k(
    info: &crate::core::pricing::LiteLLMModelInfo,
) -> Option<std::collections::HashMap<String, f64>> {
    let tiered = info
        .extra
        .iter()
        .filter_map(|(key, value)| {
            let is_token_tier = key.starts_with("input_cost_per_token_above_")
                || key.starts_with("output_cost_per_token_above_")
                || key.starts_with("cache_creation_input_token_cost_above_")
                || key.starts_with("cache_read_input_token_cost_above_");

            if is_token_tier {
                value
                    .as_f64()
                    .map(|cost_per_token| (key.clone(), price_per_token_to_per_1k(cost_per_token)))
            } else {
                None
            }
        })
        .collect::<std::collections::HashMap<_, _>>();

    if tiered.is_empty() {
        None
    } else {
        Some(tiered)
    }
}

fn price_per_token_to_per_1k(cost_per_token: f64) -> f64 {
    let cost_per_1k = cost_per_token * 1000.0;
    (cost_per_1k * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
}

/// Calculate input cost
fn calculate_input_cost(usage: &UsageTokens, cost_per_1k: f64) -> f64 {
    let non_cached_tokens = if let Some(cached) = usage.cached_tokens {
        usage.prompt_tokens.saturating_sub(cached)
    } else {
        usage.prompt_tokens
    };

    (non_cached_tokens as f64 / 1000.0) * cost_per_1k
}

/// Calculate output cost
fn calculate_output_cost(usage: &UsageTokens, cost_per_1k: f64) -> f64 {
    (usage.completion_tokens as f64 / 1000.0) * cost_per_1k
}

/// Calculate cache cost
fn calculate_cache_cost(cached_tokens: u32, _creation_cost: f64, read_cost: f64) -> f64 {
    // Assume all cached tokens are read (typical case)
    (cached_tokens as f64 / 1000.0) * read_cost
}

/// Calculate audio cost
fn calculate_audio_cost(pricing: &ModelPricing, audio_tokens: u32) -> f64 {
    if let Some(audio_cost_per_token) = pricing.input_cost_per_audio_token {
        audio_tokens as f64 * audio_cost_per_token
    } else {
        0.0
    }
}

/// Calculate image cost
fn calculate_image_cost(pricing: &ModelPricing, image_tokens: u32) -> f64 {
    if let Some(image_cost_per_token) = pricing.image_cost_per_token {
        image_tokens as f64 * image_cost_per_token
    } else {
        0.0
    }
}

/// Calculate reasoning tokens cost (for o1 models)
fn calculate_reasoning_cost(pricing: &ModelPricing, reasoning_tokens: u32) -> f64 {
    if let Some(reasoning_cost_per_token) = pricing.reasoning_cost_per_token {
        reasoning_tokens as f64 * reasoning_cost_per_token
    } else {
        0.0
    }
}

/// Estimate cost for a request
pub fn estimate_cost(
    model: &str,
    provider: &str,
    input_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<CostEstimate, CostError> {
    let pricing = get_model_pricing(model, provider)?;
    let estimated_output_tokens = max_output_tokens.unwrap_or(100); // Default estimate
    let usage = UsageTokens::new(input_tokens, estimated_output_tokens);
    let (input_cost_per_1k, output_cost_per_1k, _, _) = select_tiered_pricing(&pricing, &usage);

    let input_cost = (input_tokens as f64 / 1000.0) * input_cost_per_1k;
    let max_output_cost = (estimated_output_tokens as f64 / 1000.0) * output_cost_per_1k;

    Ok(CostEstimate {
        min_cost: input_cost,
        max_cost: input_cost + max_output_cost,
        input_cost,
        estimated_output_cost: max_output_cost,
        currency: pricing.currency,
    })
}

/// Compare costs between different models
pub fn compare_model_costs(
    models: &[(String, String)], // (model, provider) pairs
    input_tokens: u32,
    output_tokens: u32,
) -> Vec<ModelCostComparison> {
    let mut comparisons = Vec::new();
    let usage = UsageTokens::new(input_tokens, output_tokens);

    for (model, provider) in models {
        if let Ok(breakdown) = generic_cost_per_token(model, &usage, provider) {
            let total_tokens = input_tokens + output_tokens;
            let cost_per_token = if total_tokens > 0 {
                breakdown.total_cost / total_tokens as f64
            } else {
                0.0
            };
            let efficiency_score = if breakdown.total_cost > 0.0 {
                total_tokens as f64 / breakdown.total_cost
            } else {
                0.0
            };

            comparisons.push(ModelCostComparison {
                model: model.clone(),
                provider: provider.clone(),
                total_cost: breakdown.total_cost,
                cost_per_token,
                efficiency_score,
            });
        }
    }

    // Sort by cost (lowest first)
    comparisons.sort_by(|a, b| {
        a.total_cost
            .partial_cmp(&b.total_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    comparisons
}

#[cfg(test)]
mod gpt55_tests;
#[cfg(test)]
mod pricing_regression_tests;
#[cfg(test)]
mod tests;
