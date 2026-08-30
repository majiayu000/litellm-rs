mod gemini25;
mod gemini3;
mod gemini31;
mod gemini35;
mod gemini36;
mod gemini37;
mod legacy;

use std::collections::HashMap;

use serde_json::Value;

use super::GeminiModelRegistry;
use crate::core::types::model::ProviderCapability;

pub(super) fn promotional_flash_pricing_metadata() -> HashMap<String, Value> {
    HashMap::from([
        (
            "google_promotional_pricing_through".to_string(),
            serde_json::json!("2026-12-31"),
        ),
        (
            "google_standard_pricing_from".to_string(),
            serde_json::json!("2027-01-01"),
        ),
        (
            "google_standard_input_cost_per_million".to_string(),
            serde_json::json!(1.5),
        ),
        (
            "google_standard_output_cost_per_million".to_string(),
            serde_json::json!(7.5),
        ),
        (
            "google_standard_cache_read_cost_per_million".to_string(),
            serde_json::json!(0.15),
        ),
        (
            "google_current_cache_storage_cost_per_million_token_hour".to_string(),
            serde_json::json!(0.5),
        ),
        (
            "google_standard_cache_storage_cost_per_million_token_hour".to_string(),
            serde_json::json!(1.0),
        ),
        (
            "google_pricing_source".to_string(),
            serde_json::json!("https://ai.google.dev/gemini-api/docs/pricing"),
        ),
    ])
}

pub(super) fn advanced_text_capabilities() -> Vec<ProviderCapability> {
    vec![
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
        ProviderCapability::FunctionCalling,
        ProviderCapability::CodeExecution,
        ProviderCapability::BatchProcessing,
    ]
}

pub(super) fn function_batch_capabilities() -> Vec<ProviderCapability> {
    vec![
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
        ProviderCapability::FunctionCalling,
        ProviderCapability::BatchProcessing,
    ]
}

pub(super) fn register_all(registry: &mut GeminiModelRegistry) {
    gemini37::register(registry);
    gemini36::register(registry);
    gemini35::register(registry);
    gemini31::register(registry);
    gemini3::register(registry);
    gemini25::register(registry);
    legacy::register(registry);
}
