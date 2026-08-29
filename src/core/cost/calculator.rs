//! Unified Cost Calculator
//!
//! Core cost calculation logic that all providers delegate to.
//! This eliminates code duplication and ensures consistent behavior.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::LazyLock;

use crate::core::cost::types::{
    CostBreakdown, CostError, CostEstimate, ModelCostComparison, ModelPricing, UsageTokens,
};
use crate::core::cost::utils::select_tiered_pricing;
use crate::core::pricing_service::{PricingCostBreakdown, PricingService, PricingUsage};
use crate::utils::error::gateway_error::GatewayError;

mod catalog;
pub(crate) mod pricing;

use self::catalog::{
    get_pricing_with_shared_source, get_pricing_with_shared_source_at, litellm_to_cost_pricing_at,
};
#[cfg(test)]
use self::catalog::{get_shared_model_pricing, litellm_to_cost_pricing};
use self::pricing::{
    get_anthropic_pricing, get_azure_pricing, get_deepseek_pricing, get_deepseek_pricing_at,
    get_minimax_pricing, get_moonshot_pricing, get_openai_pricing, get_vertex_ai_pricing,
    get_zhipu_pricing,
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
    generic_cost_per_token_at(model, usage, provider, Utc::now())
}

/// Calculate cost using prices effective at a specific UTC instant.
pub fn generic_cost_per_token_at(
    model: &str,
    usage: &UsageTokens,
    provider: &str,
    pricing_time: DateTime<Utc>,
) -> Result<CostBreakdown, CostError> {
    let pricing_usage = pricing_usage_from_cost_usage(usage);
    match default_pricing_authority().calculate_loaded_usage_cost_for_provider_at(
        provider,
        model,
        &pricing_usage,
        pricing_time,
    ) {
        Ok(breakdown) => {
            return Ok(pricing_breakdown_to_cost_breakdown(
                model, provider, usage, breakdown,
            ));
        }
        Err(error) if !is_not_found(&error) => {
            return Err(gateway_error_to_cost_error(error, model, provider));
        }
        Err(_) => {
            // Fall through to legacy provider catalogs for models that are not in
            // the shared pricing source but were already supported before GH-726.
        }
    }

    let pricing = get_fallback_model_pricing_at(model, provider, pricing_time)?;
    calculate_with_model_pricing(model, provider, usage, pricing)
}

/// Get model pricing information
pub fn get_model_pricing(model: &str, provider: &str) -> Result<ModelPricing, CostError> {
    get_model_pricing_at(model, provider, Utc::now())
}

/// Get model pricing effective at a specific UTC instant.
pub fn get_model_pricing_at(
    model: &str,
    provider: &str,
    pricing_time: DateTime<Utc>,
) -> Result<ModelPricing, CostError> {
    if let Some((resolved_model, info)) =
        default_pricing_authority().get_model_info_for_provider_at(provider, model, pricing_time)
    {
        return litellm_to_cost_pricing_at(&resolved_model, &info, pricing_time);
    }

    get_fallback_model_pricing_at(model, provider, pricing_time)
}

fn default_pricing_authority() -> &'static PricingService {
    static DEFAULT_PRICING_AUTHORITY: LazyLock<PricingService> = LazyLock::new(|| {
        PricingService::with_embedded_default().unwrap_or_else(|error| {
            tracing::error!("failed to initialize embedded PricingService authority: {error}");
            PricingService::new(None)
        })
    });
    &DEFAULT_PRICING_AUTHORITY
}

