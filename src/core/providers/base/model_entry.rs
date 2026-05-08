//! Shared static catalog entry for provider-local model lists.
//!
//! Several providers maintain a small `LazyLock<HashMap<&str, ModelInfo>>`
//! catalog of supported models with the same field shape:
//! `{model_id, display_name, max_context_length, max_output_length,
//!   supports_tools, supports_multimodal, input_cost_per_million,
//!   output_cost_per_million}`. The struct used to be copy-pasted into each
//! provider's `model_info.rs` (manus, morph, nlp_cloud, petals, predibase,
//! ragflow). This module is the shared definition.

/// Static catalog entry describing one model supported by a provider.
///
/// Field semantics match the per-provider copies that previously lived in
/// each provider's `model_info.rs` module.
#[derive(Debug, Clone, Copy)]
pub struct ProviderModelEntry {
    /// Model ID as used in the provider API.
    pub model_id: &'static str,
    /// Human-readable display name.
    pub display_name: &'static str,
    /// Maximum context length in tokens.
    pub max_context_length: u32,
    /// Maximum output length in tokens.
    pub max_output_length: u32,
    /// Whether the model supports tools / function calling.
    pub supports_tools: bool,
    /// Whether the model supports multimodal (vision / audio) input.
    pub supports_multimodal: bool,
    /// Input cost per million tokens, in USD.
    pub input_cost_per_million: f64,
    /// Output cost per million tokens, in USD.
    pub output_cost_per_million: f64,
}
