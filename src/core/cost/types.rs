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
    /// Cost per image (for image generation)
    pub cost_per_image: Option<HashMap<String, f64>>,
    /// Tiered pricing for high volume (above threshold pricing)
    pub tiered_pricing: Option<HashMap<String, f64>>,
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
            cost_per_image: None,
            tiered_pricing: None,
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
mod tests {
    use super::*;

    // ==================== Usage → UsageTokens Conversion Tests ====================

    #[test]
    fn test_usage_to_usage_tokens_basic() {
        use crate::core::types::responses::Usage;
        let usage = Usage::new(100, 50);
        let tokens: UsageTokens = usage.into();
        assert_eq!(tokens.prompt_tokens, 100);
        assert_eq!(tokens.completion_tokens, 50);
        assert_eq!(tokens.total_tokens, 150);
        assert!(tokens.cached_tokens.is_none());
        assert!(tokens.reasoning_tokens.is_none());
    }

    #[test]
    fn test_usage_to_usage_tokens_with_details() {
        use crate::core::types::responses::{CompletionTokensDetails, PromptTokensDetails, Usage};
        let usage = Usage {
            prompt_tokens: 200,
            completion_tokens: 100,
            total_tokens: 300,
            prompt_tokens_details: Some(PromptTokensDetails {
                cached_tokens: Some(50),
                audio_tokens: Some(10),
            }),
            completion_tokens_details: Some(CompletionTokensDetails {
                reasoning_tokens: Some(30),
                audio_tokens: None,
            }),
            thinking_usage: None,
        };
        let tokens: UsageTokens = usage.into();
        assert_eq!(tokens.cached_tokens, Some(50));
        assert_eq!(tokens.audio_tokens, Some(10));
        assert_eq!(tokens.reasoning_tokens, Some(30));
        assert!(tokens.image_tokens.is_none());
    }

    // ==================== UsageTokens Tests ====================

    #[test]
    fn test_usage_tokens_new() {
        let usage = UsageTokens::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert!(usage.cached_tokens.is_none());
        assert!(usage.audio_tokens.is_none());
        assert!(usage.image_tokens.is_none());
        assert!(usage.reasoning_tokens.is_none());
    }

    #[test]
    fn test_usage_tokens_zero() {
        let usage = UsageTokens::new(0, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_usage_tokens_clone() {
        let usage = UsageTokens::new(100, 50);
        let cloned = usage.clone();
        assert_eq!(usage.prompt_tokens, cloned.prompt_tokens);
        assert_eq!(usage.completion_tokens, cloned.completion_tokens);
    }

    // ==================== ModelPricing Tests ====================

    #[test]
    fn test_model_pricing_default() {
        let pricing = ModelPricing::default();
        assert!(pricing.model.is_empty());
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.0);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0);
        assert_eq!(pricing.currency, "USD");
        assert!(pricing.cache_read_input_token_cost.is_none());
    }

