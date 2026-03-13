//! Main pricing service implementation

use super::types::{
    CostRange, CostResult, CostType, LiteLLMModelInfo, PricingData, PricingEventType,
    PricingStatistics, PricingUpdateEvent,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Pricing service using LiteLLM data format
#[derive(Debug, Clone)]
pub struct PricingService {
    /// Consolidated pricing data - single lock for model data and timestamp
    pub(super) pricing_data: Arc<RwLock<PricingData>>,
    /// HTTP client for fetching updates
    pub(super) http_client: reqwest::Client,
    /// Pricing data source URL
    pub(super) pricing_url: String,
    /// Cache TTL
    pub(super) cache_ttl: Duration,
    /// Event broadcaster for updates
    pub(super) event_sender: broadcast::Sender<PricingUpdateEvent>,
}

impl PricingService {
    /// Create a new pricing service
    pub fn new(pricing_url: Option<String>) -> Self {
        let (event_sender, _) = broadcast::channel(1000);

        let service = Self {
            pricing_data: Arc::new(RwLock::new(PricingData {
                models: HashMap::new(),
                last_updated: SystemTime::UNIX_EPOCH,
            })),
            http_client: reqwest::Client::new(),
            pricing_url: pricing_url.unwrap_or_else(|| {
                "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json".to_string()
            }),
            cache_ttl: Duration::from_secs(3600), // 1 hour
            event_sender,
        };

        info!("Pricing service initialized with LiteLLM data source");
        service
    }

    /// Get model information
    pub fn get_model_info(&self, model: &str) -> Option<LiteLLMModelInfo> {
        let data = self.pricing_data.read();
        data.models.get(model).cloned()
    }

    /// Calculate completion cost
    pub async fn calculate_completion_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
        total_time_seconds: Option<f64>,
    ) -> Result<CostResult> {
        // Auto-refresh if needed
        if self.needs_refresh()
            && let Err(e) = self.refresh_pricing_data().await
        {
            warn!("Failed to refresh pricing data: {}", e);
        }

        let model_info = self
            .get_model_info(model)
            .ok_or_else(|| GatewayError::not_found(format!("Model not found: {}", model)))?;

        let provider = model_info.litellm_provider.clone();

        // Provider-specific cost calculation
        match provider.as_str() {
            "openai" | "azure" => {
                self.calculate_token_based_cost(model, &model_info, input_tokens, output_tokens)
            }
            "anthropic" => {
                self.calculate_token_based_cost(model, &model_info, input_tokens, output_tokens)
            }
            "google" | "vertex_ai" => self.calculate_google_cost(
                model,
                &model_info,
                input_tokens,
                output_tokens,
                prompt,
                completion,
            ),
            "replicate" | "together_ai" | "baseten" => self.calculate_time_based_cost(
                model,
                &model_info,
                total_time_seconds.unwrap_or(0.0),
            ),
            "zhipuai" | "glm" => {
                self.calculate_token_based_cost(model, &model_info, input_tokens, output_tokens)
            }
            _ => {
                // Default to token-based calculation
                self.calculate_token_based_cost(model, &model_info, input_tokens, output_tokens)
            }
        }
    }

    /// Calculate token-based cost
    pub(super) fn calculate_token_based_cost(
        &self,
        model: &str,
        model_info: &LiteLLMModelInfo,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<CostResult> {
        let input_cost_per_token = model_info.input_cost_per_token.unwrap_or(0.0);
        let output_cost_per_token = model_info.output_cost_per_token.unwrap_or(0.0);

        let input_cost = (input_tokens as f64) * input_cost_per_token;
        let output_cost = (output_tokens as f64) * output_cost_per_token;
        let total_cost = input_cost + output_cost;

        Ok(CostResult {
            input_cost,
            output_cost,
            total_cost,
            input_tokens,
            output_tokens,
            model: model.to_string(),
            provider: model_info.litellm_provider.clone(),
            cost_type: CostType::TokenBased,
        })
    }

    /// Calculate Google/Vertex AI cost (character or token based)
    fn calculate_google_cost(
        &self,
        model: &str,
        model_info: &LiteLLMModelInfo,
        input_tokens: u32,
        output_tokens: u32,
        prompt: Option<&str>,
        completion: Option<&str>,
    ) -> Result<CostResult> {
        // Check if character-based pricing is available
        if model_info.input_cost_per_character.is_some()
            || model_info.output_cost_per_character.is_some()
        {
            let input_cost_per_char = model_info.input_cost_per_character.unwrap_or(0.0);
            let output_cost_per_char = model_info.output_cost_per_character.unwrap_or(0.0);

            let input_chars = prompt.map(|p| p.len()).unwrap_or(0) as f64;
            let output_chars = completion.map(|c| c.len()).unwrap_or(0) as f64;

            let input_cost = input_chars * input_cost_per_char;
            let output_cost = output_chars * output_cost_per_char;

            Ok(CostResult {
                input_cost,
                output_cost,
                total_cost: input_cost + output_cost,
                input_tokens,
                output_tokens,
                model: model.to_string(),
                provider: model_info.litellm_provider.clone(),
                cost_type: CostType::CharacterBased,
            })
        } else {
            // Fall back to token-based
            self.calculate_token_based_cost(model, model_info, input_tokens, output_tokens)
        }
    }

    /// Calculate time-based cost (for deployment providers)
    fn calculate_time_based_cost(
        &self,
        model: &str,
        model_info: &LiteLLMModelInfo,
        total_time_seconds: f64,
    ) -> Result<CostResult> {
        let cost_per_second = model_info.cost_per_second.unwrap_or(0.0);
        let total_cost = total_time_seconds * cost_per_second;

        Ok(CostResult {
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost,
            input_tokens: 0,
            output_tokens: 0,
            model: model.to_string(),
            provider: model_info.litellm_provider.clone(),
            cost_type: CostType::TimeBased,
        })
    }

    /// Get cost per token for a model
    pub fn get_cost_per_token(&self, model: &str) -> Option<(f64, f64)> {
        let model_info = self.get_model_info(model)?;
        Some((
            model_info.input_cost_per_token.unwrap_or(0.0),
            model_info.output_cost_per_token.unwrap_or(0.0),
        ))
    }

    /// Check if model supports a feature
    pub fn supports_feature(&self, model: &str, feature: &str) -> bool {
        let model_info = match self.get_model_info(model) {
            Some(info) => info,
            None => return false,
        };

        match feature {
            "function_calling" => model_info.supports_function_calling.unwrap_or(false),
            "vision" => model_info.supports_vision.unwrap_or(false),
            "streaming" => model_info.supports_streaming.unwrap_or(true), // Default to true
            "parallel_function_calling" => model_info
                .supports_parallel_function_calling
                .unwrap_or(false),
            "system_message" => model_info.supports_system_message.unwrap_or(true),
            _ => false,
        }
    }

    /// Get all available models for a provider
    pub fn get_models_by_provider(&self, provider: &str) -> Vec<String> {
        let data = self.pricing_data.read();
        data.models
            .iter()
            .filter(|(_, info)| info.litellm_provider == provider)
            .map(|(model, _)| model.clone())
            .collect()
    }

    /// Get all available providers
    pub fn get_providers(&self) -> Vec<String> {
        let data = self.pricing_data.read();
        let mut providers: Vec<String> = data
            .models
            .values()
            .map(|info| info.litellm_provider.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        providers.sort();
        providers
    }

    /// Add custom model pricing
    pub fn add_custom_model(&self, model: String, model_info: LiteLLMModelInfo) {
        {
            let mut data = self.pricing_data.write();
            data.models.insert(model.clone(), model_info.clone());
        }

        // Send update event
        let _ = self.event_sender.send(PricingUpdateEvent {
            event_type: PricingEventType::ModelAdded,
            model,
            provider: model_info.litellm_provider,
            timestamp: SystemTime::now(),
        });
    }

    /// Get pricing statistics
    pub fn get_statistics(&self) -> PricingStatistics {
        let data = self.pricing_data.read();
        let total_models = data.models.len();

        let mut provider_stats = HashMap::new();
        let mut cost_ranges = HashMap::new();

        for (_, model_info) in data.models.iter() {
            let provider = &model_info.litellm_provider;
            *provider_stats.entry(provider.clone()).or_insert(0) += 1;

            // Track cost ranges
            if let (Some(input_cost), Some(output_cost)) = (
                model_info.input_cost_per_token,
                model_info.output_cost_per_token,
            ) {
                let range = cost_ranges.entry(provider.clone()).or_insert(CostRange {
                    input_min: f64::MAX,
                    input_max: f64::MIN,
                    output_min: f64::MAX,
                    output_max: f64::MIN,
                });

                range.input_min = range.input_min.min(input_cost);
                range.input_max = range.input_max.max(input_cost);
                range.output_min = range.output_min.min(output_cost);
                range.output_max = range.output_max.max(output_cost);
            }
        }

        PricingStatistics {
            total_models,
            provider_stats,
            cost_ranges,
            last_updated: data.last_updated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Helper Functions ====================

    fn create_test_model_info(provider: &str) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(4096),
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00003),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(false),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(true),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        }
    }

    fn create_character_based_model_info() -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(8192),
            max_input_tokens: Some(8192),
            max_output_tokens: Some(8192),
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: Some(0.000001),
            output_cost_per_character: Some(0.000002),
            cost_per_second: None,
            litellm_provider: "google".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(true),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(false),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        }
    }

    fn create_time_based_model_info() -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(4096),
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: Some(0.001),
            litellm_provider: "replicate".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(false),
            supports_vision: Some(false),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(false),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        }
    }

    // ==================== PricingService Creation Tests ====================

    #[test]
    fn test_pricing_service_new_default() {
        let service = PricingService::new(None);
        assert!(
            service
                .pricing_url
                .contains("model_prices_and_context_window.json")
        );
        assert_eq!(service.cache_ttl, Duration::from_secs(3600));
    }

    #[test]
    fn test_pricing_service_new_custom_url() {
        let custom_url = "https://example.com/pricing.json";
        let service = PricingService::new(Some(custom_url.to_string()));
        assert_eq!(service.pricing_url, custom_url);
    }

    #[test]
    fn test_pricing_service_initial_state() {
        let service = PricingService::new(None);
        let data = service.pricing_data.read();
        assert!(data.models.is_empty());
        assert_eq!(data.last_updated, SystemTime::UNIX_EPOCH);
    }

    // ==================== Model Info Tests ====================

    #[test]
    fn test_get_model_info_not_found() {
        let service = PricingService::new(None);
        let result = service.get_model_info("nonexistent-model");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_model_info_after_add() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");

        service.add_custom_model("gpt-4".to_string(), model_info.clone());

        let result = service.get_model_info("gpt-4");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.litellm_provider, "openai");
    }

    // ==================== Token-Based Cost Calculation Tests ====================

    #[test]
    fn test_calculate_token_based_cost_basic() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");

        let result = service
            .calculate_token_based_cost("gpt-4", &model_info, 1000, 500)
            .unwrap();

        assert_eq!(result.input_tokens, 1000);
        assert_eq!(result.output_tokens, 500);
        assert_eq!(result.input_cost, 1000.0 * 0.00001);
        assert_eq!(result.output_cost, 500.0 * 0.00003);
        assert_eq!(result.total_cost, result.input_cost + result.output_cost);
        assert_eq!(result.cost_type, CostType::TokenBased);
        assert_eq!(result.provider, "openai");
    }

    #[test]
    fn test_calculate_token_based_cost_zero_tokens() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");

        let result = service
            .calculate_token_based_cost("gpt-4", &model_info, 0, 0)
            .unwrap();

        assert_eq!(result.input_cost, 0.0);
        assert_eq!(result.output_cost, 0.0);
        assert_eq!(result.total_cost, 0.0);
    }

    #[test]
    fn test_calculate_token_based_cost_no_pricing() {
        let service = PricingService::new(None);
        let model_info = LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "custom".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };

        let result = service
            .calculate_token_based_cost("custom-model", &model_info, 1000, 500)
            .unwrap();

        // Should default to 0 cost when no pricing is set
        assert_eq!(result.total_cost, 0.0);
    }

    #[test]
    fn test_calculate_token_based_cost_large_tokens() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");

        let result = service
            .calculate_token_based_cost("gpt-4", &model_info, 1_000_000, 100_000)
            .unwrap();

        // Large token counts should work correctly
        assert!(result.total_cost > 0.0);
        assert_eq!(result.input_tokens, 1_000_000);
        assert_eq!(result.output_tokens, 100_000);
    }

    // ==================== Time-Based Cost Calculation Tests ====================

    #[test]
    fn test_calculate_time_based_cost_basic() {
        let service = PricingService::new(None);
        let model_info = create_time_based_model_info();

        let result = service
            .calculate_time_based_cost("replicate/llama", &model_info, 10.0)
            .unwrap();

        assert_eq!(result.total_cost, 10.0 * 0.001);
        assert_eq!(result.cost_type, CostType::TimeBased);
        assert_eq!(result.input_cost, 0.0);
        assert_eq!(result.output_cost, 0.0);
        assert_eq!(result.input_tokens, 0);
        assert_eq!(result.output_tokens, 0);
    }

    #[test]
    fn test_calculate_time_based_cost_zero_time() {
        let service = PricingService::new(None);
        let model_info = create_time_based_model_info();

        let result = service
            .calculate_time_based_cost("replicate/llama", &model_info, 0.0)
            .unwrap();

        assert_eq!(result.total_cost, 0.0);
    }

    #[test]
    fn test_calculate_time_based_cost_fractional_seconds() {
        let service = PricingService::new(None);
        let model_info = create_time_based_model_info();

        let result = service
            .calculate_time_based_cost("replicate/llama", &model_info, 0.5)
            .unwrap();

        assert_eq!(result.total_cost, 0.5 * 0.001);
    }

    // ==================== Feature Support Tests ====================

    #[test]
    fn test_supports_feature_function_calling() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        assert!(service.supports_feature("gpt-4", "function_calling"));
    }

    #[test]
    fn test_supports_feature_vision() {
        let service = PricingService::new(None);
        let model_info = create_character_based_model_info();
        service.add_custom_model("gemini-pro-vision".to_string(), model_info);

        assert!(service.supports_feature("gemini-pro-vision", "vision"));
    }

    #[test]
    fn test_supports_feature_streaming() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        assert!(service.supports_feature("gpt-4", "streaming"));
    }

    #[test]
    fn test_supports_feature_parallel_function_calling() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        assert!(service.supports_feature("gpt-4", "parallel_function_calling"));
    }

    #[test]
    fn test_supports_feature_system_message() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        assert!(service.supports_feature("gpt-4", "system_message"));
    }

    #[test]
    fn test_supports_feature_unknown_feature() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        assert!(!service.supports_feature("gpt-4", "unknown_feature"));
    }

    #[test]
    fn test_supports_feature_nonexistent_model() {
        let service = PricingService::new(None);
        assert!(!service.supports_feature("nonexistent", "function_calling"));
    }

    #[test]
    fn test_supports_feature_streaming_default_true() {
        let service = PricingService::new(None);
        let model_info = LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00003),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None, // Not set
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        service.add_custom_model("test-model".to_string(), model_info);

        // Streaming defaults to true when not specified
        assert!(service.supports_feature("test-model", "streaming"));
    }

    // ==================== Cost Per Token Tests ====================

    #[test]
    fn test_get_cost_per_token_exists() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("openai");
        service.add_custom_model("gpt-4".to_string(), model_info);

        let result = service.get_cost_per_token("gpt-4");
        assert!(result.is_some());
        let (input, output) = result.unwrap();
        assert_eq!(input, 0.00001);
        assert_eq!(output, 0.00003);
    }

    #[test]
    fn test_get_cost_per_token_not_found() {
        let service = PricingService::new(None);
        let result = service.get_cost_per_token("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_cost_per_token_no_pricing() {
        let service = PricingService::new(None);
        let model_info = LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "custom".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        service.add_custom_model("free-model".to_string(), model_info);

        let result = service.get_cost_per_token("free-model");
        assert!(result.is_some());
        let (input, output) = result.unwrap();
        assert_eq!(input, 0.0);
        assert_eq!(output, 0.0);
    }

    // ==================== Provider and Model Listing Tests ====================

    #[test]
    fn test_get_models_by_provider_empty() {
        let service = PricingService::new(None);
        let models = service.get_models_by_provider("openai");
        assert!(models.is_empty());
    }

    #[test]
    fn test_get_models_by_provider_with_models() {
        let service = PricingService::new(None);
        service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
        service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
        service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));

        let openai_models = service.get_models_by_provider("openai");
        assert_eq!(openai_models.len(), 2);
        assert!(openai_models.contains(&"gpt-4".to_string()));
        assert!(openai_models.contains(&"gpt-3.5".to_string()));

        let anthropic_models = service.get_models_by_provider("anthropic");
        assert_eq!(anthropic_models.len(), 1);
        assert!(anthropic_models.contains(&"claude-3".to_string()));
    }

    #[test]
    fn test_get_providers_empty() {
        let service = PricingService::new(None);
        let providers = service.get_providers();
        assert!(providers.is_empty());
    }

    #[test]
    fn test_get_providers_with_models() {
        let service = PricingService::new(None);
        service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
        service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));
        service.add_custom_model("gemini-pro".to_string(), create_test_model_info("google"));

        let providers = service.get_providers();
        assert_eq!(providers.len(), 3);
        // Sorted alphabetically
        assert_eq!(providers[0], "anthropic");
        assert_eq!(providers[1], "google");
        assert_eq!(providers[2], "openai");
    }

    #[test]
    fn test_get_providers_deduplication() {
        let service = PricingService::new(None);
        service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
        service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
        service.add_custom_model("gpt-4-turbo".to_string(), create_test_model_info("openai"));

        let providers = service.get_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0], "openai");
    }

    // ==================== Add Custom Model Tests ====================

    #[test]
    fn test_add_custom_model() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("custom");

        service.add_custom_model("my-custom-model".to_string(), model_info.clone());

        let result = service.get_model_info("my-custom-model");
        assert!(result.is_some());
        assert_eq!(result.unwrap().litellm_provider, "custom");
    }

    #[test]
    fn test_add_custom_model_overwrites() {
        let service = PricingService::new(None);
        let model_info1 = create_test_model_info("provider1");
        let model_info2 = create_test_model_info("provider2");

        service.add_custom_model("model".to_string(), model_info1);
        service.add_custom_model("model".to_string(), model_info2);

        let result = service.get_model_info("model");
        assert!(result.is_some());
        assert_eq!(result.unwrap().litellm_provider, "provider2");
    }

    // ==================== Statistics Tests ====================

    #[test]
    fn test_get_statistics_empty() {
        let service = PricingService::new(None);
        let stats = service.get_statistics();

        assert_eq!(stats.total_models, 0);
        assert!(stats.provider_stats.is_empty());
        assert!(stats.cost_ranges.is_empty());
    }

    #[test]
    fn test_get_statistics_with_models() {
        let service = PricingService::new(None);
        service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
        service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
        service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));

        let stats = service.get_statistics();

        assert_eq!(stats.total_models, 3);
        assert_eq!(*stats.provider_stats.get("openai").unwrap(), 2);
        assert_eq!(*stats.provider_stats.get("anthropic").unwrap(), 1);
    }

    #[test]
    fn test_get_statistics_cost_ranges() {
        let service = PricingService::new(None);

        let mut cheap_model = create_test_model_info("openai");
        cheap_model.input_cost_per_token = Some(0.000001);
        cheap_model.output_cost_per_token = Some(0.000002);

        let mut expensive_model = create_test_model_info("openai");
        expensive_model.input_cost_per_token = Some(0.00006);
        expensive_model.output_cost_per_token = Some(0.00012);

        service.add_custom_model("gpt-3.5".to_string(), cheap_model);
        service.add_custom_model("gpt-4".to_string(), expensive_model);

        let stats = service.get_statistics();

        let range = stats.cost_ranges.get("openai").unwrap();
        assert_eq!(range.input_min, 0.000001);
        assert_eq!(range.input_max, 0.00006);
        assert_eq!(range.output_min, 0.000002);
        assert_eq!(range.output_max, 0.00012);
    }

    // ==================== Google/Character-Based Cost Tests ====================

    #[test]
    fn test_calculate_google_cost_character_based() {
        let service = PricingService::new(None);
        let model_info = create_character_based_model_info();

        let prompt = "Hello, world!"; // 13 chars
        let completion = "Hi there!"; // 9 chars

        let result = service
            .calculate_google_cost(
                "gemini-pro",
                &model_info,
                10,
                5,
                Some(prompt),
                Some(completion),
            )
            .unwrap();

        assert_eq!(result.cost_type, CostType::CharacterBased);
        assert_eq!(result.input_cost, 13.0 * 0.000001);
        assert_eq!(result.output_cost, 9.0 * 0.000002);
    }

    #[test]
    fn test_calculate_google_cost_fallback_to_token() {
        let service = PricingService::new(None);
        let model_info = create_test_model_info("google");

        let result = service
            .calculate_google_cost(
                "gemini-pro",
                &model_info,
                1000,
                500,
                Some("prompt"),
                Some("completion"),
            )
            .unwrap();

        // Should fall back to token-based
        assert_eq!(result.cost_type, CostType::TokenBased);
    }

    #[test]
    fn test_calculate_google_cost_no_text() {
        let service = PricingService::new(None);
        let model_info = create_character_based_model_info();

        let result = service
            .calculate_google_cost("gemini-pro", &model_info, 10, 5, None, None)
            .unwrap();

        // Should still calculate based on 0 characters
        assert_eq!(result.input_cost, 0.0);
        assert_eq!(result.output_cost, 0.0);
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_pricing_service_clone() {
        let service = PricingService::new(None);
        service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));

        let cloned = service.clone();

        // Both should see the same data
        assert!(cloned.get_model_info("gpt-4").is_some());

        // Adding to original should be visible in clone (same Arc)
        service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
        assert!(cloned.get_model_info("gpt-3.5").is_some());
    }
}
