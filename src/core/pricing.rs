//! Shared pricing data types.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;
use tracing::warn;

const EMBEDDED_MODEL_PRICES: &str = include_str!("../../config/model_prices_extended.json");

/// LiteLLM-compatible model pricing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteLLMModelInfo {
    /// Maximum total tokens
    #[serde(default, deserialize_with = "deserialize_option_u32_integral_number")]
    pub max_tokens: Option<u32>,
    /// Maximum input tokens
    #[serde(default, deserialize_with = "deserialize_option_u32_integral_number")]
    pub max_input_tokens: Option<u32>,
    /// Maximum output tokens
    #[serde(default, deserialize_with = "deserialize_option_u32_integral_number")]
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
    #[serde(default)]
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

fn deserialize_option_u32_integral_number<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<serde_json::Value>::deserialize(deserializer)? else {
        return Ok(None);
    };

    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(number) => {
            if let Some(value) = number.as_u64() {
                return u32::try_from(value)
                    .map(Some)
                    .map_err(|_| serde::de::Error::custom("token limit exceeds u32::MAX"));
            }

            if let Some(value) = number.as_i64()
                && value < 0
            {
                return Err(serde::de::Error::custom("token limit cannot be negative"));
            }

            let value = number
                .as_f64()
                .ok_or_else(|| serde::de::Error::custom("token limit must be a finite number"))?;
            if !value.is_finite() {
                return Err(serde::de::Error::custom(
                    "token limit must be a finite number",
                ));
            }
            if value < 0.0 {
                return Err(serde::de::Error::custom("token limit cannot be negative"));
            }
            if value.fract() != 0.0 {
                return Err(serde::de::Error::custom(
                    "token limit float must be integral",
                ));
            }
            if value > u32::MAX as f64 {
                return Err(serde::de::Error::custom("token limit exceeds u32::MAX"));
            }

            Ok(Some(value as u32))
        }
        _ => Err(serde::de::Error::custom(
            "token limit must be a JSON number",
        )),
    }
}

/// Compatibility alias for callers that still import provider-base pricing.
pub type ModelPricing = LiteLLMModelInfo;

