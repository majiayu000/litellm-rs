//! Main pricing service implementation

use super::types::{
    CostRange, CostResult, CostType, LiteLLMModelInfo, PricingData, PricingEventType,
    PricingStatistics, PricingUpdateEvent,
};
use crate::core::http::outbound::default_outbound_client;
use crate::utils::error::gateway_error::{GatewayError, Result};
use arc_swap::ArcSwap;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;
use tracing::info;

pub(super) fn require_pricing_field(
    value: Option<f64>,
    model: &str,
    pricing_type: &str,
    field: &str,
) -> Result<f64> {
    let value = value.ok_or_else(|| {
        GatewayError::Config(format!(
            "Missing {} for model {}: {}",
            pricing_type, model, field
        ))
    })?;
    if value < 0.0 || value.is_nan() {
        return Err(GatewayError::Config(format!(
            "Invalid {} for model {}: {} ({})",
            pricing_type, model, field, value
        )));
    }
    Ok(value)
}

pub(super) fn require_total_time_seconds(
    model: &str,
    total_time_seconds: Option<f64>,
) -> Result<f64> {
    let total_time_seconds = total_time_seconds.ok_or_else(|| {
        GatewayError::validation(format!(
            "Missing total_time_seconds for time-based pricing model {}",
            model
        ))
    })?;
    if total_time_seconds < 0.0 || total_time_seconds.is_nan() {
        return Err(GatewayError::validation(format!(
            "Invalid total_time_seconds ({}) for model {}",
            total_time_seconds, model
        )));
    }
    Ok(total_time_seconds)
}

/// Pricing service using LiteLLM data format
#[derive(Debug, Clone)]
pub struct PricingService {
    /// Immutable model/index/timestamp snapshot published atomically.
    pub(super) pricing_data: Arc<ArcSwap<PricingData>>,
    /// Serializes writers so insert and refresh cannot lose each other's update.
    pub(super) pricing_write_lock: Arc<Mutex<()>>,
    /// HTTP client for fetching updates
    pub(super) http_client: reqwest::Client,
    /// Pricing data source URL
    pub(super) pricing_url: String,
    /// Whether the built-in remote source may fall back to embedded pricing.
    pub(super) use_embedded_fallback_on_remote_error: bool,
    /// Cache TTL
    pub(super) cache_ttl: Duration,
    /// Event broadcaster for updates
    pub(super) event_sender: broadcast::Sender<PricingUpdateEvent>,
}

impl PricingService {
    /// Create a new pricing service
    pub fn new(pricing_url: Option<String>) -> Self {
        let (event_sender, _) = broadcast::channel(1000);
        let use_embedded_fallback_on_remote_error = false;

        let service = Self {
            pricing_data: Arc::new(ArcSwap::from_pointee(PricingData::default())),
            pricing_write_lock: Arc::new(Mutex::new(())),
            http_client: default_outbound_client().clone(),
            pricing_url: pricing_url.unwrap_or_default(),
            use_embedded_fallback_on_remote_error,
            cache_ttl: Duration::from_secs(3600), // 1 hour
            event_sender,
        };

        info!("Pricing service initialized with LiteLLM data source");
        service
    }

    pub(super) fn should_fallback_to_embedded_on_remote_error(&self) -> bool {
        self.use_embedded_fallback_on_remote_error
            && self.pricing_url == super::REMOTE_LITELLM_PRICING_SOURCE
            && self.pricing_data.load().models.is_empty()
    }

    /// Get model information
    pub fn get_model_info(&self, model: &str) -> Option<LiteLLMModelInfo> {
        let data = self.pricing_data.load();
        data.models.get(model).cloned()
    }

    /// Calculate Google/Vertex AI cost (character or token based)
    pub(super) fn calculate_google_cost(
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
            let input_cost_per_char = require_pricing_field(
                model_info.input_cost_per_character,
                model,
                "character pricing",
                "input_cost_per_character",
            )?;
            let output_cost_per_char = require_pricing_field(
                model_info.output_cost_per_character,
                model,
                "character pricing",
                "output_cost_per_character",
            )?;

            let input_chars = prompt.map(|p| p.chars().count()).unwrap_or(0) as f64;
            let output_chars = completion.map(|c| c.chars().count()).unwrap_or(0) as f64;

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
    pub(super) fn calculate_time_based_cost(
        &self,
        model: &str,
        model_info: &LiteLLMModelInfo,
        total_time_seconds: f64,
    ) -> Result<CostResult> {
        let cost_per_second = require_pricing_field(
            model_info.cost_per_second,
            model,
            "time pricing",
            "cost_per_second",
        )?;
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
            model_info.input_cost_per_token?,
            model_info.output_cost_per_token?,
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
        let data = self.pricing_data.load();
        data.models
            .iter()
            .filter(|(_, info)| info.litellm_provider == provider)
            .map(|(model, _)| model.clone())
            .collect()
    }

    /// Get all available providers
    pub fn get_providers(&self) -> Vec<String> {
        let data = self.pricing_data.load();
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
        let timestamp = SystemTime::now();
        {
            let _write_guard = self.pricing_write_lock.lock();
            let mut models = self.pricing_data.load().models.clone();
            models.insert(model.clone(), model_info.clone());
            self.pricing_data
                .store(Arc::new(build_pricing_data(models, timestamp)));
        }

        // Send update event
        let _ = self.event_sender.send(PricingUpdateEvent {
            event_type: PricingEventType::ModelAdded,
            model,
            provider: model_info.litellm_provider,
            timestamp,
        });
    }

    /// Get pricing statistics
    pub fn get_statistics(&self) -> PricingStatistics {
        let data = self.pricing_data.load();
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

pub(super) fn build_pricing_data(
    models: HashMap<String, LiteLLMModelInfo>,
    last_updated: SystemTime,
) -> PricingData {
    let mut exact_by_provider: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    for (canonical, info) in &models {
        let provider = crate::core::pricing::normalize_pricing_provider(&info.litellm_provider);
        exact_by_provider
            .entry(provider)
            .or_default()
            .entry(canonical.to_ascii_lowercase())
            .or_default()
            .push(canonical.clone());
    }
    for candidates in exact_by_provider.values_mut().flat_map(HashMap::values_mut) {
        candidates.sort();
        candidates.dedup();
    }
    PricingData {
        models,
        exact_by_provider,
        last_updated,
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
        assert!(service.pricing_url.is_empty());
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
        let data = service.pricing_data.load();
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

        let result = service.calculate_token_based_cost("custom-model", &model_info, 1000, 500);

        assert!(matches!(
            result,
            Err(GatewayError::Config(message))
                if message.contains("custom-model") && message.contains("token pricing")
        ));
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

    #[test]
    fn test_calculate_time_based_cost_no_pricing() {
        let service = PricingService::new(None);
        let mut model_info = create_time_based_model_info();
        model_info.cost_per_second = None;

        let result = service.calculate_time_based_cost("replicate/llama", &model_info, 10.0);

        assert!(matches!(
            result,
            Err(GatewayError::Config(message))
                if message.contains("replicate/llama") && message.contains("time pricing")
        ));
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
        assert!(result.is_none());
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod service_tests;
