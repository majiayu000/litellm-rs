//! Unified Cost Calculation Types
//!
//! Consolidates all cost-related types into a single module to eliminate duplication

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Usage information for cost calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTokens {
    /// Input/prompt tokens
    pub prompt_tokens: u32,
    /// Output/completion tokens  
    pub completion_tokens: u32,
    /// Total tokens (prompt + completion)
    pub total_tokens: u32,
    /// Cached tokens (for prompt caching)
    pub cached_tokens: Option<u32>,
    /// Audio tokens (for speech models)
    pub audio_tokens: Option<u32>,
    /// Image tokens (for vision models)
    pub image_tokens: Option<u32>,
    /// Reasoning tokens (for o1 models)
    pub reasoning_tokens: Option<u32>,
}

impl UsageTokens {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            cached_tokens: None,
            audio_tokens: None,
            image_tokens: None,
            reasoning_tokens: None,
        }
    }
}

impl From<crate::core::types::responses::Usage> for UsageTokens {
    fn from(usage: crate::core::types::responses::Usage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            cached_tokens: usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.cached_tokens),
            audio_tokens: usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|d| d.audio_tokens),
            image_tokens: None,
            reasoning_tokens: usage
                .completion_tokens_details
                .as_ref()
                .and_then(|d| d.reasoning_tokens),
        }
    }
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model name
    pub model: String,
    /// Input cost per 1K tokens (USD)
    pub input_cost_per_1k_tokens: f64,
    /// Output cost per 1K tokens (USD)
    pub output_cost_per_1k_tokens: f64,
    /// Cached input cost per 1K tokens (for prompt caching)
    pub cache_read_input_token_cost: Option<f64>,
    /// Cache creation cost per 1K tokens
    pub cache_creation_input_token_cost: Option<f64>,
    /// Audio input cost per token
    pub input_cost_per_audio_token: Option<f64>,
    /// Audio output cost per token
    pub output_cost_per_audio_token: Option<f64>,
    /// Image cost per token
    pub image_cost_per_token: Option<f64>,
    /// Reasoning tokens cost (for o1 models)
    pub reasoning_cost_per_token: Option<f64>,
    /// Cost per second (for speech/TTS models)
    pub cost_per_second: Option<f64>,
    /// Video processing cost per second
    pub video_cost_per_second: Option<f64>,
    /// Audio processing cost per second
    pub audio_cost_per_second: Option<f64>,
    /// Cost per image (for image generation)
    pub cost_per_image: Option<HashMap<String, f64>>,
    /// Tiered pricing for high volume (above threshold pricing)
    pub tiered_pricing: Option<HashMap<String, f64>>,
    /// Batch processing discount multiplier, when a provider exposes one
    pub batch_discount: Option<f64>,
    /// Currency (usually "USD")
    pub currency: String,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self {
            model: String::new(),
            input_cost_per_1k_tokens: 0.0,
            output_cost_per_1k_tokens: 0.0,
            cache_read_input_token_cost: None,
            cache_creation_input_token_cost: None,
            input_cost_per_audio_token: None,
            output_cost_per_audio_token: None,
            image_cost_per_token: None,
            reasoning_cost_per_token: None,
            cost_per_second: None,
            video_cost_per_second: None,
            audio_cost_per_second: None,
            cost_per_image: None,
            tiered_pricing: None,
            batch_discount: None,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
        }
    }
}

/// Provider-specific pricing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricing {
    /// Provider name
    pub provider: String,
    /// Default pricing fallback
    pub default_pricing: Option<ModelPricing>,
    /// Model-specific pricing
    pub model_pricing: HashMap<String, ModelPricing>,
}

/// Cost estimation for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// Minimum cost (input only)
    pub min_cost: f64,
    /// Maximum cost (input + max output)
    pub max_cost: f64,
    /// Input cost
    pub input_cost: f64,
    /// Estimated output cost
    pub estimated_output_cost: f64,
    /// Currency
    pub currency: String,
}

/// Detailed cost breakdown after completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    /// Total cost
    pub total_cost: f64,
    /// Input/prompt cost
    pub input_cost: f64,
    /// Output/completion cost
    pub output_cost: f64,
    /// Cached tokens cost (if applicable)
    pub cache_cost: f64,
    /// Audio processing cost (if applicable)
    pub audio_cost: f64,
    /// Image processing cost (if applicable)
    pub image_cost: f64,
    /// Reasoning tokens cost (if applicable)
    pub reasoning_cost: f64,
    /// Token usage breakdown
    pub usage: UsageTokens,
    /// Currency
    pub currency: String,
    /// Model used
    pub model: String,
    /// Provider used
    pub provider: String,
}

impl CostBreakdown {
    pub fn new(model: String, provider: String, usage: UsageTokens) -> Self {
        Self {
            total_cost: 0.0,
            input_cost: 0.0,
            output_cost: 0.0,
            cache_cost: 0.0,
            audio_cost: 0.0,
            image_cost: 0.0,
            reasoning_cost: 0.0,
            usage,
            currency: "USD".to_string(),
            model,
            provider,
        }
    }

    pub fn calculate_total(&mut self) {
        self.total_cost = self.input_cost
            + self.output_cost
            + self.cache_cost
            + self.audio_cost
            + self.image_cost
            + self.reasoning_cost;
    }
}

