//! Shared pricing data types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use tracing::warn;

const DEFAULT_PRICING_SOURCE: &str = "config/model_prices_extended.json";

/// LiteLLM-compatible model pricing data.
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

/// Compatibility alias for callers that still import provider-base pricing.
pub type ModelPricing = LiteLLMModelInfo;

/// Usage information for simple pricing database calculations.
///
/// Kept as a stable public type so downstream library consumers can continue
/// to construct `litellm_rs::core::pricing::Usage { prompt_tokens, ..., reasoning_tokens }`
/// with struct-literal syntax. The richer
/// [`crate::core::types::responses::Usage`] (with nested
/// `PromptTokensDetails` / `CompletionTokensDetails` / `ThinkingUsage`) is
/// the canonical shape used elsewhere; conversion helpers below bridge
/// between the two when the per-request cost path needs the canonical
/// metadata. See PR #519 architecture roadmap for the broader convergence.
#[derive(Debug, Clone)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub reasoning_tokens: Option<u32>,
}

impl Usage {
    /// Build a Usage from prompt + completion token counts.
    ///
    /// Auto-computes `total_tokens = prompt + completion` and leaves
    /// `reasoning_tokens` unset. Mirrors the canonical
    /// [`crate::core::types::responses::Usage::new`] signature so internal
    /// pricing call sites can use the same constructor on either type.
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            reasoning_tokens: None,
        }
    }
}

impl From<&crate::core::types::responses::Usage> for Usage {
    fn from(usage: &crate::core::types::responses::Usage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            reasoning_tokens: usage
                .completion_tokens_details
                .as_ref()
                .and_then(|d| d.reasoning_tokens),
        }
    }
}

/// Pricing database backed by the shared LiteLLM model info shape.
#[derive(Debug, Clone)]
pub struct PricingDatabase {
    models: HashMap<String, ModelPricing>,
}

impl PricingDatabase {
    /// Load pricing data from JSON file.
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read pricing file: {}", e))?;

        let models = parse_litellm_pricing_json(&content)
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

