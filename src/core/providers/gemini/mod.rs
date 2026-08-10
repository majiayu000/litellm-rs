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
pub fn get_model_pricing(model_id: &str) -> Option<(f64, f64)> {
    get_gemini_registry().get_model_pricing(model_id).map(|p| {
        (
            p.input_cost_per_1k_tokens * 1000.0,
            p.output_cost_per_1k_tokens * 1000.0,
        )
    })
}
