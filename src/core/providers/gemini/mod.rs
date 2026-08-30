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

use crate::core::pricing_service::{LiteLLMModelInfo, PricingService, PricingUsage};
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::error::gateway_error::GatewayError;

#[cfg(feature = "providers-extended")]
pub mod client;
#[cfg(feature = "providers-extended")]
pub mod config;
#[cfg(feature = "providers-extended")]
pub mod error;
pub mod models;
#[cfg(feature = "providers-extended")]
pub mod provider;
#[cfg(feature = "providers-extended")]
pub mod streaming;

// Re-export main types
#[cfg(feature = "providers-extended")]
pub use client::GeminiClient;
#[cfg(feature = "providers-extended")]
pub use config::GeminiConfig;
#[cfg(feature = "providers-extended")]
pub use error::GeminiError;
pub use models::{GeminiModelFamily, GoogleGeminiApiSurface, ModelFeature, get_gemini_registry};
#[cfg(feature = "providers-extended")]
pub use provider::GeminiProvider;
#[cfg(feature = "providers-extended")]
pub use streaming::GeminiStream;

pub(crate) fn calculate_gemini_cost(
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Result<f64, ProviderError> {
    let (service, _) = gemini_completion_pricing(model)?;
    service
        .calculate_loaded_usage_cost_for_provider(
            "gemini",
            model,
            &PricingUsage::new(input_tokens, output_tokens),
        )
        .map(|cost| cost.total_cost)
        .map_err(|error| gemini_pricing_error(model, error))
}

fn gemini_completion_pricing(
    model: &str,
) -> Result<(&'static PricingService, LiteLLMModelInfo), ProviderError> {
    let service = PricingService::shared_embedded_default()
        .map_err(|error| gemini_pricing_error(model, error))?;
    let (_, pricing) = service
        .get_model_info_for_provider("gemini", model)
        .ok_or_else(|| ProviderError::model_not_found("gemini", model))?;
    if pricing.mode == "embedding" {
        return Err(ProviderError::model_not_found("gemini", model));
    }
    Ok((service, pricing))
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
#[cfg(feature = "providers-extended")]
pub fn create_gemini_provider(config: GeminiConfig) -> Result<GeminiProvider, error::GeminiError> {
    GeminiProvider::new(config)
}

/// Create Gemini provider from environment
#[cfg(feature = "providers-extended")]
pub fn create_gemini_provider_from_env() -> Result<GeminiProvider, error::GeminiError> {
    let config = GeminiConfig::from_env()?;
    GeminiProvider::new(config)
}

/// Get supported models
pub fn supported_models() -> Vec<String> {
    get_gemini_registry()
        .list_model_infos_for_surface(GoogleGeminiApiSurface::DeveloperApi)
        .into_iter()
        .map(|model| model.id)
        .collect()
}

/// Check if model is supported
pub fn is_model_supported(model_id: &str) -> bool {
    get_gemini_registry()
        .get_model_spec(model_id)
        .is_some_and(|spec| GoogleGeminiApiSurface::DeveloperApi.includes(spec))
}

/// Get model pricing
pub fn get_model_pricing(model_id: &str) -> Result<(f64, f64), ProviderError> {
    let (_, pricing) = gemini_completion_pricing(model_id)?;
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
            get_model_pricing("gemini-2.5-flash").expect("catalogued model should be priced");
        assert!((input - 0.30).abs() < 1e-12);
        assert!((output - 2.50).abs() < 1e-12);
        for model in ["gemini-3.6-flash", "gemini-3.7-flash"] {
            let pricing = get_gemini_registry()
                .get_core_model_pricing(model)
                .expect("promotional pricing should be available");
            assert_eq!(pricing.input_cost_per_1k_tokens, 0.00075);
            assert_eq!(pricing.output_cost_per_1k_tokens, 0.00375);
        }
        assert_eq!(
            get_model_pricing("gemini-3.5-flash-lite").unwrap(),
            (0.3, 2.5)
        );
        assert!(matches!(
            get_model_pricing("gemini-1.5-flash"),
            Err(ProviderError::ModelNotFound { .. })
        ));
        assert!(matches!(
            get_model_pricing("unknown-google-model"),
            Err(ProviderError::ModelNotFound { .. })
        ));
    }

    #[test]
    fn static_fallback_matches_shared_promotional_authority() {
        let spec = get_gemini_registry()
            .get_model_spec("gemini-3.6-flash")
            .expect("gemini-3.6-flash should remain callable");

        assert_eq!(spec.model_info.input_cost_per_1k_tokens, Some(0.00075));
        assert_eq!(spec.model_info.output_cost_per_1k_tokens, Some(0.00375));
        assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.00075);
        assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.00375);
        assert_eq!(spec.pricing.cache_read_input_token_cost, Some(0.000075));

        let cost = models::CostCalculator::calculate_multimodal_cost(
            "gemini-3.6-flash",
            1_000,
            500,
            Some(200),
            None,
            None,
            None,
        )
        .expect("shared catalog pricing should remain available");
        assert!((cost - 0.00249).abs() < 1e-12, "unexpected cost: {cost}");
    }
}
