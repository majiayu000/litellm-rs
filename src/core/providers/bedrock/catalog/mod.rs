//! Unified Bedrock catalog (BEDROCK-B3).
//!
//! Single source of truth for Bedrock model metadata. Each [`BedrockCatalogEntry`]
//! captures vendor, lifecycle, endpoint support, inference-profile scope, limits,
//! capabilities, pricing, and source provenance, and projects into the existing
//! [`ModelConfig`](super::model_config::ModelConfig) and
//! [`ModelPricing`](crate::core::cost::types::ModelPricing) shapes consumed by the
//! rest of the gateway.
//!
//! Goals (from issue #576):
//!
//! * One catalog entry type with projections for model config and pricing.
//! * Track endpoint support, lifecycle, inference profile support, limits,
//!   capabilities, pricing, and source metadata in one place.
//! * Seed every Bedrock ID already in the repo.
//! * Cross-reference tests guarantee that no pricing ID lacks capability
//!   metadata and no metadata ID lacks an explicit pricing state.
//!
//! The catalog drives the existing `model_config` public facade: `MODEL_CONFIGS`
//! is projected from these entries so callers keep their existing lookup API
//! without duplicating capability metadata. `MODEL_PRICING` remains a separate
//! lazy map in the cost utility module and is cross-checked against catalog
//! pricing below.

use crate::core::cost::types::ModelPricing;

use super::model_config::{BedrockApiType, BedrockModelFamily, ModelConfig};

mod entries;

#[cfg(test)]
mod tests;

pub use entries::all_entries;

/// Vendor of the underlying foundation model.
///
/// Bedrock groups models by the canonical "vendor.model" prefix in the model
/// ID (e.g. `anthropic.claude-...`, `meta.llama3-...`). The [`BedrockVendor`]
/// captures the vendor in a typed form so callers do not parse strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BedrockVendor {
    Amazon,
    Anthropic,
    AI21,
    Cohere,
    DeepSeek,
    Google,
    Luma,
    Meta,
    MiniMax,
    Mistral,
    Moonshot,
    Nvidia,
    OpenAI,
    Qwen,
    Stability,
    TwelveLabs,
    Writer,
}

impl BedrockVendor {
    /// Best-effort vendor lookup from a Bedrock model ID prefix.
    pub fn from_model_id(model_id: &str) -> Option<Self> {
        let prefix = model_id.split('.').next()?;
        let vendor = match prefix {
            "amazon" => Self::Amazon,
            "anthropic" => Self::Anthropic,
            "ai21" => Self::AI21,
            "cohere" => Self::Cohere,
            "deepseek" => Self::DeepSeek,
            "google" => Self::Google,
            "luma" => Self::Luma,
            "meta" => Self::Meta,
            "minimax" => Self::MiniMax,
            "mistral" => Self::Mistral,
            "moonshot" | "moonshotai" => Self::Moonshot,
            "nvidia" => Self::Nvidia,
            "openai" => Self::OpenAI,
            "qwen" => Self::Qwen,
            "stability" => Self::Stability,
            "twelvelabs" => Self::TwelveLabs,
            "writer" => Self::Writer,
            _ => return None,
        };
        Some(vendor)
    }
}

/// Lifecycle stage of a Bedrock model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelLifecycle {
    /// Generally available and supported.
    Live,
    /// Preview / early access — usage may be gated by AWS.
    Preview,
    /// Deprecated by AWS; still callable until the deprecation date.
    Deprecated {
        /// ISO-8601 date (`YYYY-MM-DD`) on which AWS will retire the model.
        deprecation_date: &'static str,
    },
}

/// API endpoints a Bedrock model supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndpointSupport {
    pub converse: bool,
    pub invoke: bool,
    pub streaming: bool,
}

impl EndpointSupport {
    /// Converse + ConverseStream (most modern chat models).
    pub const CONVERSE: Self = Self {
        converse: true,
        invoke: false,
        streaming: true,
    };
    /// InvokeModel only (legacy and most non-chat endpoints).
    pub const INVOKE: Self = Self {
        converse: false,
        invoke: true,
        streaming: true,
    };
    /// InvokeModel without streaming (embeddings, image, batch-only).
    pub const INVOKE_NON_STREAMING: Self = Self {
        converse: false,
        invoke: true,
        streaming: false,
    };
}

