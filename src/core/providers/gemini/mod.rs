//! Google Gemini Provider
//!
//! Support for Google AI Studio and Vertex AI Gemini model series
//!
//! # Supported Models
//! - Gemini 3.5 Flash (latest)
//! - Gemini 3.1 Flash / Pro Preview
//! - Gemini 2.5 Pro / Flash
//! - Gemini 2.0 Flash
//! - Gemini 1.5 Pro
//! - Gemini 1.5 Flash
//! - Gemini 1.0 Pro
//!
//! # Features
//! - Multimodal support (text, images, videos, audio)
//! - Tool calling and function calling
//! - Context caching
//! - Batch processing
//! - Real-time streaming responses

use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::error::gateway_error::GatewayError;

pub mod client;
pub mod config;
pub mod error;
pub mod models;
pub mod provider;
pub mod streaming;

// Re-export main types
pub use client::GeminiClient;
pub use config::GeminiConfig;
pub use error::GeminiError;
pub use models::{GeminiModelFamily, ModelFeature, get_gemini_registry};
pub use provider::GeminiProvider;
pub use streaming::GeminiStream;

pub(crate) fn calculate_gemini_cost(
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Result<f64, ProviderError> {
    PricingService::shared_embedded_default()
        .and_then(|service| {
            service.calculate_loaded_usage_cost_for_provider(
                "gemini",
                model,
                &PricingUsage::new(input_tokens, output_tokens),
            )
        })
        .map(|cost| cost.total_cost)
        .map_err(|error| gemini_pricing_error(model, error))
}

fn gemini_pricing_error(model: &str, error: GatewayError) -> ProviderError {
    match error {
        GatewayError::NotFound(_) => ProviderError::model_not_found("gemini", model),
        error => ProviderError::Other {
            provider: "gemini",
            message: format!("pricing authority failed for model '{model}': {error}"),
        },
    }
}

// Convenience functions

/// Create Gemini provider
pub fn create_gemini_provider(config: GeminiConfig) -> Result<GeminiProvider, error::GeminiError> {
    GeminiProvider::new(config)
}

/// Create Gemini provider from environment
pub fn create_gemini_provider_from_env() -> Result<GeminiProvider, error::GeminiError> {
    let config = GeminiConfig::from_env()?;
    GeminiProvider::new(config)
}

/// Get supported models
pub fn supported_models() -> Vec<String> {
    get_gemini_registry()
        .list_models()
        .into_iter()
        .map(|spec| spec.model_info.id.clone())
        .collect()
}

/// Check if model is supported
pub fn is_model_supported(model_id: &str) -> bool {
    get_gemini_registry().get_model_spec(model_id).is_some()
}

/// Get model pricing
pub fn get_model_pricing(model_id: &str) -> Result<(f64, f64), ProviderError> {
    let (_, pricing) = PricingService::shared_embedded_default()
        .map_err(|error| gemini_pricing_error(model_id, error))?
        .get_model_info_for_provider("gemini", model_id)
        .ok_or_else(|| ProviderError::model_not_found("gemini", model_id))?;
    let input = pricing
        .input_cost_per_token
        .ok_or_else(|| ProviderError::Other {
            provider: "gemini",
            message: format!("missing input token pricing for model '{model_id}'"),
        })?;
    let output = pricing
        .output_cost_per_token
        .ok_or_else(|| ProviderError::Other {
            provider: "gemini",
            message: format!("missing output token pricing for model '{model_id}'"),
        })?;
    Ok((input * 1_000_000.0, output * 1_000_000.0))
}

#[cfg(test)]
mod pricing_tests {
    use super::*;

    #[test]
    fn public_pricing_uses_per_million_catalog_units() {
        let (input, output) =
            get_model_pricing("gemini-1.5-flash").expect("catalogued model should be priced");
        assert!((input - 0.075).abs() < 1e-12);
        assert!((output - 0.30).abs() < 1e-12);
        assert!(matches!(
            get_model_pricing("unknown-google-model"),
            Err(ProviderError::ModelNotFound { .. })
        ));
    }
}