        Ok(Self::default())
    }

    /// Load pricing data from the default source.
    ///
    /// Kept as a compatibility wrapper for older call sites/tests.
    pub fn from_python_json() -> Result<Self, String> {
        Self::from_default_source()
    }

    /// Calculate cost for a model and token usage.
    pub fn calculate(&self, model: &str, usage: &Usage) -> f64 {
        if let Some(pricing) = self.models.get(model) {
            return self.calculate_with_pricing(pricing, usage);
        }

        let normalized_model = normalize_model_key(model);
        if normalized_model != model
            && let Some(pricing) = self.models.get(normalized_model)
        {
            return self.calculate_with_pricing(pricing, usage);
        }

        if let Some((_, pricing)) = self
            .models
            .iter()
            .filter(|(key, _)| model_matches_key(normalized_model, key))
            .max_by_key(|(key, _)| key.len())
        {
            return self.calculate_with_pricing(pricing, usage);
        }

        0.0
    }

    /// Calculate cost for a provider/model pair and token usage.
    ///
    /// Provider dispatch should use this method instead of `calculate` so a
    /// same-named model on another provider does not accidentally supply prices.
    pub fn calculate_for_provider(&self, provider: &str, model: &str, usage: &Usage) -> f64 {
        if let Some(pricing) = self.models.get(model)
            && pricing_matches_provider(model, pricing, provider)
        {
            return self.calculate_with_pricing(pricing, usage);
        }

        let normalized_model = normalize_model_key(model);
        if normalized_model != model
            && let Some(pricing) = self.models.get(normalized_model)
            && pricing_matches_provider(normalized_model, pricing, provider)
        {
            return self.calculate_with_pricing(pricing, usage);
        }

        if let Some((_, pricing)) = self
            .models
            .iter()
            .filter(|(key, pricing)| {
                pricing_matches_provider(key, pricing, provider)
                    && model_matches_key(normalized_model, key)
            })
            .max_by_key(|(key, _)| key.len())
        {
            return self.calculate_with_pricing(pricing, usage);
        }

        0.0
    }

    fn calculate_with_pricing(&self, pricing: &ModelPricing, usage: &Usage) -> f64 {
        let mut cost = 0.0;

        let input_cost_per_token = tiered_cost_per_token(
            pricing,
            pricing.input_cost_per_token.unwrap_or(0.0),
            "input_cost_per_token_above_",
            usage.prompt_tokens,
        );
        let output_cost_per_token = tiered_cost_per_token(
            pricing,
            pricing.output_cost_per_token.unwrap_or(0.0),
            "output_cost_per_token_above_",
            usage.prompt_tokens,
        );

        cost += usage.prompt_tokens as f64 * input_cost_per_token;
        cost += usage.completion_tokens as f64 * output_cost_per_token;

        if let Some(reasoning_tokens) = usage.reasoning_tokens {
            cost += reasoning_tokens as f64 * extra_f64(pricing, "output_cost_per_reasoning_token");
        }

        cost
    }

    /// Get raw LiteLLM model information for a model.
    pub fn get_model_info(&self, model: &str) -> Option<&ModelPricing> {
        self.models.get(model)
    }

    /// Get the configured max token limit for a model.
    pub fn get_max_tokens(&self, model: &str) -> Option<u32> {
        self.get_model_info(model).and_then(|info| {
            info.max_tokens
                .or(info.max_input_tokens)
                .or(info.max_output_tokens)
        })
    }

    /// Get all models associated with a provider.
    pub fn get_provider_models(&self, provider: &str) -> Vec<String> {
        self.models
            .iter()
            .filter_map(|(model_id, pricing)| {
                if pricing.litellm_provider.to_lowercase() == provider.to_lowercase()
                    || model_id.to_lowercase().contains(&provider.to_lowercase())
                {
                    Some(model_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Convert shared pricing metadata into public model metadata.
    pub fn to_model_info(
        &self,
        model_id: &str,
        provider: &str,
    ) -> Option<crate::core::types::model::ModelInfo> {
        use crate::core::types::model::ModelInfo;

        let pricing = self.get_model_info(model_id)?;

        Some(ModelInfo {
            id: model_id.to_string(),
            name: model_id.replace(['-', '_'], " "),
            provider: provider.to_string(),
            max_context_length: pricing
                .max_input_tokens
                .unwrap_or_else(|| pricing.max_tokens.unwrap_or(4096)),
            max_output_length: pricing.max_output_tokens,
            supports_streaming: pricing.supports_streaming.unwrap_or(true),
            supports_tools: pricing.supports_function_calling.unwrap_or(false),
            supports_multimodal: pricing.supports_vision.unwrap_or(false),
            input_cost_per_1k_tokens: pricing.input_cost_per_token.map(price_per_token_to_per_1k),
            output_cost_per_1k_tokens: pricing.output_cost_per_token.map(price_per_token_to_per_1k),
            currency: "USD".to_string(),
            capabilities: vec![],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        })
    }

    /// Check whether a model supports a feature.
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

fn pricing_matches_provider(model_key: &str, pricing: &ModelPricing, provider: &str) -> bool {
    let provider = provider.to_ascii_lowercase();
    let pricing_provider = pricing.litellm_provider.to_ascii_lowercase();
    let model_key = model_key.to_ascii_lowercase();

    pricing_provider == provider || model_key.starts_with(&format!("{provider}/"))
}

fn model_matches_key(model: &str, key: &str) -> bool {
    model == key || model.contains(key) || key.contains(model)
}

fn normalize_model_key(model: &str) -> &str {
    model
        .rsplit_once('/')
        .map(|(_, model)| model)
        .unwrap_or(model)
}

fn extra_f64(pricing: &ModelPricing, key: &str) -> f64 {
    pricing
        .extra
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

fn tiered_cost_per_token(
    pricing: &ModelPricing,
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

fn extract_tier_threshold(key: &str) -> Option<u32> {
    let threshold = key.split("_above_").nth(1)?.split("_tokens").next()?;
    if let Some(number) = threshold.strip_suffix('k') {
        number.parse::<u32>().ok().map(|value| value * 1000)
    } else {
        threshold.parse::<u32>().ok()
    }
}

fn price_per_token_to_per_1k(cost_per_token: f64) -> f64 {
    let cost_per_1k = cost_per_token * 1000.0;
    (cost_per_1k * 1_000_000_000_000.0).round() / 1_000_000_000_000.0
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

fn builtin_gpt55_model(snapshot: bool) -> ModelPricing {
    let mut model = builtin_model("openai", 0.000005, 0.00003, 1_048_576, 128_000, true, true);
    model.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::Value::from(0.0000005),
    );
    model.extra.insert(
        "input_cost_per_token_above_272k_tokens".to_string(),
        serde_json::Value::from(0.00001),
    );
    model.extra.insert(
        "output_cost_per_token_above_272k_tokens".to_string(),
        serde_json::Value::from(0.000045),
    );
    model.extra.insert(
        "cache_read_input_token_cost_above_272k_tokens".to_string(),
        serde_json::Value::from(0.000001),
    );
    if snapshot {
        model
            .extra
            .insert("snapshot".to_string(), serde_json::Value::from(true));
    }
    model
}

fn builtin_gpt55_pro_model(snapshot: bool) -> ModelPricing {
    let mut model = builtin_model("openai", 0.00003, 0.00018, 1_048_576, 128_000, true, true);
    model.supports_streaming = Some(false);
    model.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::Value::from(0.00003),
    );
    if snapshot {
        model
            .extra
            .insert("snapshot".to_string(), serde_json::Value::from(true));
    }
    model
}

impl Default for PricingDatabase {
    fn default() -> Self {
        let mut models = HashMap::new();

        models.insert("gpt-5.5".to_string(), builtin_gpt55_model(false));

        models.insert("gpt-5.5-2026-04-23".to_string(), builtin_gpt55_model(true));

        models.insert("gpt-5.5-pro".to_string(), builtin_gpt55_pro_model(false));

        models.insert(
            "gpt-5.5-pro-2026-04-23".to_string(),
            builtin_gpt55_pro_model(true),
        );

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

        models.insert(
            "claude-3-opus".to_string(),
            builtin_model("anthropic", 0.000015, 0.000075, 200000, 4096, true, true),
        );

        models.insert(
            "claude-3-sonnet".to_string(),
            builtin_model("anthropic", 0.000003, 0.000015, 200000, 4096, true, true),
        );

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

/// Global pricing database loaded from the canonical local pricing source.
pub static GLOBAL_PRICING_DB: LazyLock<PricingDatabase> = LazyLock::new(|| {
    PricingDatabase::from_python_json().unwrap_or_else(|e| {
        warn!(
            error = %e,
            "Failed to load pricing data from file, using built-in defaults"
        );
        PricingDatabase::default()
    })
});

/// Get the shared global pricing database.
pub fn get_pricing_db() -> &'static PricingDatabase {
    &GLOBAL_PRICING_DB
}

/// Quick cost calculation for compatibility callers.
pub fn calculate_cost(model: &str, prompt_tokens: u32, completion_tokens: u32) -> f64 {
    let usage = Usage::new(prompt_tokens, completion_tokens);
    GLOBAL_PRICING_DB.calculate(model, &usage)
}

/// Parse LiteLLM pricing JSON into the shared pricing model map.
///
/// LiteLLM pricing files can contain documentation/sample keys next to model
/// entries. Every runtime pricing entry point should apply the same filtering
/// so gateway cost calculations and the pricing service see the same dataset.
pub fn parse_litellm_pricing_json(
    content: &str,
) -> Result<HashMap<String, LiteLLMModelInfo>, serde_json::Error> {
    let all_data: HashMap<String, serde_json::Value> = serde_json::from_str(content)?;
    all_data
        .into_iter()
        .filter(|(key, _)| !is_litellm_pricing_metadata_key(key))
        .map(|(key, value)| serde_json::from_value(value).map(|pricing| (key, pricing)))
        .collect()
}

pub fn is_litellm_pricing_metadata_key(key: &str) -> bool {
    key == "sample_spec" || key.starts_with('_') || key.contains("example")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn parse_litellm_pricing_json_filters_metadata_entries() {
        let content = r#"{
            "sample_spec": {"this": "is not a model"},
            "_comment": {"ignored": true},
            "provider_example_model": {"ignored": true},
            "gpt-test": {
                "max_tokens": 4096,
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "litellm_provider": "openai",
                "mode": "chat"
            }
        }"#;

        let parsed = parse_litellm_pricing_json(content).unwrap();

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed["gpt-test"].litellm_provider, "openai");
        assert_eq!(parsed["gpt-test"].input_cost_per_token, Some(0.000001));
    }

    #[test]
    fn parse_litellm_pricing_json_rejects_malformed_model_entries() {
        let content = r#"{
            "bad-model": {
                "input_cost_per_token": 0.000001,
                "output_cost_per_token": 0.000002,
                "mode": "chat"
            }
        }"#;

        assert!(parse_litellm_pricing_json(content).is_err());
    }

    #[test]
    fn test_default_pricing() {
        let db = PricingDatabase::default();

        let usage = Usage::new(1000, 500);

        let cost = db.calculate("gpt-4", &usage);
        assert!(cost > 0.0);
        assert_eq!(cost, 1000.0 * 0.00003 + 500.0 * 0.00006);

        let cost = db.calculate("claude-3-opus", &usage);
        assert!(cost > 0.0);
    }

    #[test]
    fn calculate_for_provider_uses_matching_provider_rates() {
        let db = PricingDatabase::default();
        let usage = Usage::new(1000, 500);

        assert_eq!(
            db.calculate_for_provider("openai", "gpt-4", &usage),
            1000.0 * 0.00003 + 500.0 * 0.00006
        );
        assert_eq!(db.calculate_for_provider("anthropic", "gpt-4", &usage), 0.0);
        assert_eq!(
            db.calculate_for_provider("openai", "claude-3-opus", &usage),
            0.0
        );
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
    fn gpt55_shared_pricing_charges_long_context_tiers() {
        let db = PricingDatabase::default();
        let usage = Usage::new(300_000, 2_000);

        assert!((db.calculate("gpt-5.5", &usage) - 3.09).abs() < 1e-12);
        assert!((db.calculate_for_provider("openai", "gpt-5.5", &usage) - 3.09).abs() < 1e-12);

        let Ok(shared_db) = PricingDatabase::from_default_source() else {
            panic!("shared pricing source should load");
        };
        assert!((shared_db.calculate("gpt-5.5", &usage) - 3.09).abs() < 1e-12);
        assert!((calculate_cost("gpt-5.5", 300_000, 2_000) - 3.09).abs() < 1e-12);
    }

    #[test]
    fn gpt55_provider_prefixed_pro_pricing_uses_exact_model() {
        let usage = Usage::new(1_000, 500);
        let expected_pro_cost = 1_000.0 * 0.00003 + 500.0 * 0.00018;

        let db = PricingDatabase::default();
        assert!((db.calculate("openai/gpt-5.5-pro", &usage) - expected_pro_cost).abs() < 1e-12);
        assert!(
            (db.calculate_for_provider("openai", "openai/gpt-5.5-pro", &usage) - expected_pro_cost)
                .abs()
                < 1e-12
        );

        let Ok(shared_db) = PricingDatabase::from_default_source() else {
            panic!("shared pricing source should load");
        };
        assert!(
            (shared_db.calculate("openai/gpt-5.5-pro", &usage) - expected_pro_cost).abs() < 1e-12
        );
        assert!(
            (shared_db.calculate_for_provider("openai", "openai/gpt-5.5-pro", &usage)
                - expected_pro_cost)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn provider_prefixed_exact_prices_are_preserved_before_normalization() {
        let usage = Usage::new(1_000, 1_000);
        let mut db = PricingDatabase::default();
        let azure_override = builtin_model("azure", 0.000001, 0.000002, 8_192, 4_096, true, false);
        db.models.insert("azure/gpt-4".to_string(), azure_override);

        let expected_azure_cost = 1_000.0 * 0.000001 + 1_000.0 * 0.000002;

        assert!((db.calculate("azure/gpt-4", &usage) - expected_azure_cost).abs() < 1e-12);
        assert!(
            (db.calculate_for_provider("azure", "azure/gpt-4", &usage) - expected_azure_cost).abs()
                < 1e-12
        );
    }

    #[test]
    fn gpt55_builtin_pro_model_info_is_non_streaming() {
        let db = PricingDatabase::default();
        let Some(info) = db.to_model_info("gpt-5.5-pro", "openai") else {
            panic!("built-in GPT-5.5 Pro pricing should be present");
        };

        assert!(!info.supports_streaming);
    }

    #[test]
    fn test_default_source_loads_shared_pricing_file() {
        let db = PricingDatabase::from_default_source().unwrap();

        assert!(db.get_model_info("gpt-4o").is_some());
        assert!(db.calculate("gpt-4o", &Usage::new(1000, 500)) > 0.0);
    }

    #[test]
    fn provider_code_uses_core_pricing_directly() {
        let providers_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers");
        let mut rust_files = Vec::new();
        collect_rust_files(&providers_dir, &mut rust_files);

        for path in rust_files {
            if is_pricing_compatibility_module(&path) {
                continue;
            }

            let content = fs::read_to_string(&path).unwrap_or_else(|err| {
                panic!("failed to read provider source {}: {}", path.display(), err)
            });

            for forbidden in [
                "providers::base::pricing",
                "providers::base::get_pricing_db",
                "providers::base::PricingDatabase",
                "providers::base::{get_pricing_db",
                "providers::base::{PricingDatabase",
            ] {
                assert!(
                    !content.contains(forbidden),
                    "{} should import pricing database APIs from core::pricing, not {}",
                    path.display(),
                    forbidden
                );
            }
        }
    }

    fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("provider source directory should be readable") {
            let entry = entry.expect("provider source entry should be readable");
            let path = entry.path();
            let file_type = entry
                .file_type()
                .expect("provider source file type should be readable");

            if file_type.is_dir() {
                collect_rust_files(&path, files);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                files.push(path);
            }
        }
    }

    fn is_pricing_compatibility_module(path: &Path) -> bool {
        path.ends_with("src/core/providers/base/pricing.rs")
            || path.ends_with("src/core/providers/base/mod.rs")
    }
}
