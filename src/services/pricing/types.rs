//! Type definitions for the pricing service

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// LiteLLM compatible model pricing data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteLLMModelInfo {
    /// Maximum total tokens
    pub max_tokens: Option<u32>,
    /// Maximum input tokens
    pub max_input_tokens: Option<u32>,
    /// Maximum output tokens
    pub max_output_tokens: Option<u32>,
    /// Input cost per token
    pub input_cost_per_token: Option<f64>,
    /// Output cost per token
    pub output_cost_per_token: Option<f64>,
    /// Input cost per character (for some providers)
    pub input_cost_per_character: Option<f64>,
    /// Output cost per character (for some providers)
    pub output_cost_per_character: Option<f64>,
    /// Cost per second (for time-based providers)
    pub cost_per_second: Option<f64>,
    /// LiteLLM provider name
    pub litellm_provider: String,
    /// Model mode (chat, completion, embedding, etc.)
    pub mode: String,
    /// Supports function calling
    pub supports_function_calling: Option<bool>,
    /// Supports vision
    pub supports_vision: Option<bool>,
    /// Supports streaming
    pub supports_streaming: Option<bool>,
    /// Supports parallel function calling
    pub supports_parallel_function_calling: Option<bool>,
    /// Supports system message
    pub supports_system_message: Option<bool>,
    /// Additional metadata
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Consolidated pricing data - single lock for all pricing state
#[derive(Debug)]
pub(super) struct PricingData {
    /// Model pricing data (model_name -> LiteLLMModelInfo)
    pub models: HashMap<String, LiteLLMModelInfo>,
    /// Last update time
    pub last_updated: SystemTime,
}

impl Default for PricingData {
    fn default() -> Self {
        Self {
            models: HashMap::new(),
            last_updated: SystemTime::UNIX_EPOCH,
        }
    }
}

/// Pricing update event
/// Event for pricing updates
#[derive(Debug, Clone)]
pub struct PricingUpdateEvent {
    /// Type of pricing event that occurred
    pub event_type: PricingEventType,
    /// Model name that was affected
    pub model: String,
    /// Provider name that was affected
    pub provider: String,
    /// When the event occurred
    pub timestamp: SystemTime,
}

/// Types of pricing events that can occur
#[derive(Debug, Clone)]
pub enum PricingEventType {
    /// A new model was added to the pricing data
    ModelAdded,
    /// An existing model's pricing was updated
    ModelUpdated,
    /// A model was removed from the pricing data
    ModelRemoved,
    /// The entire pricing dataset was refreshed
    DataRefreshed,
}

/// Cost calculation result
#[derive(Debug, Clone, Serialize)]
pub struct CostResult {
    /// Cost for input tokens/characters
    pub input_cost: f64,
    /// Cost for output tokens/characters
    pub output_cost: f64,
    /// Total cost (input + output)
    pub total_cost: f64,
    /// Number of input tokens used
    pub input_tokens: u32,
    /// Number of output tokens used
    pub output_tokens: u32,
    /// The model name used for pricing calculation
    pub model: String,
    /// The provider name (e.g., "openai", "anthropic")
    pub provider: String,
    /// The type of cost calculation used
    pub cost_type: CostType,
}

/// Type of cost calculation method
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum CostType {
    /// Cost calculated based on token count
    TokenBased,
    /// Cost calculated based on character count
    CharacterBased,
    /// Cost calculated based on time duration
    TimeBased,
    /// Custom cost calculation method
    Custom,
}

/// Pricing statistics
#[derive(Debug, Clone)]
pub struct PricingStatistics {
    /// Total number of models in the pricing database
    pub total_models: usize,
    /// Number of models per provider
    pub provider_stats: HashMap<String, usize>,
    /// Cost ranges for each provider
    pub cost_ranges: HashMap<String, CostRange>,
    /// When the pricing data was last updated
    pub last_updated: SystemTime,
}

/// Cost range statistics for a provider
#[derive(Debug, Clone)]
pub struct CostRange {
    /// Minimum input cost per token
    pub input_min: f64,
    /// Maximum input cost per token
    pub input_max: f64,
    /// Minimum output cost per token
    pub output_min: f64,
    /// Maximum output cost per token
    pub output_max: f64,
}