    #[test]
    fn test_model_pricing_serialization() {
        let pricing = ModelPricing {
            model: "gpt-4".to_string(),
            input_cost_per_1k_tokens: 0.03,
            output_cost_per_1k_tokens: 0.06,
            ..Default::default()
        };

        let json = serde_json::to_value(&pricing).unwrap();
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["input_cost_per_1k_tokens"], 0.03);
        assert_eq!(json["output_cost_per_1k_tokens"], 0.06);
    }

    // ==================== CostEstimate Tests ====================

    #[test]
    fn test_cost_estimate_structure() {
        let estimate = CostEstimate {
            min_cost: 0.01,
            max_cost: 0.05,
            input_cost: 0.01,
            estimated_output_cost: 0.04,
            currency: "USD".to_string(),
        };

        assert_eq!(estimate.min_cost, 0.01);
        assert_eq!(estimate.max_cost, 0.05);
        assert_eq!(estimate.currency, "USD");
    }

    #[test]
    fn test_cost_estimate_serialization() {
        let estimate = CostEstimate {
            min_cost: 0.01,
            max_cost: 0.05,
            input_cost: 0.01,
            estimated_output_cost: 0.04,
            currency: "USD".to_string(),
        };

        let json = serde_json::to_string(&estimate).unwrap();
        assert!(json.contains("min_cost"));
        assert!(json.contains("max_cost"));
    }

    // ==================== CostBreakdown Tests ====================

    #[test]
    fn test_cost_breakdown_new() {
        let usage = UsageTokens::new(100, 50);
        let breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

        assert_eq!(breakdown.model, "gpt-4");
        assert_eq!(breakdown.provider, "openai");
        assert_eq!(breakdown.total_cost, 0.0);
        assert_eq!(breakdown.input_cost, 0.0);
        assert_eq!(breakdown.output_cost, 0.0);
        assert_eq!(breakdown.currency, "USD");
    }

    #[test]
    fn test_cost_breakdown_calculate_total() {
        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

        breakdown.input_cost = 0.01;
        breakdown.output_cost = 0.02;
        breakdown.cache_cost = 0.005;
        breakdown.audio_cost = 0.003;
        breakdown.image_cost = 0.002;
        breakdown.reasoning_cost = 0.001;

        breakdown.calculate_total();

        let expected = 0.01 + 0.02 + 0.005 + 0.003 + 0.002 + 0.001;
        assert!((breakdown.total_cost - expected).abs() < 1e-10);
    }

    #[test]
    fn test_cost_breakdown_serialization() {
        let usage = UsageTokens::new(100, 50);
        let breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);

        let json = serde_json::to_value(&breakdown).unwrap();
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["provider"], "openai");
    }

    // ==================== CostTracker Tests ====================

    #[test]
    fn test_cost_tracker_new() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.total_cost(), 0.0);
        assert_eq!(tracker.request_count(), 0);
    }

    #[test]
    fn test_cost_tracker_add_request_cost() {
        let mut tracker = CostTracker::new();

        let usage = UsageTokens::new(100, 50);
        let mut breakdown = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
        breakdown.total_cost = 0.05;

        tracker.add_request_cost(breakdown);

        assert_eq!(tracker.total_cost(), 0.05);
        assert_eq!(tracker.request_count(), 1);
    }

    #[test]
    fn test_cost_tracker_multiple_requests() {
        let mut tracker = CostTracker::new();

        for i in 0..5 {
            let usage = UsageTokens::new(100, 50);
            let mut breakdown =
                CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
            breakdown.total_cost = 0.01 * (i + 1) as f64;
            tracker.add_request_cost(breakdown);
        }

        assert_eq!(tracker.request_count(), 5);
        // 0.01 + 0.02 + 0.03 + 0.04 + 0.05 = 0.15
        assert!((tracker.total_cost() - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_cost_tracker_average_cost_per_request() {
        let mut tracker = CostTracker::new();

        for _ in 0..4 {
            let usage = UsageTokens::new(100, 50);
            let mut breakdown =
                CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
            breakdown.total_cost = 0.02;
            tracker.add_request_cost(breakdown);
        }

        assert!((tracker.average_cost_per_request() - 0.02).abs() < 1e-10);
    }

    #[test]
    fn test_cost_tracker_average_cost_empty() {
        let tracker = CostTracker::new();
        assert_eq!(tracker.average_cost_per_request(), 0.0);
    }

    #[test]
    fn test_cost_tracker_cost_by_provider() {
        let mut tracker = CostTracker::new();

        let usage1 = UsageTokens::new(100, 50);
        let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
        breakdown1.total_cost = 0.05;
        tracker.add_request_cost(breakdown1);

        let usage2 = UsageTokens::new(100, 50);
        let mut breakdown2 =
            CostBreakdown::new("claude-3".to_string(), "anthropic".to_string(), usage2);
        breakdown2.total_cost = 0.03;
        tracker.add_request_cost(breakdown2);

        assert!((tracker.cost_by_provider("openai") - 0.05).abs() < 1e-10);
        assert!((tracker.cost_by_provider("anthropic") - 0.03).abs() < 1e-10);
        assert_eq!(tracker.cost_by_provider("unknown"), 0.0);
    }

    #[test]
    fn test_cost_tracker_cost_by_model() {
        let mut tracker = CostTracker::new();

        let usage1 = UsageTokens::new(100, 50);
        let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
        breakdown1.total_cost = 0.05;
        tracker.add_request_cost(breakdown1);

        let usage2 = UsageTokens::new(100, 50);
        let mut breakdown2 =
            CostBreakdown::new("gpt-3.5".to_string(), "openai".to_string(), usage2);
        breakdown2.total_cost = 0.01;
        tracker.add_request_cost(breakdown2);

        assert!((tracker.cost_by_model("gpt-4") - 0.05).abs() < 1e-10);
        assert!((tracker.cost_by_model("gpt-3.5") - 0.01).abs() < 1e-10);
        assert_eq!(tracker.cost_by_model("unknown"), 0.0);
    }

    #[test]
    fn test_cost_tracker_most_expensive_request() {
        let mut tracker = CostTracker::new();

        let costs = [0.01, 0.05, 0.02, 0.03];
        for cost in costs {
            let usage = UsageTokens::new(100, 50);
            let mut breakdown =
                CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
            breakdown.total_cost = cost;
            tracker.add_request_cost(breakdown);
        }

        let most_expensive = tracker.most_expensive_request().unwrap();
        assert!((most_expensive.total_cost - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_cost_tracker_cheapest_request() {
        let mut tracker = CostTracker::new();

        let costs = [0.01, 0.05, 0.02, 0.03];
        for cost in costs {
            let usage = UsageTokens::new(100, 50);
            let mut breakdown =
                CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage);
            breakdown.total_cost = cost;
            tracker.add_request_cost(breakdown);
        }

        let cheapest = tracker.cheapest_request().unwrap();
        assert!((cheapest.total_cost - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_cost_tracker_most_expensive_empty() {
        let tracker = CostTracker::new();
        assert!(tracker.most_expensive_request().is_none());
    }

    #[test]
    fn test_cost_tracker_get_summary() {
        let mut tracker = CostTracker::new();

        let usage1 = UsageTokens::new(100, 50);
        let mut breakdown1 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage1);
        breakdown1.total_cost = 0.05;
        breakdown1.input_cost = 0.03;
        breakdown1.output_cost = 0.02;
        tracker.add_request_cost(breakdown1);

        let usage2 = UsageTokens::new(200, 100);
        let mut breakdown2 = CostBreakdown::new("gpt-4".to_string(), "openai".to_string(), usage2);
        breakdown2.total_cost = 0.10;
        breakdown2.input_cost = 0.06;
        breakdown2.output_cost = 0.04;
        tracker.add_request_cost(breakdown2);

        let summary = tracker.get_summary();

        assert_eq!(summary.total_requests, 2);
        assert!((summary.total_cost - 0.15).abs() < 1e-10);
        assert_eq!(summary.total_input_tokens, 300);
        assert_eq!(summary.total_output_tokens, 150);
        assert_eq!(summary.total_tokens, 450);
        assert!((summary.total_input_cost - 0.09).abs() < 1e-10);
        assert!((summary.total_output_cost - 0.06).abs() < 1e-10);
        assert_eq!(summary.currency, "USD");
    }

    // ==================== CostResult Tests ====================

    #[test]
    fn test_cost_result_new() {
        let result = CostResult::new(0.05, 0.10);
        assert_eq!(result.input_cost, 0.05);
        assert_eq!(result.output_cost, 0.10);
        assert!((result.total_cost - 0.15).abs() < 1e-10);
        assert!(result.additional_costs.is_empty());
    }

    #[test]
    fn test_cost_result_with_additional_cost() {
        let result = CostResult::new(0.05, 0.10)
            .with_additional_cost("cache".to_string(), 0.02)
            .with_additional_cost("audio".to_string(), 0.01);

        assert!((result.total_cost - 0.18).abs() < 1e-10);
        assert_eq!(result.additional_costs.len(), 2);
        assert_eq!(result.additional_costs.get("cache"), Some(&0.02));
        assert_eq!(result.additional_costs.get("audio"), Some(&0.01));
    }

    // ==================== CostError Tests ====================

    #[test]
    fn test_cost_error_model_not_supported() {
        let error = CostError::ModelNotSupported {
            model: "unknown-model".to_string(),
            provider: "openai".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("unknown-model"));
        assert!(msg.contains("openai"));
    }

    #[test]
    fn test_cost_error_provider_not_supported() {
        let error = CostError::ProviderNotSupported {
            provider: "unknown-provider".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("unknown-provider"));
    }

    #[test]
    fn test_cost_error_missing_pricing() {
        let error = CostError::MissingPricing {
            model: "gpt-4".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("gpt-4"));
    }

    #[test]
    fn test_cost_error_invalid_usage() {
        let error = CostError::InvalidUsage {
            message: "negative tokens".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("negative tokens"));
    }

    #[test]
    fn test_cost_error_calculation_error() {
        let error = CostError::CalculationError {
            message: "overflow occurred".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("overflow occurred"));
    }

    #[test]
    fn test_cost_error_config_error() {
        let error = CostError::ConfigError {
            message: "invalid config".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("invalid config"));
    }

    #[test]
    fn test_cost_error_clone() {
        let error = CostError::ModelNotSupported {
            model: "test".to_string(),
            provider: "test".to_string(),
        };
        let cloned = error.clone();
        assert_eq!(error.to_string(), cloned.to_string());
    }

    // ==================== ModelCostComparison Tests ====================

    #[test]
    fn test_model_cost_comparison_structure() {
        let comparison = ModelCostComparison {
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            total_cost: 0.05,
            cost_per_token: 0.00005,
            efficiency_score: 20000.0,
        };

        assert_eq!(comparison.model, "gpt-4");
        assert_eq!(comparison.provider, "openai");
        assert_eq!(comparison.total_cost, 0.05);
    }

    #[test]
    fn test_model_cost_comparison_serialization() {
        let comparison = ModelCostComparison {
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            total_cost: 0.05,
            cost_per_token: 0.00005,
            efficiency_score: 20000.0,
        };

        let json = serde_json::to_value(&comparison).unwrap();
        assert_eq!(json["model"], "gpt-4");
        assert_eq!(json["total_cost"], 0.05);
    }

    // ==================== ProviderPricing Tests ====================

    #[test]
    fn test_provider_pricing_structure() {
        let mut model_pricing = HashMap::new();
        model_pricing.insert(
            "gpt-4".to_string(),
            ModelPricing {
                model: "gpt-4".to_string(),
                input_cost_per_1k_tokens: 0.03,
                output_cost_per_1k_tokens: 0.06,
                ..Default::default()
            },
        );

        let provider_pricing = ProviderPricing {
            provider: "openai".to_string(),
            default_pricing: None,
            model_pricing,
        };

        assert_eq!(provider_pricing.provider, "openai");
        assert!(provider_pricing.model_pricing.contains_key("gpt-4"));
    }

    // ==================== CostSummary Tests ====================

    #[test]
    fn test_cost_summary_serialization() {
        let summary = CostSummary {
            total_cost: 0.15,
            total_requests: 2,
            total_input_tokens: 300,
            total_output_tokens: 150,
            total_tokens: 450,
            total_input_cost: 0.09,
            total_output_cost: 0.06,
            average_cost_per_request: 0.075,
            provider_breakdown: HashMap::new(),
            model_breakdown: HashMap::new(),
            currency: "USD".to_string(),
        };

        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["total_cost"], 0.15);
        assert_eq!(json["total_requests"], 2);
        assert_eq!(json["currency"], "USD");
    }
}
