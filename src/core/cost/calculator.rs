//! Unified Cost Calculator
//!
//! Core cost calculation logic that all providers delegate to.
//! This eliminates code duplication and ensures consistent behavior.

use async_trait::async_trait;

use crate::core::cost::types::{
    CostBreakdown, CostError, CostEstimate, ModelCostComparison, ModelPricing, UsageTokens,
};
use crate::core::cost::utils::select_tiered_pricing;

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
    // This will be populated with actual pricing data
    // For now, return a basic implementation

    match provider.to_lowercase().as_str() {
        "openai" => get_openai_pricing(model),
        "anthropic" => get_anthropic_pricing(model),
        "azure" => get_azure_pricing(model),
        "vertex_ai" | "vertexai" => get_vertex_ai_pricing(model),
        "deepseek" => get_deepseek_pricing(model),
        "moonshot" => get_moonshot_pricing(model),
        "minimax" => get_minimax_pricing(model),
        "zhipu" | "zhipuai" | "glm" | "zai" => get_zhipu_pricing(model),
        _ => Err(CostError::ProviderNotSupported {
            provider: provider.to_string(),
        }),
    }
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

    let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;

    let estimated_output_tokens = max_output_tokens.unwrap_or(100); // Default estimate
    let max_output_cost =
        (estimated_output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

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

// Provider-specific pricing functions
// These will be populated with actual pricing data from JSON or database

fn get_openai_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("gpt-5.4-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.030,
            output_cost_per_1k_tokens: 0.180,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00075,
            output_cost_per_1k_tokens: 0.0045,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4-nano") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0002,
            output_cost_per_1k_tokens: 0.00125,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.021,
            output_cost_per_1k_tokens: 0.168,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2-codex") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00175,
            output_cost_per_1k_tokens: 0.014,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-codex") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.2") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00175,
            output_cost_per_1k_tokens: 0.014,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.1-thinking") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.020,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00025,
            output_cost_per_1k_tokens: 0.002,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-5-nano") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00005,
            output_cost_per_1k_tokens: 0.0004,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1.5") || m.contains("chatgpt-image-latest") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.020,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-image-1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.020,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("computer-use-preview") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.012,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.020,
            output_cost_per_1k_tokens: 0.080,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-deep-research") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.010,
            output_cost_per_1k_tokens: 0.040,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0011,
            output_cost_per_1k_tokens: 0.0044,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o3") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o4-mini-deep-research") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("o4-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0011,
            output_cost_per_1k_tokens: 0.0044,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-audio-1.5") || m == "gpt-audio" => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0025,
            output_cost_per_1k_tokens: 0.010,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-audio-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-realtime-1.5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.004,
            output_cost_per_1k_tokens: 0.016,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-realtime-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4o-mini") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00015,
            output_cost_per_1k_tokens: 0.0006,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4o") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-4-turbo") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.01,
            output_cost_per_1k_tokens: 0.03,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gpt-3.5-turbo") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0005,
            output_cost_per_1k_tokens: 0.0015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "openai".to_string(),
            });
        }
    };

    Ok(pricing)
}

fn get_anthropic_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("claude-opus-4-6") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.005,
            output_cost_per_1k_tokens: 0.025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4-1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-opus-4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-sonnet-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-sonnet-4") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-5-sonnet") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-5-haiku") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-haiku-4-5") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-opus") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.015,
            output_cost_per_1k_tokens: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-sonnet") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.003,
            output_cost_per_1k_tokens: 0.015,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-3-haiku") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00025,
            output_cost_per_1k_tokens: 0.00125,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-2.1") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.008,
            output_cost_per_1k_tokens: 0.024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("claude-instant") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0008,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "anthropic".to_string(),
            });
        }
    };

    Ok(pricing)
}

fn get_azure_pricing(model: &str) -> Result<ModelPricing, CostError> {
    // Azure pricing is typically the same as OpenAI but may have regional differences
    get_openai_pricing(model).map(|mut pricing| {
        pricing.model = model.to_string();
        pricing
    })
}