// ====================================================================================
// TESTS
// ====================================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    // ====================================================================================
    // LiteLLMModelInfo Tests
    // ====================================================================================

    #[test]
    fn test_model_info_minimal() {
        let info = LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        assert_eq!(info.litellm_provider, "openai");
        assert_eq!(info.mode, "chat");
    }

    #[test]
    fn test_model_info_full() {
        let info = LiteLLMModelInfo {
            max_tokens: Some(128000),
            max_input_tokens: Some(100000),
            max_output_tokens: Some(8192),
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00003),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(true),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(true),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        };
        assert_eq!(info.max_tokens, Some(128000));
        assert_eq!(info.input_cost_per_token, Some(0.00001));
        assert!(info.supports_function_calling.unwrap());
    }

    #[test]
    fn test_model_info_character_based() {
        let info = LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: Some(0.0000001),
            output_cost_per_character: Some(0.0000002),
            cost_per_second: None,
            litellm_provider: "google".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        assert!(info.input_cost_per_character.is_some());
        assert!(info.output_cost_per_character.is_some());
    }

    #[test]
    fn test_model_info_time_based() {
        let info = LiteLLMModelInfo {
            max_tokens: None,
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: None,
            output_cost_per_token: None,
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: Some(0.001),
            litellm_provider: "replicate".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        assert_eq!(info.cost_per_second, Some(0.001));
    }

    #[test]
    fn test_model_info_with_extra() {
        let mut extra = HashMap::new();
        extra.insert(
            "custom_field".to_string(),
            serde_json::json!("custom_value"),
        );
        extra.insert("custom_number".to_string(), serde_json::json!(42));

        let info = LiteLLMModelInfo {
            max_tokens: None,
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
            extra,
        };
        assert_eq!(info.extra.len(), 2);
        assert_eq!(info.extra.get("custom_field").unwrap(), "custom_value");
    }

    #[test]
    fn test_model_info_clone() {
        let info = LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00002),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        let cloned = info.clone();
        assert_eq!(info.max_tokens, cloned.max_tokens);
        assert_eq!(info.litellm_provider, cloned.litellm_provider);
    }

    #[test]
    fn test_model_info_serialization() {
        let info = LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: None,
            max_output_tokens: None,
            input_cost_per_token: Some(0.00001),
            output_cost_per_token: Some(0.00002),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: "openai".to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("openai"));
        assert!(json.contains("4096"));
    }

    #[test]
    fn test_model_info_deserialization() {
        let json = r#"{
            "max_tokens": 8192,
            "input_cost_per_token": 0.00001,
            "output_cost_per_token": 0.00002,
            "litellm_provider": "anthropic",
            "mode": "chat",
            "supports_function_calling": true
        }"#;
        let info: LiteLLMModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.max_tokens, Some(8192));
        assert_eq!(info.litellm_provider, "anthropic");
        assert!(info.supports_function_calling.unwrap());
    }

    // ====================================================================================
    // PricingData Tests
    // ====================================================================================

    #[test]
    fn test_pricing_data_default() {
        let data = PricingData::default();
        assert!(data.models.is_empty());
        assert_eq!(data.last_updated, SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn test_pricing_data_with_models() {
        let mut models = HashMap::new();
        models.insert(
            "gpt-4".to_string(),
            LiteLLMModelInfo {
                max_tokens: Some(8192),
                max_input_tokens: None,
                max_output_tokens: None,
                input_cost_per_token: Some(0.00003),
                output_cost_per_token: Some(0.00006),
                input_cost_per_character: None,
                output_cost_per_character: None,
                cost_per_second: None,
                litellm_provider: "openai".to_string(),
                mode: "chat".to_string(),
                supports_function_calling: Some(true),
                supports_vision: None,
                supports_streaming: None,
                supports_parallel_function_calling: None,
                supports_system_message: None,
                extra: HashMap::new(),
            },
        );

        let data = PricingData {
            models,
            last_updated: SystemTime::now(),
        };

        assert_eq!(data.models.len(), 1);
        assert!(data.models.contains_key("gpt-4"));
    }

    // ====================================================================================
    // PricingUpdateEvent Tests
    // ====================================================================================

    #[test]
    fn test_pricing_update_event_creation() {
        let event = PricingUpdateEvent {
            event_type: PricingEventType::ModelAdded,
            model: "gpt-4-turbo".to_string(),
            provider: "openai".to_string(),
            timestamp: SystemTime::now(),
        };
        assert_eq!(event.model, "gpt-4-turbo");
        assert_eq!(event.provider, "openai");
    }

    #[test]
    fn test_pricing_update_event_clone() {
        let event = PricingUpdateEvent {
            event_type: PricingEventType::ModelUpdated,
            model: "claude-3".to_string(),
            provider: "anthropic".to_string(),
            timestamp: SystemTime::now(),
        };
        let cloned = event.clone();
        assert_eq!(event.model, cloned.model);
        assert_eq!(event.provider, cloned.provider);
    }

    // ====================================================================================
    // PricingEventType Tests
    // ====================================================================================

    #[test]
    fn test_pricing_event_type_model_added() {
        let event_type = PricingEventType::ModelAdded;
        assert!(matches!(event_type, PricingEventType::ModelAdded));
    }

    #[test]
    fn test_pricing_event_type_model_updated() {
        let event_type = PricingEventType::ModelUpdated;
        assert!(matches!(event_type, PricingEventType::ModelUpdated));
    }

    #[test]
    fn test_pricing_event_type_model_removed() {
        let event_type = PricingEventType::ModelRemoved;
        assert!(matches!(event_type, PricingEventType::ModelRemoved));
    }

    #[test]
    fn test_pricing_event_type_data_refreshed() {
        let event_type = PricingEventType::DataRefreshed;
        assert!(matches!(event_type, PricingEventType::DataRefreshed));
    }

    #[test]
    fn test_pricing_event_type_clone() {
        let event_type = PricingEventType::ModelAdded;
        let cloned = event_type.clone();
        assert!(matches!(cloned, PricingEventType::ModelAdded));
    }

    // ====================================================================================
    // CostResult Tests
    // ====================================================================================

    #[test]
    fn test_cost_result_creation() {
        let result = CostResult {
            input_cost: 0.001,
            output_cost: 0.002,
            total_cost: 0.003,
            input_tokens: 100,
            output_tokens: 50,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            cost_type: CostType::TokenBased,
        };
        assert_eq!(result.input_cost, 0.001);
        assert_eq!(result.output_cost, 0.002);
        assert_eq!(result.total_cost, 0.003);
        assert_eq!(result.input_tokens, 100);
        assert_eq!(result.output_tokens, 50);
    }

    #[test]
    fn test_cost_result_zero_cost() {
        let result = CostResult {
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            model: "test".to_string(),
            provider: "test".to_string(),
            cost_type: CostType::TokenBased,
        };
        assert_eq!(result.total_cost, 0.0);
    }

    #[test]
    fn test_cost_result_clone() {
        let result = CostResult {
            input_cost: 0.01,
            output_cost: 0.02,
            total_cost: 0.03,
            input_tokens: 1000,
            output_tokens: 500,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            cost_type: CostType::TokenBased,
        };
        let cloned = result.clone();
        assert_eq!(result.total_cost, cloned.total_cost);
        assert_eq!(result.model, cloned.model);
    }

    #[test]
    fn test_cost_result_serialization() {
        let result = CostResult {
            input_cost: 0.001,
            output_cost: 0.002,
            total_cost: 0.003,
            input_tokens: 100,
            output_tokens: 50,
            model: "gpt-4".to_string(),
            provider: "openai".to_string(),
            cost_type: CostType::TokenBased,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("0.003"));
    }

    // ====================================================================================
    // CostType Tests
    // ====================================================================================

    #[test]
    fn test_cost_type_token_based() {
        let cost_type = CostType::TokenBased;
        assert!(matches!(cost_type, CostType::TokenBased));
    }

    #[test]
    fn test_cost_type_character_based() {
        let cost_type = CostType::CharacterBased;
        assert!(matches!(cost_type, CostType::CharacterBased));
    }

    #[test]
    fn test_cost_type_time_based() {
        let cost_type = CostType::TimeBased;
        assert!(matches!(cost_type, CostType::TimeBased));
    }

    #[test]
    fn test_cost_type_custom() {
        let cost_type = CostType::Custom;
        assert!(matches!(cost_type, CostType::Custom));
    }

    #[test]
    fn test_cost_type_clone() {
        let cost_type = CostType::TokenBased;
        let cloned = cost_type.clone();
        assert!(matches!(cloned, CostType::TokenBased));
    }

    #[test]
    fn test_cost_type_serialization() {
        let token = CostType::TokenBased;
        let char = CostType::CharacterBased;
        let time = CostType::TimeBased;
        let custom = CostType::Custom;

        let json_token = serde_json::to_string(&token).unwrap();
        let json_char = serde_json::to_string(&char).unwrap();
        let json_time = serde_json::to_string(&time).unwrap();
        let json_custom = serde_json::to_string(&custom).unwrap();

        assert!(json_token.contains("TokenBased"));
        assert!(json_char.contains("CharacterBased"));
        assert!(json_time.contains("TimeBased"));
        assert!(json_custom.contains("Custom"));
    }

    // ====================================================================================
    // PricingStatistics Tests
    // ====================================================================================

    #[test]
    fn test_pricing_statistics_creation() {
        let mut provider_stats = HashMap::new();
        provider_stats.insert("openai".to_string(), 50);
        provider_stats.insert("anthropic".to_string(), 20);

        let stats = PricingStatistics {
            total_models: 70,
            provider_stats,
            cost_ranges: HashMap::new(),
            last_updated: SystemTime::now(),
        };

        assert_eq!(stats.total_models, 70);
        assert_eq!(stats.provider_stats.len(), 2);
        assert_eq!(stats.provider_stats.get("openai"), Some(&50));
    }

    #[test]
    fn test_pricing_statistics_with_cost_ranges() {
        let mut cost_ranges = HashMap::new();
        cost_ranges.insert(
            "openai".to_string(),
            CostRange {
                input_min: 0.00001,
                input_max: 0.0001,
                output_min: 0.00002,
                output_max: 0.0002,
            },
        );

        let stats = PricingStatistics {
            total_models: 10,
            provider_stats: HashMap::new(),
            cost_ranges,
            last_updated: SystemTime::now(),
        };

        assert!(stats.cost_ranges.contains_key("openai"));
    }

    #[test]
    fn test_pricing_statistics_clone() {
        let stats = PricingStatistics {
            total_models: 100,
            provider_stats: HashMap::new(),
            cost_ranges: HashMap::new(),
            last_updated: SystemTime::now(),
        };
        let cloned = stats.clone();
        assert_eq!(stats.total_models, cloned.total_models);
    }

    // ====================================================================================
    // CostRange Tests
    // ====================================================================================

    #[test]
    fn test_cost_range_creation() {
        let range = CostRange {
            input_min: 0.00001,
            input_max: 0.0001,
            output_min: 0.00002,
            output_max: 0.0002,
        };
        assert_eq!(range.input_min, 0.00001);
        assert_eq!(range.input_max, 0.0001);
        assert_eq!(range.output_min, 0.00002);
        assert_eq!(range.output_max, 0.0002);
    }

    #[test]
    fn test_cost_range_same_min_max() {
        let range = CostRange {
            input_min: 0.00001,
            input_max: 0.00001,
            output_min: 0.00002,
            output_max: 0.00002,
        };
        assert_eq!(range.input_min, range.input_max);
        assert_eq!(range.output_min, range.output_max);
    }

    #[test]
    fn test_cost_range_zero() {
        let range = CostRange {
            input_min: 0.0,
            input_max: 0.0,
            output_min: 0.0,
            output_max: 0.0,
        };
        assert_eq!(range.input_min, 0.0);
    }

    #[test]
    fn test_cost_range_clone() {
        let range = CostRange {
            input_min: 0.00001,
            input_max: 0.0001,
            output_min: 0.00002,
            output_max: 0.0002,
        };
        let cloned = range.clone();
        assert_eq!(range.input_min, cloned.input_min);
        assert_eq!(range.output_max, cloned.output_max);
    }

    #[test]
    fn test_cost_range_range_calculation() {
        let range = CostRange {
            input_min: 0.00001,
            input_max: 0.0001,
            output_min: 0.00002,
            output_max: 0.0002,
        };
        let input_range = range.input_max - range.input_min;
        let output_range = range.output_max - range.output_min;
        assert!(input_range > 0.0);
        assert!(output_range > 0.0);
    }
}