fn get_fallback_model_pricing(model: &str, provider: &str) -> Result<ModelPricing, CostError> {
    let normalized_provider = crate::core::pricing::normalize_pricing_provider(provider);

    match normalized_provider.as_str() {
        "openai" => get_pricing_with_shared_source(model, &["openai"], get_openai_pricing),
        "anthropic" if is_xiaomi_mimo_model(model) => {
            get_pricing_with_shared_source(model, &["xiaomi_mimo", "xiaomi", "mimo"], |model| {
                Err(CostError::ModelNotSupported {
                    model: model.to_string(),
                    provider: "anthropic".to_string(),
                })
            })
        }
        "anthropic" => get_pricing_with_shared_source(model, &["anthropic"], get_anthropic_pricing),
        "azure" => get_azure_pricing(model),
        "vertex_ai" => {
            get_pricing_with_shared_source(model, &["vertex_ai", "google"], get_vertex_ai_pricing)
        }
        "gemini" => {
            get_pricing_with_shared_source(model, &["gemini", "vertex_ai"], get_vertex_ai_pricing)
        }
        "bedrock" => get_pricing_with_shared_source(model, &["bedrock"], get_bedrock_pricing),
        "amazon_nova" => get_amazon_nova_pricing(model),
        "openai_like" => get_openai_like_pricing(model),
        "xai" => get_xai_pricing(model),
        "groq" => get_pricing_with_shared_source(model, &["groq"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "groq".to_string(),
            })
        }),
        "cohere" => get_pricing_with_shared_source(model, &["cohere"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "cohere".to_string(),
            })
        }),
        "deepseek" => get_pricing_with_shared_source(model, &["deepseek"], get_deepseek_pricing),
        "xiaomi_mimo" => {
            get_pricing_with_shared_source(model, &["xiaomi_mimo", "xiaomi", "mimo"], |model| {
                Err(CostError::ModelNotSupported {
                    model: model.to_string(),
                    provider: "xiaomi_mimo".to_string(),
                })
            })
        }
        "moonshot" => get_pricing_with_shared_source(model, &["moonshot"], get_moonshot_pricing),
        "minimax" => get_pricing_with_shared_source(model, &["minimax"], get_minimax_pricing),
        "zhipuai" => get_pricing_with_shared_source(model, &["zhipuai", "glm"], get_zhipu_pricing),
        "zai" => get_pricing_with_shared_source(model, &["zai"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "zai".to_string(),
            })
        }),
        "together_ai" => get_pricing_with_shared_source(model, &["together_ai"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "together_ai".to_string(),
            })
        }),
        "fireworks_ai" => get_pricing_with_shared_source(model, &["fireworks_ai"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "fireworks_ai".to_string(),
            })
        }),
        "aiml" => get_pricing_with_shared_source(model, &["aiml"], |model| {
            Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "aiml".to_string(),
            })
        }),
        _ => Err(CostError::ProviderNotSupported {
            provider: provider.to_string(),
        }),
    }
}

fn get_fallback_model_pricing_at(
    model: &str,
    provider: &str,
    pricing_time: DateTime<Utc>,
) -> Result<ModelPricing, CostError> {
    let normalized_provider = crate::core::pricing::normalize_pricing_provider(provider);
    if normalized_provider == "deepseek" {
        return get_pricing_with_shared_source_at(
            model,
            &["deepseek"],
            |model| get_deepseek_pricing_at(model, pricing_time),
            pricing_time,
        );
    }
    if normalized_provider == "openai_like" {
        if let Some((prefixed_provider, stripped_model)) = provider_prefixed_model(model) {
            let prefixed_provider =
                crate::core::pricing::normalize_pricing_provider(prefixed_provider);
            if prefixed_provider != "openai_like" {
                return get_model_pricing_at(stripped_model, &prefixed_provider, pricing_time);
            }
        }
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "openai_like".to_string(),
        });
    }
    get_fallback_model_pricing(model, provider)
}