/// Inference profile scope (cross-region routing prefix).
///
/// Bedrock supports geo and region inference profiles such as
/// `us.anthropic.claude-...` or `global.anthropic.claude-...`. Each variant
/// here represents a scope that may be prefixed to the canonical model ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceProfileScope {
    Global,
    UnitedStates,
    Europe,
    AsiaPacific,
    SouthAmerica,
    Canada,
    MiddleEast,
    Africa,
}

/// Hard limits exposed by Bedrock for a given model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelLimits {
    pub max_context_length: u32,
    pub max_output_length: Option<u32>,
}

/// Capability flags surfaced to upstream consumers (router, cost layer, UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub function_calling: bool,
    pub multimodal: bool,
    pub vision: bool,
    pub thinking: bool,
    pub embeddings: bool,
    pub image_generation: bool,
    pub audio: bool,
}

impl ModelCapabilities {
    /// Standard chat capabilities: streaming + tools + vision-class multimodal.
    pub const CHAT_MULTIMODAL: Self = Self {
        streaming: true,
        function_calling: true,
        multimodal: true,
        vision: true,
        thinking: false,
        embeddings: false,
        image_generation: false,
        audio: false,
    };

    /// Chat without tools or vision (legacy Claude v1/v2, older Llama, Mistral).
    pub const CHAT_TEXT_ONLY: Self = Self {
        streaming: true,
        function_calling: false,
        multimodal: false,
        vision: false,
        thinking: false,
        embeddings: false,
        image_generation: false,
        audio: false,
    };

    /// Text-only Bedrock chat with tool calling but no vision.
    pub const CHAT_TOOLS_TEXT: Self = Self {
        streaming: true,
        function_calling: true,
        multimodal: false,
        vision: false,
        thinking: false,
        embeddings: false,
        image_generation: false,
        audio: false,
    };

    /// Embeddings model (no streaming, no tools).
    pub const EMBEDDINGS: Self = Self {
        streaming: false,
        function_calling: false,
        multimodal: false,
        vision: false,
        thinking: false,
        embeddings: true,
        image_generation: false,
        audio: false,
    };

    /// Multimodal embeddings (image / video + text).
    pub const EMBEDDINGS_MULTIMODAL: Self = Self {
        streaming: false,
        function_calling: false,
        multimodal: true,
        vision: true,
        thinking: false,
        embeddings: true,
        image_generation: false,
        audio: false,
    };

    /// Image / video generation model.
    pub const IMAGE_GENERATION: Self = Self {
        streaming: false,
        function_calling: false,
        multimodal: true,
        vision: true,
        thinking: false,
        embeddings: false,
        image_generation: true,
        audio: false,
    };
}

/// Reason a catalog entry has no pricing (so cross-reference tests can pass).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoPricingReason {
    /// Pricing depends on something beyond per-token billing (image / video
    /// per-second or per-frame, rerank per-query etc.) and is intentionally
    /// not surfaced via the per-1k-token field.
    NonTokenBilling,
    /// AWS has not published a public price yet (preview models).
    NotPublished,
    /// Reserved fallback for ARN-resolved entries with runtime-only pricing.
    RuntimeResolved,
}

/// Pricing in USD per 1k tokens, plus optional metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct BedrockPricing {
    pub input_cost_per_1k_tokens: f64,
    pub output_cost_per_1k_tokens: f64,
    /// Optional input price for cached prompt tokens (Anthropic / Nova features).
    pub cache_read_input_cost_per_1k_tokens: Option<f64>,
    /// Optional input price for cache-creation writes.
    pub cache_write_input_cost_per_1k_tokens: Option<f64>,
    /// `USD` unless AWS quotes a different currency for the region.
    pub currency: &'static str,
}

impl BedrockPricing {
    /// Build a pricing record with the simple per-token shape used by most
    /// Bedrock models.
    pub const fn per_1k(input: f64, output: f64) -> Self {
        Self {
            input_cost_per_1k_tokens: input,
            output_cost_per_1k_tokens: output,
            cache_read_input_cost_per_1k_tokens: None,
            cache_write_input_cost_per_1k_tokens: None,
            currency: "USD",
        }
    }
}

