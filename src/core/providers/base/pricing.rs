//! Unified pricing calculation system
//!
//! Shares LiteLLM pricing JSON data with the runtime pricing service.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use tracing::warn;

const DEFAULT_PRICING_SOURCE: &str = "config/model_prices_extended.json";

/// Compatibility alias for callers that still import provider-base pricing.
pub type ModelPricing = crate::core::pricing::LiteLLMModelInfo;

/// Usage information
#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

/// Pricing database
#[derive(Debug, Clone)]
pub struct PricingDatabase {
    models: HashMap<String, ModelPricing>,
}

impl PricingDatabase {
    /// Load pricing data from JSON file
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read pricing file: {}", e))?;

        let models = crate::core::pricing::parse_litellm_pricing_json(&content)
            .map_err(|e| format!("Failed to parse pricing JSON: {}", e))?;

        Ok(Self { models })
    }

    /// Load from the same local pricing source used by gateway configuration.
    pub fn from_default_source() -> Result<Self, String> {
        let possible_paths = vec![
            DEFAULT_PRICING_SOURCE,
            "../config/model_prices_extended.json",
            "../../config/model_prices_extended.json",
            "../../../config/model_prices_extended.json",
        ];

        for path in &possible_paths {
            if Path::new(path).exists() {
                return Self::from_json_file(path);
            }
        }

        // Default
        Ok(Self::default())
    }

    /// Load pricing data from the default source.
    ///
    /// Kept as a compatibility wrapper for older call sites/tests.
    pub fn from_python_json() -> Result<Self, String> {
        Self::from_default_source()
    }

    /// Calculate cost
    pub fn calculate(&self, model: &str, usage: &Usage) -> f64 {
        // Direct model lookup
        if let Some(pricing) = self.models.get(model) {
            return self.calculate_with_pricing(pricing, usage);
        }

        // Handle
        for (key, pricing) in &self.models {
            if model.contains(key) || key.contains(model) {
                return self.calculate_with_pricing(pricing, usage);
            }
        }

        // Pricing information not found
        0.0
    }

    /// Calculate cost using specified pricing information from usage
    fn calculate_with_pricing(&self, pricing: &ModelPricing, usage: &Usage) -> f64 {
        let mut cost = 0.0;

        // Input token cost
        cost += usage.prompt_tokens as f64 * pricing.input_cost_per_token.unwrap_or(0.0);

        // Output token cost
        cost += usage.completion_tokens as f64 * pricing.output_cost_per_token.unwrap_or(0.0);

        // Reasoning token cost (if available)
        if let Some(reasoning_tokens) = usage.reasoning_tokens {
            cost += reasoning_tokens as f64 * extra_f64(pricing, "output_cost_per_reasoning_token");
        }

        cost
    }

    /// Model
    pub fn get_model_info(&self, model: &str) -> Option<&ModelPricing> {
        self.models.get(model)
    }

    /// Model
    pub fn get_max_tokens(&self, model: &str) -> Option<u32> {
        self.get_model_info(model).and_then(|info| {
            info.max_tokens
                .or(info.max_input_tokens)
                .or(info.max_output_tokens)
        })
    }

    /// Model
    pub fn get_provider_models(&self, provider: &str) -> Vec<String> {
        self.models
            .iter()
            .filter_map(|(model_id, pricing)| {
                if pricing.litellm_provider.to_lowercase() == provider.to_lowercase()
                    || model_id.to_lowercase().contains(&provider.to_lowercase())
                {
                    // If no explicit provider field, infer through model name
                    Some(model_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Create
    pub fn to_model_info(
        &self,
        model_id: &str,
        provider: &str,
    ) -> Option<crate::core::types::model::ModelInfo> {
        use crate::core::types::model::ModelInfo;
        use std::collections::HashMap;

        let pricing = self.get_model_info(model_id)?;

        Some(ModelInfo {
            id: model_id.to_string(),
            name: model_id.replace(['-', '_'], " "), // Simple name transformation
            provider: provider.to_string(),
            max_context_length: pricing
                .max_input_tokens
                .unwrap_or_else(|| pricing.max_tokens.unwrap_or(4096)),
            max_output_length: pricing.max_output_tokens,
            supports_streaming: true, // Most modern models support streaming
            supports_tools: pricing.supports_function_calling.unwrap_or(false),
            supports_multimodal: pricing.supports_vision.unwrap_or(false),
            input_cost_per_1k_tokens: pricing.input_cost_per_token.map(|cost| cost * 1000.0),
            output_cost_per_1k_tokens: pricing.output_cost_per_token.map(|cost| cost * 1000.0),
            currency: "USD".to_string(),
            capabilities: vec![], // Can be extended later
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        })
    }

    /// Check
    pub fn supports_feature(&self, model: &str, feature: &str) -> bool {
        self.get_model_info(model)
            .map(|info| match feature {
                "function_calling" => info.supports_function_calling.unwrap_or(false),
                "vision" => info.supports_vision.unwrap_or(false),
                _ => false,
            })
            .unwrap_or(false)
    }
}

fn extra_f64(pricing: &ModelPricing, key: &str) -> f64 {
    pricing
        .extra
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

fn builtin_model(
    provider: &str,
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    max_tokens: u32,
    max_output_tokens: u32,
    supports_function_calling: bool,
    supports_vision: bool,
) -> ModelPricing {
    ModelPricing {
        max_tokens: Some(max_tokens),
        max_input_tokens: Some(max_tokens),
        max_output_tokens: Some(max_output_tokens),
        input_cost_per_token: Some(input_cost_per_token),
        output_cost_per_token: Some(output_cost_per_token),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(supports_function_calling),
        supports_vision: Some(supports_vision),
        supports_streaming: Some(true),
        supports_parallel_function_calling: None,
        supports_system_message: Some(true),
        extra: HashMap::new(),
    }
}

impl Default for PricingDatabase {
    fn default() -> Self {
        // Built-in pricing for some common models as backup
        let mut models = HashMap::new();

        // OpenAI models
        models.insert(
            "gpt-4".to_string(),
            builtin_model("openai", 0.00003, 0.00006, 8192, 4096, true, false),
        );

        models.insert(
            "gpt-4-turbo".to_string(),
            builtin_model("openai", 0.00001, 0.00003, 128000, 4096, true, true),
        );

        models.insert(
            "gpt-3.5-turbo".to_string(),
            builtin_model("openai", 0.0000005, 0.0000015, 16385, 4096, true, false),
        );

        // Anthropic models
        models.insert(
            "claude-3-opus".to_string(),
            builtin_model("anthropic", 0.000015, 0.000075, 200000, 4096, true, true),
        );

        models.insert(
            "claude-3-sonnet".to_string(),
            builtin_model("anthropic", 0.000003, 0.000015, 200000, 4096, true, true),
        );

        // DeepSeek models - updated pricing
        models.insert(
            "deepseek-chat".to_string(),
            builtin_model(
                "deepseek", 0.00000056, 0.00000168, 128000, 8192, true, false,
            ),
        );

        models.insert(
            "deepseek-reasoner".to_string(),
            builtin_model(
                "deepseek", 0.00000056, 0.00000168, 128000, 8192, true, false,
            ),
        );

        Self { models }
    }
}

// Global pricing database (lazy loading)
pub static GLOBAL_PRICING_DB: LazyLock<PricingDatabase> = LazyLock::new(|| {
    PricingDatabase::from_python_json().unwrap_or_else(|e| {
        warn!(
            error = %e,
            "Failed to load pricing data from file, using built-in defaults"
        );
        PricingDatabase::default()
    })
});

/// Get
pub fn get_pricing_db() -> &'static PricingDatabase {
    &GLOBAL_PRICING_DB
}

/// Quick cost calculation
pub fn calculate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    let usage = Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
        reasoning_tokens: None,
    };

    GLOBAL_PRICING_DB.calculate(model, &usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_pricing() {
        let db = PricingDatabase::default();

        let usage = Usage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
            reasoning_tokens: None,
        };

        // Test GPT-4 pricing
        let cost = db.calculate("gpt-4", &usage);
        assert!(cost > 0.0);
        assert_eq!(cost, 1000.0 * 0.00003 + 500.0 * 0.00006);

        // Test Claude pricing
        let cost = db.calculate("claude-3-opus", &usage);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_model_info() {
        let db = PricingDatabase::default();

        assert!(db.get_model_info("gpt-4").is_some());
        assert!(db.get_model_info("non-existent-model").is_none());

        assert_eq!(db.get_max_tokens("gpt-4"), Some(8192));
        assert!(db.supports_feature("gpt-4", "function_calling"));
        assert!(!db.supports_feature("gpt-4", "vision"));
        assert!(db.supports_feature("gpt-4-turbo", "vision"));
    }

    #[test]
    fn test_quick_calculate() {
        let cost = calculate_cost("gpt-3.5-turbo", 1000, 500);
        assert!(cost > 0.0);
    }

    #[test]
    fn test_default_source_loads_shared_pricing_file() {
        let db = PricingDatabase::from_default_source().unwrap();

        assert!(db.get_model_info("gpt-4o").is_some());
        assert!(
            db.calculate(
                "gpt-4o",
                &Usage {
                    prompt_tokens: 1000,
                    completion_tokens: 500,
                    total_tokens: 1500,
                    reasoning_tokens: None,
                }
            ) > 0.0
        );
    }
}
