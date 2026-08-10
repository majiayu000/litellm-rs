//! Google Vertex AI Provider Implementation
//!
//! Comprehensive support for Google Vertex AI including:
//! - Gemini models (Pro, Flash, Ultra)
//! - Partner models (Anthropic, AI21, Meta Llama)
//! - Model Garden
//! - Multimodal embeddings
//! - Image generation
//! - Text-to-speech
//! - Context caching
//! - Batch operations

pub mod auth;
pub mod batches;
pub mod client;
pub mod common_utils;
pub mod context_caching;
pub mod count_tokens;
pub mod embeddings;
pub mod error;
pub mod files;
pub mod fine_tuning;
pub mod gemini;
pub mod gemini_embeddings;
pub mod google_genai;
pub mod image_generation;
pub mod models;
pub mod multimodal_embeddings;
pub mod partner_models;
pub mod text_to_speech;
pub mod transformers;
pub mod vector_stores;
pub mod vertex_ai_partner_models;
pub mod vertex_embeddings;
pub mod vertex_model_garden;

pub use auth::{VertexAuth, VertexCredentials};
pub use client::VertexAIProvider;
pub use common_utils::VertexAIConfig;
pub use error::VertexAIError;

use crate::core::net::{ProviderEndpointAccess, ProviderEndpointPolicy};
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::error::gateway_error::GatewayError;

pub(crate) fn calculate_vertex_cost(
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Result<f64, ProviderError> {
    PricingService::shared_embedded_default()
        .and_then(|service| {
            service.calculate_loaded_usage_cost_for_provider(
                "vertex_ai",
                model,
                &PricingUsage::new(input_tokens, output_tokens),
            )
        })
        .map(|cost| cost.total_cost)
        .map_err(|error| vertex_pricing_error(model, error))
}

pub(crate) fn vertex_prices_per_1k(
    model: &str,
) -> Result<(Option<f64>, Option<f64>), ProviderError> {
    let (_, pricing) = PricingService::shared_embedded_default()
        .map_err(|error| vertex_pricing_error(model, error))?
        .get_model_info_for_provider("vertex_ai", model)
        .ok_or_else(|| ProviderError::model_not_found("vertex_ai", model))?;
    Ok((
        pricing.input_cost_per_token.map(|price| price * 1_000.0),
        pricing.output_cost_per_token.map(|price| price * 1_000.0),
    ))
}

fn vertex_pricing_error(model: &str, error: GatewayError) -> ProviderError {
    match error {
        GatewayError::NotFound(_) => ProviderError::model_not_found("vertex_ai", model),
        error => ProviderError::Other {
            provider: "vertex_ai",
            message: format!("pricing authority failed for model '{model}': {error}"),
        },
    }
}

/// Main VertexAI Provider Configuration
#[derive(Debug, Clone)]
pub struct VertexAIProviderConfig {
    /// Google Cloud Project ID
    pub project_id: String,

    /// Vertex AI region (e.g., "us-central1")
    pub location: String,

    /// API version to use ("v1" or "v1beta1")
    pub api_version: String,

    /// Authentication credentials
    pub credentials: VertexCredentials,

    /// Custom API endpoint (optional)
    pub api_base: Option<String>,

    /// Outbound network access policy
    pub endpoint_access: ProviderEndpointAccess,

    /// Request timeout in seconds
    pub timeout_seconds: u64,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Enable experimental features
    pub enable_experimental: bool,
}

impl Default for VertexAIProviderConfig {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            location: "us-central1".to_string(),
            api_version: "v1".to_string(),
            credentials: VertexCredentials::ApplicationDefault,
            api_base: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout_seconds: 60,
            max_retries: 3,
            enable_experimental: false,
        }
    }
}

impl crate::core::traits::provider::ProviderConfig for VertexAIProviderConfig {
    fn validate(&self) -> Result<(), String> {
        if self.project_id.is_empty() {
            return Err("Project ID is required".to_string());
        }
        if self.location.is_empty() {
            return Err("Location is required".to_string());
        }
        if self.endpoint_access == ProviderEndpointAccess::PrivateNetwork && self.api_base.is_none()
        {
            return Err("private_network endpoint access requires api_base".to_string());
        }
        ProviderEndpointPolicy::for_base_url(
            self.endpoint_access,
            self.api_base
                .as_deref()
                .unwrap_or("https://aiplatform.googleapis.com"),
        )
        .map_err(|error| format!("invalid Vertex AI API base policy: {error}"))?;
        Ok(())
    }