#[cfg(test)]
#[test]
fn amazon_nova_fallback_pricing_prefers_catalog_over_shared_bedrock() {
    assert!(get_shared_model_pricing("amazon.nova-pro-v1:0", &["bedrock"]).is_some());
    for model in ["amazon.nova-pro-v1:0", "nova-pro"] {
        let pricing = get_fallback_model_pricing(model, "amazon_nova").unwrap();
        assert_eq!(pricing.model, "amazon.nova-pro-v1:0");
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0008);
    }
}
fn pricing_usage_from_cost_usage(usage: &UsageTokens) -> PricingUsage {
    PricingUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
        cache_creation_tokens: None,
        cache_read_tokens: None,
        audio_tokens: usage.audio_tokens,
        output_audio_tokens: None,
        image_tokens: usage.image_tokens,
        reasoning_tokens: usage.reasoning_tokens,
        output_image_count: None,
        output_image_pricing_keys: Vec::new(),
    }
}

fn pricing_breakdown_to_cost_breakdown(
    model: &str,
    provider: &str,
    usage: &UsageTokens,
    pricing: PricingCostBreakdown,
) -> CostBreakdown {
    CostBreakdown {
        total_cost: pricing.total_cost,
        input_cost: pricing.input_cost,
        output_cost: pricing.output_cost,
        cache_cost: pricing.cache_cost,
        audio_cost: pricing.audio_cost,
        image_cost: pricing.image_cost,
        reasoning_cost: pricing.reasoning_cost,
        usage: usage.clone(),
        currency: pricing.currency,
        model: model.to_string(),
        provider: provider.to_string(),
    }
}

fn calculate_with_model_pricing(
    model: &str,
    provider: &str,
    usage: &UsageTokens,
    pricing: ModelPricing,
) -> Result<CostBreakdown, CostError> {
    let mut breakdown = CostBreakdown::new(model.to_string(), provider.to_string(), usage.clone());
    let (_, _, cache_creation_cost_per_1k, cache_read_cost_per_1k) =
        select_tiered_pricing(&pricing, usage);
    let (input_cost_per_1k, output_cost_per_1k, _, _) = select_tiered_pricing(&pricing, usage);

    breakdown.input_cost = calculate_input_cost(usage, input_cost_per_1k);
    breakdown.output_cost = calculate_output_cost(usage, output_cost_per_1k);

    if let Some(cached_tokens) = usage.cached_tokens {
        breakdown.cache_cost = calculate_cache_cost(
            cached_tokens,
            cache_creation_cost_per_1k,
            cache_read_cost_per_1k,
        );
    }
    if let Some(audio_tokens) = usage.audio_tokens {
        breakdown.audio_cost = calculate_audio_cost(&pricing, audio_tokens);
    }
    if let Some(image_tokens) = usage.image_tokens {
        breakdown.image_cost = calculate_image_cost(&pricing, image_tokens);
    }
    if let Some(reasoning_tokens) = usage.reasoning_tokens {
        breakdown.reasoning_cost = calculate_reasoning_cost(&pricing, reasoning_tokens);
    }

    breakdown.calculate_total();
    Ok(breakdown)
}

fn gateway_error_to_cost_error(error: GatewayError, model: &str, provider: &str) -> CostError {
    match error {
        GatewayError::NotFound(_) if provider_is_supported(provider) => {
            CostError::ModelNotSupported {
                model: model.to_string(),
                provider: provider.to_string(),
            }
        }
        GatewayError::NotFound(_) => CostError::ProviderNotSupported {
            provider: provider.to_string(),
        },
        GatewayError::Config(message) if message.contains("Missing") => CostError::MissingPricing {
            model: model.to_string(),
        },
        GatewayError::Validation(message) => CostError::InvalidUsage { message },
        GatewayError::Config(message) => CostError::ConfigError { message },
        other => CostError::CalculationError {
            message: other.to_string(),
        },
    }
}

fn is_not_found(error: &GatewayError) -> bool {
    matches!(error, GatewayError::NotFound(_))
}