fn get_vertex_ai_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("gemini-pro") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00125,
            output_cost_per_1k_tokens: 0.00375,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        m if m.contains("gemini-flash") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.000075,
            output_cost_per_1k_tokens: 0.0003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "vertex_ai".to_string(),
            });
        }
    };

    Ok(pricing)
}

fn get_deepseek_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let pricing = match model.to_lowercase().as_str() {
        m if m.contains("deepseek-chat") => ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00014,
            output_cost_per_1k_tokens: 0.00028,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        },
        _ => {
            return Err(CostError::ModelNotSupported {
                model: model.to_string(),
                provider: "deepseek".to_string(),
            });
        }
    };

    Ok(pricing)
}

fn get_moonshot_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("kimi-k2.5") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("kimi-k2-thinking-turbo")
        || normalized_model.contains("kimi-k2-turbo-preview")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00115,
            output_cost_per_1k_tokens: 0.008,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("kimi-k2-thinking")
        || normalized_model.contains("kimi-k2-0905-preview")
        || normalized_model.contains("kimi-k2-0711-preview")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0025,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-8k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0002,
            output_cost_per_1k_tokens: 0.002,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-32k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("moonshot-v1-128k") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.002,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "moonshot".to_string(),
        });
    };

    Ok(pricing)
}

fn get_minimax_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("m2.5-lightning") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0024,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("m2.5")
        || normalized_model.contains("m2.1")
        || normalized_model.contains("minimax-m2")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0003,
            output_cost_per_1k_tokens: 0.0012,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "minimax".to_string(),
        });
    };

    Ok(pricing)
}