/// Optional source citation for the catalog entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMetadata {
    pub url: &'static str,
    /// ISO-8601 date (`YYYY-MM-DD`) the entry was last cross-referenced
    /// against the upstream AWS source.
    pub verified_date: &'static str,
}

impl SourceMetadata {
    pub const AWS_BEDROCK_PRICING: Self = Self {
        url: "https://aws.amazon.com/bedrock/pricing/",
        verified_date: "2026-05-25",
    };
    pub const AWS_BEDROCK_MODELS: Self = Self {
        url: "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html",
        verified_date: "2026-05-25",
    };
}

/// Single source of truth for a Bedrock model.
///
/// Every Bedrock model ID supported by this gateway has exactly one
/// [`BedrockCatalogEntry`]. The same entry is projected into the legacy
/// [`ModelConfig`] map (capability metadata) and into the
/// [`ModelPricing`] map (cost layer). Cross-reference tests in
/// `tests.rs` ensure the projections never drift from the existing surfaces.
#[derive(Debug, Clone)]
pub struct BedrockCatalogEntry {
    /// Bedrock-native model ID (e.g. `anthropic.claude-3-5-sonnet-20241022-v2:0`).
    pub model_id: &'static str,
    /// Canonical lookup key used by upstream pricing / catalog APIs. Usually
    /// equal to `model_id`, but may differ for alias entries.
    pub canonical_id: &'static str,
    pub display_name: &'static str,
    pub vendor: BedrockVendor,
    pub family: BedrockModelFamily,
    pub api_type: BedrockApiType,
    pub lifecycle: ModelLifecycle,
    pub endpoints: EndpointSupport,
    pub inference_profiles: &'static [InferenceProfileScope],
    pub limits: ModelLimits,
    pub capabilities: ModelCapabilities,
    /// `Some(pricing)` when AWS publishes a per-token rate, `None` paired with
    /// `no_pricing_reason` otherwise.
    pub pricing: Option<BedrockPricing>,
    pub no_pricing_reason: Option<NoPricingReason>,
    pub source: SourceMetadata,
}

impl BedrockCatalogEntry {
    /// Project to the public [`ModelConfig`] shape consumed by
    /// `get_model_config()` and related facade helpers.
    pub fn to_model_config(&self) -> ModelConfig {
        let (input, output) = match &self.pricing {
            Some(p) => (p.input_cost_per_1k_tokens, p.output_cost_per_1k_tokens),
            None => (0.0, 0.0),
        };
        ModelConfig {
            family: self.family.clone(),
            api_type: self.api_type.clone(),
            supports_streaming: self.capabilities.streaming,
            supports_function_calling: self.capabilities.function_calling,
            supports_multimodal: self.capabilities.multimodal,
            max_context_length: self.limits.max_context_length,
            max_output_length: self.limits.max_output_length,
            input_cost_per_1k: input,
            output_cost_per_1k: output,
        }
    }

    /// Project to the existing [`ModelPricing`] shape used by the cost layer.
    /// Returns `None` when the model has no per-token pricing.
    pub fn to_model_pricing(&self) -> Option<ModelPricing> {
        self.pricing.as_ref().map(|p| ModelPricing {
            model: self.model_id.to_string(),
            input_cost_per_1k_tokens: p.input_cost_per_1k_tokens,
            output_cost_per_1k_tokens: p.output_cost_per_1k_tokens,
            currency: p.currency.to_string(),
            ..Default::default()
        })
    }

    /// True iff this entry has either pricing or an explicit no-pricing
    /// reason. Used by the integrity invariants.
    pub fn has_pricing_state(&self) -> bool {
        self.pricing.is_some() || self.no_pricing_reason.is_some()
    }
}

/// Look up a catalog entry by Bedrock model ID.
pub fn get_catalog_entry(model_id: &str) -> Option<&'static BedrockCatalogEntry> {
    all_entries().iter().find(|e| e.model_id == model_id)
}

/// All Bedrock model IDs currently known to the catalog.
pub fn all_model_ids() -> Vec<&'static str> {
    all_entries().iter().map(|e| e.model_id).collect()
}
