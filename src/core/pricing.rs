//! Shared pricing data types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}