fn get_zhipu_pricing(model: &str) -> Result<ModelPricing, CostError> {
    use chrono::Utc;

    let normalized_model = model.to_lowercase();

    let pricing = if normalized_model.contains("glm-5-code") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0012,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5.1") || normalized_model.contains("glm-5-1") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0015,
            output_cost_per_1k_tokens: 0.005,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5-turbo") || normalized_model.contains("glm-5v") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0012,
            output_cost_per_1k_tokens: 0.004,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-5") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.001,
            output_cost_per_1k_tokens: 0.0032,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if (normalized_model.contains("glm-4.7-flash")
        || normalized_model.contains("glm-4.5-flash")
        || normalized_model.contains("glm-4.6v-flash"))
        && !normalized_model.contains("flashx")
    {
        // Free tier flash models
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4.7")
        || normalized_model.contains("glm-4-7")
        || normalized_model.contains("glm-4.6")
        || normalized_model.contains("glm-4.5")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0006,
            output_cost_per_1k_tokens: 0.0022,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4-flash") {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.00005,
            output_cost_per_1k_tokens: 0.0001,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else if normalized_model.contains("glm-4-plus")
        || normalized_model.contains("glm-4-air")
        || normalized_model.contains("glm-4")
    {
        ModelPricing {
            model: model.to_string(),
            input_cost_per_1k_tokens: 0.0001,
            output_cost_per_1k_tokens: 0.0003,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        }
    } else {
        return Err(CostError::ModelNotSupported {
            model: model.to_string(),
            provider: "zhipu".to_string(),
        });
    };

    Ok(pricing)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create basic usage
    fn create_usage(prompt_tokens: u32, completion_tokens: u32) -> UsageTokens {
        UsageTokens::new(prompt_tokens, completion_tokens)
    }

    // Tests for generic_cost_per_token
    #[test]
    fn test_generic_cost_per_token_basic() {
        let usage = create_usage(1000, 500);
        let result = generic_cost_per_token("gpt-4o-mini", &usage, "openai");

        assert!(result.is_ok());
        let breakdown = result.unwrap();
        assert_eq!(breakdown.model, "gpt-4o-mini");
        assert_eq!(breakdown.provider, "openai");
        assert_eq!(breakdown.usage.prompt_tokens, 1000);
        assert_eq!(breakdown.usage.completion_tokens, 500);

        // Expected: 1000 tokens * 0.00015 / 1k = 0.00015
        // Expected: 500 tokens * 0.0006 / 1k = 0.0003
        assert!((breakdown.input_cost - 0.00015).abs() < 1e-6);
        assert!((breakdown.output_cost - 0.0003).abs() < 1e-6);
        assert!((breakdown.total_cost - 0.00045).abs() < 1e-6);
    }

    #[test]
    fn test_generic_cost_per_token_with_cache() {
        let mut usage = create_usage(2000, 1000);
        usage.cached_tokens = Some(500);

        let result = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(result.is_ok());
        let breakdown = result.unwrap();

        // Input cost should only be for non-cached tokens (2000 - 500 = 1500)
        let expected_input = (1500.0 / 1000.0) * 0.005;
        assert!((breakdown.input_cost - expected_input).abs() < 1e-6);
        // Note: cache_cost may be 0 if pricing data doesn't include cache_read_input_token_cost
        // The important thing is that input cost is calculated correctly excluding cached tokens
    }

    #[test]
    fn test_generic_cost_per_token_with_reasoning() {
        let mut usage = create_usage(1000, 500);
        usage.reasoning_tokens = Some(200);

        // Create custom pricing with reasoning cost
        let result = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(result.is_ok());
        // Reasoning cost should be calculated if pricing supports it
    }

    #[test]
    fn test_generic_cost_per_token_unsupported_model() {
        let usage = create_usage(1000, 500);
        let result = generic_cost_per_token("unknown-model", &usage, "openai");

        assert!(result.is_err());
        match result.unwrap_err() {
            CostError::ModelNotSupported { model, provider } => {
                assert_eq!(model, "unknown-model");
                assert_eq!(provider, "openai");
            }
            _ => panic!("Expected ModelNotSupported error"),
        }
    }

    #[test]
    fn test_generic_cost_per_token_unsupported_provider() {
        let usage = create_usage(1000, 500);
        let result = generic_cost_per_token("gpt-4o", &usage, "unknown-provider");

        assert!(result.is_err());
        match result.unwrap_err() {
            CostError::ProviderNotSupported { provider } => {
                assert_eq!(provider, "unknown-provider");
            }
            _ => panic!("Expected ProviderNotSupported error"),
        }
    }

    // Tests for get_model_pricing
    #[test]
    fn test_get_openai_pricing_gpt4o_mini() {
        let pricing = get_model_pricing("gpt-4o-mini", "openai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.00015);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0006);
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn test_get_openai_pricing_gpt4o() {
        let pricing = get_model_pricing("gpt-4o", "openai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.015);
    }

    #[test]
    fn test_get_openai_pricing_gpt4_turbo() {
        let pricing = get_model_pricing("gpt-4-turbo", "openai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.01);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.03);
    }

    #[test]
    fn test_get_openai_pricing_gpt35_turbo() {
        let pricing = get_model_pricing("gpt-3.5-turbo", "openai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0005);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0015);
    }

    #[test]
    fn test_get_anthropic_pricing_claude35_sonnet() {
        let pricing = get_model_pricing("claude-3-5-sonnet", "anthropic");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.003);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.015);
    }

    #[test]
    fn test_get_anthropic_pricing_claude_opus_46() {
        let pricing = get_model_pricing("claude-opus-4-6", "anthropic");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.025);
    }

    #[test]
    fn test_get_anthropic_pricing_claude_sonnet_45() {
        let pricing = get_model_pricing("claude-sonnet-4-5", "anthropic");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.003);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.015);
    }

    #[test]
    fn test_get_anthropic_pricing_claude35_haiku() {
        let pricing = get_model_pricing("claude-3-5-haiku", "anthropic");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.005);
    }

    #[test]
    fn test_get_anthropic_pricing_claude3_haiku() {
        let pricing = get_model_pricing("claude-3-haiku", "anthropic");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.00025);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.00125);
    }

    #[test]
    fn test_get_vertex_ai_pricing_gemini_pro() {
        let pricing = get_model_pricing("gemini-pro", "vertex_ai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.00125);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.00375);
    }

    #[test]
    fn test_get_vertex_ai_pricing_gemini_flash() {
        let pricing = get_model_pricing("gemini-flash", "vertexai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.000075);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0003);
    }

    #[test]
    fn test_get_deepseek_pricing() {
        let pricing = get_model_pricing("deepseek-chat", "deepseek");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.00014);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.00028);
    }

    #[test]
    fn test_get_moonshot_pricing_8k() {
        let pricing = get_model_pricing("moonshot-v1-8k", "moonshot");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0002);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.002);
    }

    #[test]
    fn test_get_moonshot_pricing_32k() {
        let pricing = get_model_pricing("moonshot-v1-32k", "moonshot");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.003);
    }

    #[test]
    fn test_get_moonshot_pricing_128k() {
        let pricing = get_model_pricing("moonshot-v1-128k", "moonshot");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.002);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.005);
    }

    #[test]
    fn test_get_moonshot_pricing_kimi_k2_5() {
        let pricing = get_model_pricing("kimi-k2.5", "moonshot");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0006);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.003);
    }

    #[test]
    fn test_get_minimax_pricing_m2_5() {
        let pricing = get_model_pricing("MiniMax-M2.5", "minimax");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0003);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0012);
    }

    #[test]
    fn test_get_zhipu_pricing_glm_5() {
        let pricing = get_model_pricing("glm-5", "zhipuai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.001);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0032);
    }

    #[test]
    fn test_get_zhipu_pricing_glm_4_flash() {
        let pricing = get_model_pricing("glm-4-flash", "zhipuai");
        assert!(pricing.is_ok());
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.00005);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0001);
    }

    #[test]
    fn test_get_azure_pricing() {
        let pricing = get_model_pricing("gpt-4o", "azure");
        assert!(pricing.is_ok());
        // Azure uses OpenAI pricing
        let pricing = pricing.unwrap();
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
    }

    // Tests for calculate_input_cost
    #[test]
    fn test_calculate_input_cost_no_cache() {
        let usage = create_usage(1000, 500);
        let cost = calculate_input_cost(&usage, 1.0);
        assert_eq!(cost, 1.0);
    }

    #[test]
    fn test_calculate_input_cost_with_cache() {
        let mut usage = create_usage(2000, 500);
        usage.cached_tokens = Some(500);
        let cost = calculate_input_cost(&usage, 1.0);
        // Should only charge for 1500 non-cached tokens
        assert_eq!(cost, 1.5);
    }

    #[test]
    fn test_calculate_input_cost_all_cached() {
        let mut usage = create_usage(1000, 500);
        usage.cached_tokens = Some(1000);
        let cost = calculate_input_cost(&usage, 1.0);
        // All tokens cached, should be 0
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_input_cost_zero_tokens() {
        let usage = create_usage(0, 500);
        let cost = calculate_input_cost(&usage, 1.0);
        assert_eq!(cost, 0.0);
    }

    // Tests for calculate_output_cost
    #[test]
    fn test_calculate_output_cost_basic() {
        let usage = create_usage(1000, 500);
        let cost = calculate_output_cost(&usage, 2.0);
        assert_eq!(cost, 1.0); // 500 / 1000 * 2.0
    }

    #[test]
    fn test_calculate_output_cost_zero() {
        let usage = create_usage(1000, 0);
        let cost = calculate_output_cost(&usage, 2.0);
        assert_eq!(cost, 0.0);
    }

    // Tests for calculate_cache_cost
    #[test]
    fn test_calculate_cache_cost() {
        let cost = calculate_cache_cost(1000, 0.5, 0.1);
        // Using read cost: 1000 / 1000 * 0.1 = 0.1
        assert_eq!(cost, 0.1);
    }

    #[test]
    fn test_calculate_cache_cost_zero_tokens() {
        let cost = calculate_cache_cost(0, 0.5, 0.1);
        assert_eq!(cost, 0.0);
    }

    // Tests for calculate_audio_cost
    #[test]
    fn test_calculate_audio_cost_with_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            input_cost_per_audio_token: Some(0.001),
            ..Default::default()
        };

        let cost = calculate_audio_cost(&pricing, 1000);
        assert_eq!(cost, 1.0); // 1000 * 0.001
    }

    #[test]
    fn test_calculate_audio_cost_no_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        };

        let cost = calculate_audio_cost(&pricing, 1000);
        assert_eq!(cost, 0.0);
    }

    // Tests for calculate_image_cost
    #[test]
    fn test_calculate_image_cost_with_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            image_cost_per_token: Some(0.002),
            ..Default::default()
        };

        let cost = calculate_image_cost(&pricing, 500);
        assert_eq!(cost, 1.0); // 500 * 0.002
    }

    #[test]
    fn test_calculate_image_cost_no_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        };

        let cost = calculate_image_cost(&pricing, 500);
        assert_eq!(cost, 0.0);
    }

    // Tests for calculate_reasoning_cost
    #[test]
    fn test_calculate_reasoning_cost_with_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            reasoning_cost_per_token: Some(0.003),
            ..Default::default()
        };

        let cost = calculate_reasoning_cost(&pricing, 300);
        assert_eq!(cost, 0.9); // 300 * 0.003
    }

    #[test]
    fn test_calculate_reasoning_cost_no_pricing() {
        use chrono::Utc;
        let pricing = ModelPricing {
            model: "test".to_string(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
            ..Default::default()
        };

        let cost = calculate_reasoning_cost(&pricing, 300);
        assert_eq!(cost, 0.0);
    }

    // Tests for estimate_cost
    #[test]
    fn test_estimate_cost_basic() {
        let result = estimate_cost("gpt-4o-mini", "openai", 1000, Some(500));
        assert!(result.is_ok());
        let estimate = result.unwrap();

        let expected_input = (1000.0 / 1000.0) * 0.00015;
        let expected_output = (500.0 / 1000.0) * 0.0006;

        assert!((estimate.input_cost - expected_input).abs() < 1e-6);
        assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
        assert_eq!(estimate.min_cost, expected_input);
        assert!((estimate.max_cost - (expected_input + expected_output)).abs() < 1e-6);
        assert_eq!(estimate.currency, "USD");
    }

    #[test]
    fn test_estimate_cost_no_max_output() {
        let result = estimate_cost("gpt-4o", "openai", 1000, None);
        assert!(result.is_ok());
        let estimate = result.unwrap();

        // Should use default 100 tokens
        let expected_output = (100.0 / 1000.0) * 0.015;
        assert!((estimate.estimated_output_cost - expected_output).abs() < 1e-6);
    }

    #[test]
    fn test_estimate_cost_unsupported_model() {
        let result = estimate_cost("unknown-model", "openai", 1000, Some(500));
        assert!(result.is_err());
    }

    // Tests for compare_model_costs
    #[test]
    fn test_compare_model_costs_single_model() {
        let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
        let comparisons = compare_model_costs(&models, 1000, 500);

        assert_eq!(comparisons.len(), 1);
        assert_eq!(comparisons[0].model, "gpt-4o-mini");
        assert_eq!(comparisons[0].provider, "openai");
        assert!(comparisons[0].total_cost > 0.0);
        assert!(comparisons[0].cost_per_token > 0.0);
        assert!(comparisons[0].efficiency_score > 0.0);
    }

    #[test]
    fn test_compare_model_costs_multiple_models() {
        let models = vec![
            ("gpt-4o".to_string(), "openai".to_string()),
            ("gpt-4o-mini".to_string(), "openai".to_string()),
            ("claude-3-haiku".to_string(), "anthropic".to_string()),
        ];
        let comparisons = compare_model_costs(&models, 1000, 500);

        assert_eq!(comparisons.len(), 3);

        // Should be sorted by cost (lowest first)
        for i in 1..comparisons.len() {
            assert!(comparisons[i - 1].total_cost <= comparisons[i].total_cost);
        }

        // Verify efficiency score calculation
        for comparison in &comparisons {
            let expected_efficiency = 1500.0 / comparison.total_cost;
            assert!((comparison.efficiency_score - expected_efficiency).abs() < 1e-6);
        }
    }

    #[test]
    fn test_compare_model_costs_with_invalid_model() {
        let models = vec![
            ("gpt-4o-mini".to_string(), "openai".to_string()),
            ("invalid-model".to_string(), "openai".to_string()),
            ("claude-3-haiku".to_string(), "anthropic".to_string()),
        ];
        let comparisons = compare_model_costs(&models, 1000, 500);

        // Should only include valid models
        assert_eq!(comparisons.len(), 2);
    }

    #[test]
    fn test_compare_model_costs_empty_list() {
        let models: Vec<(String, String)> = vec![];
        let comparisons = compare_model_costs(&models, 1000, 500);
        assert_eq!(comparisons.len(), 0);
    }

    #[test]
    fn test_compare_model_costs_zero_tokens() {
        let models = vec![("gpt-4o-mini".to_string(), "openai".to_string())];
        let comparisons = compare_model_costs(&models, 0, 0);

        // Should handle zero tokens gracefully
        assert_eq!(comparisons.len(), 1);
        assert_eq!(comparisons[0].total_cost, 0.0);
    }

    // Tests for cost breakdown calculation with all features
    #[test]
    fn test_generic_cost_per_token_all_features() {
        let mut usage = create_usage(5000, 2000);
        usage.cached_tokens = Some(1000);
        usage.audio_tokens = Some(500);
        usage.image_tokens = Some(300);
        usage.reasoning_tokens = Some(200);

        let result = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(result.is_ok());
        let breakdown = result.unwrap();

        // Verify total is sum of all components
        let calculated_total = breakdown.input_cost
            + breakdown.output_cost
            + breakdown.cache_cost
            + breakdown.audio_cost
            + breakdown.image_cost
            + breakdown.reasoning_cost;

        assert!((breakdown.total_cost - calculated_total).abs() < 1e-10);
    }

    // Edge case tests
    #[test]
    fn test_large_token_counts() {
        let usage = create_usage(1_000_000, 500_000);
        let result = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(result.is_ok());
        let breakdown = result.unwrap();
        assert!(breakdown.total_cost > 0.0);
        assert!(breakdown.total_cost < 1_000_000.0); // Sanity check
    }

    #[test]
    fn test_case_insensitive_model_names() {
        let usage = create_usage(1000, 500);

        let result1 = generic_cost_per_token("GPT-4O-MINI", &usage, "openai");
        let result2 = generic_cost_per_token("gpt-4o-mini", &usage, "openai");
        let result3 = generic_cost_per_token("Gpt-4O-Mini", &usage, "openai");

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        let cost1 = result1.unwrap().total_cost;
        let cost2 = result2.unwrap().total_cost;
        let cost3 = result3.unwrap().total_cost;

        assert!((cost1 - cost2).abs() < 1e-10);
        assert!((cost2 - cost3).abs() < 1e-10);
    }

    #[test]
    fn test_case_insensitive_provider_names() {
        let result1 = get_model_pricing("gpt-4o", "OpenAI");
        let result2 = get_model_pricing("gpt-4o", "OPENAI");
        let result3 = get_model_pricing("gpt-4o", "openai");

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
    }

    #[test]
    fn test_new_openai_models_have_cost_pricing() {
        let gpt54_pro = get_model_pricing("gpt-5.4-pro", "openai").unwrap();
        assert_eq!(gpt54_pro.input_cost_per_1k_tokens, 0.030);
        assert_eq!(gpt54_pro.output_cost_per_1k_tokens, 0.180);

        let realtime = get_model_pricing("gpt-realtime-1.5", "openai").unwrap();
        assert_eq!(realtime.input_cost_per_1k_tokens, 0.004);
        assert_eq!(realtime.output_cost_per_1k_tokens, 0.016);

        let deep_research = get_model_pricing("o3-deep-research", "openai").unwrap();
        assert_eq!(deep_research.input_cost_per_1k_tokens, 0.010);
        assert_eq!(deep_research.output_cost_per_1k_tokens, 0.040);
    }

    #[test]
    fn test_new_anthropic_models_have_cost_pricing() {
        let opus41 = get_model_pricing("claude-opus-4-1-20250805", "anthropic").unwrap();
        assert_eq!(opus41.input_cost_per_1k_tokens, 0.015);
        assert_eq!(opus41.output_cost_per_1k_tokens, 0.075);

        let opus4 = get_model_pricing("claude-opus-4-20250514", "anthropic").unwrap();
        assert_eq!(opus4.input_cost_per_1k_tokens, 0.015);
        assert_eq!(opus4.output_cost_per_1k_tokens, 0.075);

        let haiku45 = get_model_pricing("claude-haiku-4-5-20251001", "anthropic").unwrap();
        assert_eq!(haiku45.input_cost_per_1k_tokens, 0.001);
        assert_eq!(haiku45.output_cost_per_1k_tokens, 0.005);
    }

    #[test]
    fn test_vertex_ai_provider_variants() {
        let usage = create_usage(1000, 500);

        let result1 = generic_cost_per_token("gemini-pro", &usage, "vertex_ai");
        let result2 = generic_cost_per_token("gemini-pro", &usage, "vertexai");

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        let cost1 = result1.unwrap().total_cost;
        let cost2 = result2.unwrap().total_cost;
        assert!((cost1 - cost2).abs() < 1e-10);
    }

    #[test]
    fn test_cached_tokens_exceed_prompt_tokens() {
        // Edge case: cached tokens shouldn't exceed prompt tokens
        let mut usage = create_usage(1000, 500);
        usage.cached_tokens = Some(1500);

        let result = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(result.is_ok());

        // Input cost should be 0 due to saturation
        let breakdown = result.unwrap();
        assert_eq!(breakdown.input_cost, 0.0);
    }

    // Integration tests
    #[test]
    fn test_cost_calculation_workflow() {
        // Simulate a complete workflow
        let usage = create_usage(2000, 1000);

        // 1. Get pricing
        let pricing = get_model_pricing("gpt-4o-mini", "openai");
        assert!(pricing.is_ok());

        // 2. Calculate cost
        let breakdown = generic_cost_per_token("gpt-4o-mini", &usage, "openai");
        assert!(breakdown.is_ok());
        let breakdown = breakdown.unwrap();

        // 3. Verify breakdown structure
        assert_eq!(breakdown.model, "gpt-4o-mini");
        assert_eq!(breakdown.provider, "openai");
        assert_eq!(breakdown.currency, "USD");
        assert!(breakdown.total_cost > 0.0);
        assert_eq!(breakdown.usage.total_tokens, 3000);
    }

    #[test]
    fn test_estimate_and_actual_cost_consistency() {
        let input_tokens = 1000;
        let output_tokens = 500;

        // Estimate cost
        let estimate = estimate_cost("gpt-4o", "openai", input_tokens, Some(output_tokens));
        assert!(estimate.is_ok());
        let estimate = estimate.unwrap();

        // Calculate actual cost
        let usage = create_usage(input_tokens, output_tokens);
        let breakdown = generic_cost_per_token("gpt-4o", &usage, "openai");
        assert!(breakdown.is_ok());
        let breakdown = breakdown.unwrap();

        // Actual cost should match estimate max_cost
        assert!((breakdown.total_cost - estimate.max_cost).abs() < 1e-10);
        assert!((breakdown.input_cost - estimate.input_cost).abs() < 1e-10);
    }
}