/// Model cost comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCostComparison {
    /// Model name
    pub model: String,
    /// Provider name
    pub provider: String,
    /// Total cost for the comparison
    pub total_cost: f64,
    /// Cost per token
    pub cost_per_token: f64,
    /// Cost efficiency score (higher is better)
    pub efficiency_score: f64,
}

/// Cost tracking for multiple requests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostTracker {
    /// Total accumulated cost
    total_cost: f64,
    /// Individual request costs
    request_costs: Vec<CostBreakdown>,
    /// Cost by provider
    provider_costs: HashMap<String, f64>,
    /// Cost by model
    model_costs: HashMap<String, f64>,
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add cost for a request
    pub fn add_request_cost(&mut self, breakdown: CostBreakdown) {
        self.total_cost += breakdown.total_cost;

        // Track by provider
        *self
            .provider_costs
            .entry(breakdown.provider.clone())
            .or_insert(0.0) += breakdown.total_cost;

        // Track by model
        *self
            .model_costs
            .entry(breakdown.model.clone())
            .or_insert(0.0) += breakdown.total_cost;

        self.request_costs.push(breakdown);
    }

    /// Get total cost
    pub fn total_cost(&self) -> f64 {
        self.total_cost
    }

    /// Get number of requests
    pub fn request_count(&self) -> usize {
        self.request_costs.len()
    }

    /// Get average cost per request
    pub fn average_cost_per_request(&self) -> f64 {
        if self.request_costs.is_empty() {
            0.0
        } else {
            self.total_cost / self.request_costs.len() as f64
        }
    }

    /// Get cost by provider
    pub fn cost_by_provider(&self, provider: &str) -> f64 {
        self.provider_costs.get(provider).copied().unwrap_or(0.0)
    }

    /// Get cost by model
    pub fn cost_by_model(&self, model: &str) -> f64 {
        self.model_costs.get(model).copied().unwrap_or(0.0)
    }

    /// Get most expensive request
    pub fn most_expensive_request(&self) -> Option<&CostBreakdown> {
        self.request_costs.iter().max_by(|a, b| {
            a.total_cost
                .partial_cmp(&b.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get cheapest request
    pub fn cheapest_request(&self) -> Option<&CostBreakdown> {
        self.request_costs.iter().min_by(|a, b| {
            a.total_cost
                .partial_cmp(&b.total_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get cost summary
    pub fn get_summary(&self) -> CostSummary {
        let total_input_tokens: u32 = self
            .request_costs
            .iter()
            .map(|c| c.usage.prompt_tokens)
            .sum();
        let total_output_tokens: u32 = self
            .request_costs
            .iter()
            .map(|c| c.usage.completion_tokens)
            .sum();
        let total_input_cost: f64 = self.request_costs.iter().map(|c| c.input_cost).sum();
        let total_output_cost: f64 = self.request_costs.iter().map(|c| c.output_cost).sum();

        CostSummary {
            total_cost: self.total_cost,
            total_requests: self.request_costs.len(),
            total_input_tokens,
            total_output_tokens,
            total_tokens: total_input_tokens + total_output_tokens,
            total_input_cost,
            total_output_cost,
            average_cost_per_request: self.average_cost_per_request(),
            provider_breakdown: self.provider_costs.clone(),
            model_breakdown: self.model_costs.clone(),
            currency: "USD".to_string(),
        }
    }
}

/// Cost summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    /// Total cost across all requests
    pub total_cost: f64,
    /// Total number of requests
    pub total_requests: usize,
    /// Total input tokens
    pub total_input_tokens: u32,
    /// Total output tokens
    pub total_output_tokens: u32,
    /// Total tokens (input + output)
    pub total_tokens: u32,
    /// Total input cost
    pub total_input_cost: f64,
    /// Total output cost
    pub total_output_cost: f64,
    /// Average cost per request
    pub average_cost_per_request: f64,
    /// Cost breakdown by provider
    pub provider_breakdown: HashMap<String, f64>,
    /// Cost breakdown by model
    pub model_breakdown: HashMap<String, f64>,
    /// Currency
    pub currency: String,
}

/// Generic cost calculation result
#[derive(Debug, Clone)]
pub struct CostResult {
    /// Input cost in USD
    pub input_cost: f64,
    /// Output cost in USD  
    pub output_cost: f64,
    /// Total cost in USD
    pub total_cost: f64,
    /// Additional costs breakdown
    pub additional_costs: HashMap<String, f64>,
}

impl CostResult {
    pub fn new(input_cost: f64, output_cost: f64) -> Self {
        Self {
            input_cost,
            output_cost,
            total_cost: input_cost + output_cost,
            additional_costs: HashMap::new(),
        }
    }

    pub fn with_additional_cost(mut self, cost_type: String, amount: f64) -> Self {
        self.additional_costs.insert(cost_type, amount);
        self.total_cost += amount;
        self
    }
}

/// Cost calculation errors
#[derive(Debug, Error, Clone)]
pub enum CostError {
    #[error("Model not supported: {model} for provider {provider}")]
    ModelNotSupported { model: String, provider: String },

    #[error("Provider not supported: {provider}")]
    ProviderNotSupported { provider: String },

    #[error("Missing pricing information for model: {model}")]
    MissingPricing { model: String },

    #[error("Invalid usage data: {message}")]
    InvalidUsage { message: String },

    #[error("Calculation error: {message}")]
    CalculationError { message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