fn provider_is_supported(provider: &str) -> bool {
    matches!(
        crate::core::pricing::normalize_pricing_provider(provider).as_str(),
        "openai"
            | "anthropic"
            | "azure"
            | "azure_ai"
            | "vertex_ai"
            | "gemini"
            | "bedrock"
            | "amazon_nova"
            | "openai_like"
            | "xai"
            | "groq"
            | "cohere"
            | "deepseek"
            | "xiaomi_mimo"
            | "moonshot"
            | "minimax"
            | "zhipuai"
            | "zai"
            | "together_ai"
            | "fireworks_ai"
            | "aiml"
    )
}

fn is_xiaomi_mimo_model(model: &str) -> bool {
    crate::core::pricing::normalize_model_key(model).starts_with("mimo-")
}

fn get_bedrock_pricing(model: &str) -> Result<ModelPricing, CostError> {
    crate::core::providers::bedrock::CostCalculator::get_core_model_pricing(model).ok_or_else(
        || CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "bedrock".to_string(),
        },
    )
}

fn get_amazon_nova_pricing(model: &str) -> Result<ModelPricing, CostError> {
    let entry = crate::core::providers::registry::catalog::amazon_nova_catalog_model(model)
        .ok_or_else(|| CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "amazon_nova".to_string(),
        })?;
    Ok(ModelPricing {
        model: entry.model_id.to_string(),
        input_cost_per_1k_tokens: entry.input_cost_per_million / 1_000.0,
        output_cost_per_1k_tokens: entry.output_cost_per_million / 1_000.0,
        ..Default::default()
    })
}

fn get_openai_like_pricing(model: &str) -> Result<ModelPricing, CostError> {
    if let Some((provider, stripped_model)) = provider_prefixed_model(model) {
        let normalized_provider = crate::core::pricing::normalize_pricing_provider(provider);
        if normalized_provider != "openai_like" {
            return get_model_pricing(stripped_model, &normalized_provider);
        }
    }

    Err(CostError::ModelNotSupported {
        model: model.to_string(),
        provider: "openai_like".to_string(),
    })
}

fn get_xai_pricing(model: &str) -> Result<ModelPricing, CostError> {
    if !crate::core::providers::openai_like::models::is_xai_priced_model(model) {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "xai".to_string(),
        });
    }

    let model_info = crate::core::providers::openai_like::models::get_openai_like_registry()
        .get_model_info(model);
    let input = model_info
        .input_cost_per_1k_tokens
        .ok_or_else(|| CostError::MissingPricing {
            model: model.to_string(),
        })?;
    let output = model_info
        .output_cost_per_1k_tokens
        .ok_or_else(|| CostError::MissingPricing {
            model: model.to_string(),
        })?;

    Ok(ModelPricing {
        model: model.to_string(),
        input_cost_per_1k_tokens: input,
        output_cost_per_1k_tokens: output,
        currency: model_info.currency,
        ..Default::default()
    })
}

fn provider_prefixed_model(model: &str) -> Option<(&str, &str)> {
    let (provider, stripped_model) = model.split_once('/')?;
    if provider.is_empty() || stripped_model.is_empty() {
        return None;
    }
    Some((provider, stripped_model))
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
    match default_pricing_authority().estimate_loaded_completion_cost_for_provider(
        provider,
        model,
        input_tokens,
        max_output_tokens,
    ) {
        Ok(estimate) => {
            return Ok(CostEstimate {
                min_cost: estimate.min_cost,
                max_cost: estimate.max_cost,
                input_cost: estimate.input_cost,
                estimated_output_cost: estimate.estimated_output_cost,
                currency: estimate.currency,
            });
        }
        Err(error) if !is_not_found(&error) => {
            return Err(gateway_error_to_cost_error(error, model, provider));
        }
        Err(_) => {
            // Fall through to legacy provider catalogs for compatibility models
            // absent from the shared pricing authority.
        }
    }

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
mod openai_current_tests;
#[cfg(test)]
mod pricing_regression_tests;
#[cfg(test)]
mod tests;