    fn api_key(&self) -> Option<&str> {
        None // Vertex AI uses credentials, not API keys
    }

    fn api_base(&self) -> Option<&str> {
        self.api_base
            .as_deref()
            .or(Some("https://aiplatform.googleapis.com"))
    }

    fn endpoint_access(&self) -> ProviderEndpointAccess {
        self.endpoint_access
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Supported Vertex AI models
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VertexAIModel {
    // Gemini 3.1 models (2026 - latest previews)
    Gemini31ProPreview, // gemini-3.1-pro-preview
    Gemini31Flash,      // gemini-3.1-flash
    Gemini31FlashLite,  // gemini-3.1-flash-lite

    // Gemini 3.0 models (2025-2026)
    Gemini3Pro,          // gemini-3-pro
    Gemini3ProDeepThink, // gemini-3-pro-deep-think
    Gemini3FlashPreview, // gemini-3-flash-preview
    Gemini3ProImage,     // gemini-3-pro-image-preview

    // Gemini 2.5 models (2025 - Latest)
    Gemini25Pro,       // gemini-2.5-pro
    Gemini25Flash,     // gemini-2.5-flash
    Gemini25FlashLite, // gemini-2.5-flash-lite

    // Gemini 2.0 models (2024-2025)
    Gemini20Flash,         // gemini-2.0-flash
    Gemini20FlashExp,      // gemini-2.0-flash-exp
    Gemini20FlashThinking, // gemini-2.0-flash-thinking-exp
    Gemini20FlashLite,     // gemini-2.0-flash-lite

    // Gemini 1.5 models (2024)
    GeminiPro,       // gemini-1.5-pro
    GeminiProVision, // gemini-1.5-pro-vision (legacy)
    GeminiFlash,     // gemini-1.5-flash
    GeminiFlash8B,   // gemini-1.5-flash-8b

    // Gemini 1.0 models (legacy)
    GeminiUltra, // gemini-ultra (deprecated)

    // Partner models - Claude
    ClaudeOpus47,   // claude-opus-4-7
    ClaudeOpus46,   // claude-opus-4-6@20260114
    ClaudeOpus45,   // claude-opus-4-5@20251110
    ClaudeSonnet46, // claude-sonnet-4-6
    ClaudeSonnet45, // claude-sonnet-4-5@20250929
    ClaudeHaiku45,  // claude-haiku-4-5@20251001
    ClaudeSonnet4,  // claude-sonnet-4@20250514
    Claude3Opus,
    Claude3Sonnet,
    Claude3Haiku,
    Claude35Sonnet, // claude-3-5-sonnet@20241022

    // Meta models - Llama 3.x and 4
    Llama3_70B,
    Llama3_8B,
    Llama31_405B,   // llama-3.1-405b
    Llama31_70B,    // llama-3.1-70b
    Llama32_90B,    // llama-3.2-90b
    Llama4Scout,    // llama-4-scout (2025)
    Llama4Maverick, // llama-4-maverick (2025)

    // AI21 models
    Jamba15Large,
    Jamba15Mini,
    Jamba2, // jamba-2 (2025)

    // Mistral models
    MistralLarge, // mistral-large
    MistralNemo,  // mistral-nemo
    Codestral,    // codestral

    // Custom model
    Custom(String),
}

impl VertexAIModel {
    /// Get the model ID string for API calls
    pub fn model_id(&self) -> String {
        match self {
            // Gemini 3.1 models
            Self::Gemini31ProPreview => "gemini-3.1-pro-preview".to_string(),
            Self::Gemini31Flash => "gemini-3.1-flash".to_string(),
            Self::Gemini31FlashLite => "gemini-3.1-flash-lite".to_string(),

            // Gemini 3.0 models
            Self::Gemini3Pro => "gemini-3-pro".to_string(),
            Self::Gemini3ProDeepThink => "gemini-3-pro-deep-think".to_string(),
            Self::Gemini3FlashPreview => "gemini-3-flash-preview".to_string(),
            Self::Gemini3ProImage => "gemini-3-pro-image-preview".to_string(),

            // Gemini 2.5 models
            Self::Gemini25Pro => "gemini-2.5-pro".to_string(),
            Self::Gemini25Flash => "gemini-2.5-flash".to_string(),
            Self::Gemini25FlashLite => "gemini-2.5-flash-lite".to_string(),

            // Gemini 2.0 models
            Self::Gemini20Flash => "gemini-2.0-flash".to_string(),
            Self::Gemini20FlashExp => "gemini-2.0-flash-exp".to_string(),
            Self::Gemini20FlashThinking => "gemini-2.0-flash-thinking-exp-1219".to_string(),
            Self::Gemini20FlashLite => "gemini-2.0-flash-lite".to_string(),

            // Gemini 1.5 models
            Self::GeminiPro => "gemini-1.5-pro-002".to_string(),
            Self::GeminiProVision => "gemini-1.5-pro-vision".to_string(),
            Self::GeminiFlash => "gemini-1.5-flash-002".to_string(),
            Self::GeminiFlash8B => "gemini-1.5-flash-8b".to_string(),

            // Legacy
            Self::GeminiUltra => "gemini-ultra".to_string(),

            // Claude models
            Self::ClaudeOpus47 => "claude-opus-4-7".to_string(),
            Self::ClaudeOpus46 => "claude-opus-4-6@20260114".to_string(),
            Self::ClaudeOpus45 => "claude-opus-4-5@20251110".to_string(),
            Self::ClaudeSonnet46 => "claude-sonnet-4-6".to_string(),
            Self::ClaudeSonnet45 => "claude-sonnet-4-5@20250929".to_string(),
            Self::ClaudeHaiku45 => "claude-haiku-4-5@20251001".to_string(),
            Self::ClaudeSonnet4 => "claude-sonnet-4@20250514".to_string(),
            Self::Claude3Opus => "claude-3-opus@20240229".to_string(),
            Self::Claude3Sonnet => "claude-3-sonnet@20240229".to_string(),
            Self::Claude3Haiku => "claude-3-haiku@20240307".to_string(),
            Self::Claude35Sonnet => "claude-3-5-sonnet@20241022".to_string(),

            // Meta Llama models
            Self::Llama3_70B => "meta/llama3-70b-instruct-maas".to_string(),
            Self::Llama3_8B => "meta/llama3-8b-instruct-maas".to_string(),
            Self::Llama31_405B => "meta/llama-3.1-405b-instruct-maas".to_string(),
            Self::Llama31_70B => "meta/llama-3.1-70b-instruct-maas".to_string(),
            Self::Llama32_90B => "meta/llama-3.2-90b-vision-instruct-maas".to_string(),
            Self::Llama4Scout => "meta/llama-4-scout-17b-16e-instruct".to_string(),
            Self::Llama4Maverick => "meta/llama-4-maverick-17b-128e-instruct".to_string(),

            // AI21 models
            Self::Jamba15Large => "ai21/jamba-1.5-large".to_string(),
            Self::Jamba15Mini => "ai21/jamba-1.5-mini".to_string(),
            Self::Jamba2 => "ai21/jamba-2-instruct".to_string(),

            // Mistral models
            Self::MistralLarge => "mistral/mistral-large-2411".to_string(),
            Self::MistralNemo => "mistral/mistral-nemo".to_string(),
            Self::Codestral => "mistral/codestral-2501".to_string(),

            Self::Custom(id) => id.clone(),
        }
    }

    /// Check if this is a Gemini model
    pub fn is_gemini(&self) -> bool {
        matches!(
            self,
            Self::Gemini31ProPreview
                | Self::Gemini31Flash
                | Self::Gemini31FlashLite
                | Self::Gemini3Pro
                | Self::Gemini3ProDeepThink
                | Self::Gemini3FlashPreview
                | Self::Gemini3ProImage
                | Self::Gemini25Pro
                | Self::Gemini25Flash
                | Self::Gemini25FlashLite
                | Self::Gemini20Flash
                | Self::Gemini20FlashExp
                | Self::Gemini20FlashThinking
                | Self::Gemini20FlashLite
                | Self::GeminiPro
                | Self::GeminiProVision
                | Self::GeminiFlash
                | Self::GeminiFlash8B
                | Self::GeminiUltra
        )
    }

    /// Check if this is a partner model
    pub fn is_partner_model(&self) -> bool {
        matches!(
            self,
            Self::ClaudeOpus47
                | Self::ClaudeOpus46
                | Self::ClaudeOpus45
                | Self::ClaudeSonnet46
                | Self::ClaudeSonnet45
                | Self::ClaudeHaiku45
                | Self::ClaudeSonnet4
                | Self::Claude3Opus
                | Self::Claude3Sonnet
                | Self::Claude3Haiku
                | Self::Claude35Sonnet
                | Self::Llama3_70B
                | Self::Llama3_8B
                | Self::Llama31_405B
                | Self::Llama31_70B
                | Self::Llama32_90B
                | Self::Llama4Scout
                | Self::Llama4Maverick
                | Self::Jamba15Large
                | Self::Jamba15Mini
                | Self::Jamba2
                | Self::MistralLarge
                | Self::MistralNemo
                | Self::Codestral
        )
    }

    /// Check if model supports vision/multimodal
    pub fn supports_vision(&self) -> bool {
        matches!(
            self,
            Self::Gemini31ProPreview
                | Self::Gemini31Flash
                | Self::Gemini31FlashLite
                | Self::Gemini3Pro
                | Self::Gemini3ProDeepThink
                | Self::Gemini3FlashPreview
                | Self::Gemini3ProImage
                | Self::Gemini25Pro
                | Self::Gemini25Flash
                | Self::Gemini25FlashLite
                | Self::Gemini20Flash
                | Self::Gemini20FlashExp
                | Self::Gemini20FlashThinking
                | Self::Gemini20FlashLite
                | Self::GeminiPro
                | Self::GeminiProVision
                | Self::GeminiFlash
                | Self::GeminiFlash8B
                | Self::ClaudeOpus47
                | Self::ClaudeOpus46
                | Self::ClaudeOpus45
                | Self::ClaudeSonnet46
                | Self::ClaudeSonnet45
                | Self::ClaudeHaiku45
                | Self::ClaudeSonnet4
                | Self::Claude3Opus
                | Self::Claude3Sonnet
                | Self::Claude3Haiku
                | Self::Claude35Sonnet
                | Self::Llama32_90B
                | Self::Llama4Scout
                | Self::Llama4Maverick
        )
    }

    /// Check if model supports system messages
    pub fn supports_system_messages(&self) -> bool {
        !matches!(self, Self::Custom(_))
    }

    /// Check if model supports response schema/JSON mode
    pub fn supports_response_schema(&self) -> bool {
        self.is_gemini()
    }

    /// Check if model supports function calling
    pub fn supports_function_calling(&self) -> bool {
        self.is_gemini()
            || matches!(
                self,
                Self::ClaudeOpus47
                    | Self::ClaudeOpus46
                    | Self::ClaudeOpus45
                    | Self::ClaudeSonnet46
                    | Self::ClaudeSonnet45
                    | Self::ClaudeHaiku45
                    | Self::ClaudeSonnet4
                    | Self::Claude3Opus
                    | Self::Claude3Sonnet
                    | Self::Claude3Haiku
                    | Self::Claude35Sonnet
                    | Self::MistralLarge
            )
    }

    /// Check if model supports thinking/reasoning mode
    pub fn supports_thinking_mode(&self) -> bool {
        matches!(
            self,
            Self::Gemini3ProDeepThink
                | Self::Gemini25Pro
                | Self::Gemini25Flash
                | Self::Gemini20FlashThinking
        )
    }

    /// Get maximum context window
    pub fn max_context_tokens(&self) -> usize {
        match self {
            // Gemini 3.1 models
            Self::Gemini31ProPreview | Self::Gemini31Flash | Self::Gemini31FlashLite => 1_048_576,

            // Gemini 3.0 models
            Self::Gemini3Pro | Self::Gemini3ProDeepThink | Self::Gemini3FlashPreview => 1_000_000,
            Self::Gemini3ProImage => 65_536,

            // Gemini 2.5 models - 1M+ context
            Self::Gemini25Pro => 1_048_576,       // 1M tokens
            Self::Gemini25Flash => 1_048_576,     // 1M tokens
            Self::Gemini25FlashLite => 1_048_576, // 1M tokens

            // Gemini 2.0 models
            Self::Gemini20Flash => 1_048_576,
            Self::Gemini20FlashExp => 1_048_576,
            Self::Gemini20FlashThinking => 1_048_576,
            Self::Gemini20FlashLite => 1_048_576,

            // Gemini 1.5 models
            Self::GeminiPro => 2_097_152, // 2M tokens
            Self::GeminiProVision => 2_097_152,
            Self::GeminiFlash => 1_048_576, // 1M tokens
            Self::GeminiFlash8B => 1_048_576,

            // Legacy
            Self::GeminiUltra => 1_048_576,

            // Claude models
            Self::ClaudeOpus47 => 1_000_000,
            Self::ClaudeOpus46 => 1_000_000,
            Self::ClaudeOpus45 => 200_000,
            Self::ClaudeSonnet46 => 1_000_000,
            Self::ClaudeSonnet45 => 200_000,
            Self::ClaudeHaiku45 => 200_000,
            Self::ClaudeSonnet4 => 200_000,
            Self::Claude3Opus => 200_000,
            Self::Claude3Sonnet => 200_000,
            Self::Claude3Haiku => 200_000,
            Self::Claude35Sonnet => 200_000,

            // Llama models
            Self::Llama3_70B => 32_768,
            Self::Llama3_8B => 8_192,
            Self::Llama31_405B => 128_000,
            Self::Llama31_70B => 128_000,
            Self::Llama32_90B => 128_000,
            Self::Llama4Scout => 10_000_000, // 10M context (claimed)
            Self::Llama4Maverick => 1_000_000, // 1M context

            // AI21 models
            Self::Jamba15Large => 256_000,
            Self::Jamba15Mini => 256_000,
            Self::Jamba2 => 256_000,

            // Mistral models
            Self::MistralLarge => 128_000,
            Self::MistralNemo => 128_000,
            Self::Codestral => 256_000,

            Self::Custom(_) => 32_768, // Default
        }
    }
}

/// Parse model string to VertexAIModel enum
pub fn parse_vertex_model(model: &str) -> VertexAIModel {
    let model_lower = model.to_lowercase();

    // Gemini 3.1 models (check before Gemini 3.0 as more specific)
    if model_lower.contains("gemini-3.1-flash-lite") {
        return VertexAIModel::Gemini31FlashLite;
    }
    if model_lower.contains("gemini-3.1-flash") {
        return VertexAIModel::Gemini31Flash;
    }
    if model_lower.contains("gemini-3.1-pro") {
        return VertexAIModel::Gemini31ProPreview;
    }

    // Gemini 3.0 models
    if model_lower.contains("gemini-3") && model_lower.contains("deep-think") {
        return VertexAIModel::Gemini3ProDeepThink;
    }
    if model_lower.contains("gemini-3") && model_lower.contains("image") {
        return VertexAIModel::Gemini3ProImage;
    }
    if model_lower.contains("gemini-3-flash") || model_lower.contains("gemini-3.0-flash") {
        return VertexAIModel::Gemini3FlashPreview;
    }
    if model_lower.contains("gemini-3-pro") || model_lower.contains("gemini-3.0-pro") {
        return VertexAIModel::Gemini3Pro;
    }

    // Gemini 2.5 models (newest first)
    if model_lower.contains("gemini-2.5-pro") {
        return VertexAIModel::Gemini25Pro;
    }
    if model_lower.contains("gemini-2.5-flash-lite") {
        return VertexAIModel::Gemini25FlashLite;
    }
    if model_lower.contains("gemini-2.5-flash") {
        return VertexAIModel::Gemini25Flash;
    }

    // Gemini 2.0 models
    if model_lower.contains("gemini-2.0-flash-thinking") {
        return VertexAIModel::Gemini20FlashThinking;
    }
    if model_lower.contains("gemini-2.0-flash-lite") {
        return VertexAIModel::Gemini20FlashLite;
    }
    if model_lower.contains("gemini-2.0-flash-exp") {
        return VertexAIModel::Gemini20FlashExp;
    }
    if model_lower.contains("gemini-2.0-flash") {
        return VertexAIModel::Gemini20Flash;
    }

    // Gemini 1.5 models
    if model_lower.contains("gemini-1.5-flash-8b") {
        return VertexAIModel::GeminiFlash8B;
    }
    if model_lower.contains("gemini-1.5-pro-vision") || model_lower.contains("gemini-pro-vision") {
        return VertexAIModel::GeminiProVision;
    }
    if model_lower.contains("gemini-1.5-pro") || model == "gemini-pro" {
        return VertexAIModel::GeminiPro;
    }
    if model_lower.contains("gemini-1.5-flash") || model == "gemini-flash" {
        return VertexAIModel::GeminiFlash;
    }

    // Legacy Gemini
    if model_lower.contains("gemini-ultra") {
        return VertexAIModel::GeminiUltra;
    }

    // Claude models (check more specific first)
    if model_lower.contains("claude-opus-4-7") || model_lower.contains("claude-opus-4.7") {
        return VertexAIModel::ClaudeOpus47;
    }
    if model_lower.contains("claude-opus-4-6") || model_lower.contains("claude-opus-4.6") {
        return VertexAIModel::ClaudeOpus46;
    }
    if model_lower.contains("claude-opus-4-5") || model_lower.contains("claude-opus-4.5") {
        return VertexAIModel::ClaudeOpus45;
    }
    if model_lower.contains("claude-sonnet-4-6") || model_lower.contains("claude-sonnet-4.6") {
        return VertexAIModel::ClaudeSonnet46;
    }
    if model_lower.contains("claude-sonnet-4-5") || model_lower.contains("claude-sonnet-4.5") {
        return VertexAIModel::ClaudeSonnet45;
    }
    if model_lower.contains("claude-haiku-4-5") || model_lower.contains("claude-haiku-4.5") {
        return VertexAIModel::ClaudeHaiku45;
    }
    if model_lower.contains("claude-sonnet-4")
        && !model_lower.contains("claude-sonnet-4-5")
        && !model_lower.contains("claude-sonnet-4-6")
    {
        return VertexAIModel::ClaudeSonnet4;
    }
    if model_lower.contains("claude-3-5-sonnet") || model_lower.contains("claude-3.5-sonnet") {
        return VertexAIModel::Claude35Sonnet;
    }
    if model_lower.contains("claude-3-opus") {
        return VertexAIModel::Claude3Opus;
    }
    if model_lower.contains("claude-3-sonnet") {
        return VertexAIModel::Claude3Sonnet;
    }
    if model_lower.contains("claude-3-haiku") {
        return VertexAIModel::Claude3Haiku;
    }

    // Llama 4 models (newest first)
    if model_lower.contains("llama-4-scout") || model_lower.contains("llama4-scout") {
        return VertexAIModel::Llama4Scout;
    }
    if model_lower.contains("llama-4-maverick") || model_lower.contains("llama4-maverick") {
        return VertexAIModel::Llama4Maverick;
    }

    // Llama 3.x models
    if model_lower.contains("llama-3.2-90b") || model_lower.contains("llama3.2-90b") {
        return VertexAIModel::Llama32_90B;
    }
    if model_lower.contains("llama-3.1-405b") || model_lower.contains("llama3.1-405b") {
        return VertexAIModel::Llama31_405B;
    }
    if model_lower.contains("llama-3.1-70b") || model_lower.contains("llama3.1-70b") {
        return VertexAIModel::Llama31_70B;
    }
    if model_lower.contains("llama3-70b") || model_lower.contains("llama-3-70b") {
        return VertexAIModel::Llama3_70B;
    }
    if model_lower.contains("llama3-8b") || model_lower.contains("llama-3-8b") {
        return VertexAIModel::Llama3_8B;
    }

    // AI21 Jamba models
    if model_lower.contains("jamba-2") {
        return VertexAIModel::Jamba2;
    }
    if model_lower.contains("jamba-1.5-large") {
        return VertexAIModel::Jamba15Large;
    }
    if model_lower.contains("jamba-1.5-mini") {
        return VertexAIModel::Jamba15Mini;
    }

    // Mistral models
    if model_lower.contains("codestral") {
        return VertexAIModel::Codestral;
    }
    if model_lower.contains("mistral-large") {
        return VertexAIModel::MistralLarge;
    }
    if model_lower.contains("mistral-nemo") {
        return VertexAIModel::MistralNemo;
    }

    VertexAIModel::Custom(model.to_string())
}

#[cfg(test)]
mod tests;