pub(crate) type PricingModelMap = HashMap<String, ModelPricing>;

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

    /// Load from the bundled default pricing source used by gateway configuration.
    pub fn from_default_source() -> Result<Self, String> {
        let models = embedded_default_pricing_models()
            .map_err(|e| format!("Failed to parse embedded pricing JSON: {}", e))?;

        Ok(Self { models })
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

        if requires_bidirectional_database_token_pricing(pricing)
            && (pricing.input_cost_per_token.is_none() || pricing.output_cost_per_token.is_none())
        {
            warn!(
                "model pricing row for provider '{}' mode '{}' is missing one side of token pricing; skipping partial billing",
                pricing.litellm_provider, pricing.mode
            );
            return 0.0;
        }

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
        let provider = normalize_pricing_provider(provider);
        self.models
            .iter()
            .filter_map(|(model_id, pricing)| {
                let pricing_provider = normalize_pricing_provider(&pricing.litellm_provider);
                if pricing_provider == provider {
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
            supports_tools: pricing_supports_tools(pricing),
            supports_multimodal: pricing_supports_multimodal(pricing),
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

fn pricing_supports_multimodal(pricing: &ModelPricing) -> bool {
    pricing.supports_vision.unwrap_or(false)
        || pricing
            .extra
            .get("supported_modalities")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|modalities| {
                modalities.iter().any(|modality| {
                    modality
                        .as_str()
                        .is_some_and(|value| matches!(value, "image" | "video"))
                })
            })
        || pricing
            .extra
            .get("supports_video_input")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
}

fn pricing_supports_tools(pricing: &ModelPricing) -> bool {
    pricing.supports_function_calling.unwrap_or(false)
        || pricing
            .extra
            .get("supports_tool_choice")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
}

fn requires_bidirectional_database_token_pricing(pricing: &ModelPricing) -> bool {
    matches!(pricing.mode.as_str(), "chat" | "completion")
        || (pricing.mode.is_empty() && !has_non_token_database_pricing(pricing))
}

fn has_non_token_database_pricing(pricing: &ModelPricing) -> bool {
    pricing.cost_per_second.is_some()
        || pricing
            .extra
            .get("video_cost_per_second")
            .and_then(serde_json::Value::as_f64)
            .is_some()
        || pricing
            .extra
            .get("audio_cost_per_second")
            .and_then(serde_json::Value::as_f64)
            .is_some()
        || pricing
            .extra
            .get("image_cost_per_token")
            .and_then(serde_json::Value::as_f64)
            .is_some()
        || pricing
            .extra
            .get("output_cost_per_image")
            .and_then(serde_json::Value::as_f64)
            .is_some()
}

fn pricing_matches_provider(_model_key: &str, pricing: &ModelPricing, provider: &str) -> bool {
    let provider = normalize_pricing_provider(provider);
    let pricing_provider = normalize_pricing_provider(&pricing.litellm_provider);

    pricing_provider == provider
}

pub(crate) fn normalize_pricing_provider(provider: &str) -> String {
    match provider.to_ascii_lowercase().replace('-', "_").as_str() {
        "vertexai" | "google" => "vertex_ai".to_string(),
        "zhipu" | "glm" => "zhipuai".to_string(),
        "mimo" | "xiaomi" => "xiaomi_mimo".to_string(),
        "together" | "togetherai" => "together_ai".to_string(),
        "fireworks" | "fireworksai" => "fireworks_ai".to_string(),
        "aiml_api" | "aimlapi" => "aiml".to_string(),
        other => other.to_string(),
    }
}

fn model_matches_key(model: &str, key: &str) -> bool {
    fn model_id_matches(candidate: &str, requested: &str) -> bool {
        if candidate == requested {
            return true;
        }

        candidate
            .strip_prefix(requested)
            .and_then(|suffix| suffix.strip_prefix('-'))
            .is_some_and(alias_suffix_matches)
    }

    model_id_matches(key, model)
        || model_id_matches(model, key)
        || key
            .rsplit_once('/')
            .map(|(_, model_id)| {
                model_id_matches(model_id, model) || model_id_matches(model, model_id)
            })
            .unwrap_or(false)
}

fn alias_suffix_matches(suffix: &str) -> bool {
    if suffix == "latest" {
        return true;
    }

    let digit_prefix_len = suffix.chars().take_while(|ch| ch.is_ascii_digit()).count();
    digit_prefix_len >= 4
        && suffix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub(crate) fn normalize_model_key(model: &str) -> &str {
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

fn builtin_deepseek_v4_model(
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_token_cost: f64,
) -> ModelPricing {
    let mut model = builtin_model(
        "deepseek",
        input_cost_per_token,
        output_cost_per_token,
        1_048_576,
        393_216,
        true,
        false,
    );
    model.extra.insert(
        "cache_read_input_token_cost".to_string(),
        serde_json::Value::from(cache_read_input_token_cost),
    );
    model.extra.insert(
        "pricing_status".to_string(),
        serde_json::Value::from("official_off_peak_rate_checked_2026_08_24"),
    );
    model
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

        for model in ["deepseek-v4-flash", "deepseek-chat", "deepseek-reasoner"] {
            models.insert(
                model.to_string(),
                builtin_deepseek_v4_model(0.00000022, 0.00000066, 0.000000007),
            );
        }

        models.insert(
            "deepseek-v4-pro".to_string(),
            builtin_deepseek_v4_model(0.00000066, 0.00000198, 0.000000022),
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

pub(crate) fn embedded_default_pricing_models() -> serde_json::Result<PricingModelMap> {
    parse_litellm_pricing_json(EMBEDDED_MODEL_PRICES)
}

pub fn is_litellm_pricing_metadata_key(key: &str) -> bool {
    key == "sample_spec" || key.starts_with('_') || key.contains("example")
}

#[cfg(test)]
mod tests;
