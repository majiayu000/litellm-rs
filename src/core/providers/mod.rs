//! AI Provider implementations using Rust-idiomatic enum-based design
//!
//! This module contains the unified Provider enum and all provider implementations.

// Base infrastructure
pub mod base;

// Provider modules - alphabetically ordered
// Tier 1 providers removed in favor of registry/catalog.rs are commented with their tier.
#[cfg(feature = "providers-extended")]
pub mod ai21;
// aiml_api: Tier 1 -> registry/catalog.rs
// aleph_alpha: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod amazon_nova;
pub mod anthropic;
// anyscale: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extra")]
pub mod azure;
#[cfg(feature = "providers-extra")]
pub mod azure_ai;
// baichuan: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod baseten;
#[cfg(feature = "providers-extra")]
pub mod bedrock;
// bytez: Tier 1 -> registry/catalog.rs
// cerebras: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod clarifai;
pub mod cloudflare;
#[cfg(feature = "providers-extended")]
pub mod codestral;
#[cfg(feature = "providers-extended")]
pub mod cohere;
// comet_api: Tier 1 -> registry/catalog.rs
// compactifai: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod custom_api;
// dashscope: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod databricks;
#[cfg(feature = "providers-extended")]
pub mod datarobot;
#[cfg(feature = "providers-extended")]
pub mod deepgram;
// deepinfra: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod deepl;
// deepseek: Tier 1 -> registry/catalog.rs
// docker_model_runner: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod elevenlabs;
#[cfg(feature = "providers-extended")]
pub mod empower;
#[cfg(feature = "providers-extended")]
pub mod exa_ai;
#[cfg(feature = "providers-extended")]
pub mod fal_ai;
// featherless: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod firecrawl;
// fireworks: Tier 1 -> registry/catalog.rs
// friendliai: Tier 1 -> registry/catalog.rs
// galadriel: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod gemini;
#[cfg(feature = "providers-extended")]
pub mod gigachat;
#[cfg(feature = "providers-extended")]
pub mod github;
#[cfg(feature = "providers-extended")]
pub mod github_copilot;
#[cfg(feature = "providers-extended")]
pub mod google_pse;
#[cfg(feature = "providers-extended")]
pub mod gradient_ai;
// groq: Tier 1 -> registry/catalog.rs
// heroku: Tier 1 -> registry/catalog.rs
// hosted_vllm: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod huggingface;
// hyperbolic: Tier 1 -> registry/catalog.rs
// infinity: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod jina;
// lambda_ai: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod langgraph;
// lemonade: Tier 1 -> registry/catalog.rs
// linkup: Tier 1 -> registry/catalog.rs
// llamafile: Tier 1 -> registry/catalog.rs
// lm_studio: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod manus;
// maritalk: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extra")]
pub mod meta_llama;
#[cfg(feature = "providers-extended")]
pub mod milvus;
// minimax: Tier 1 -> registry/catalog.rs
pub mod mistral;
// moonshot: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod morph;
// nanogpt: Tier 1 -> registry/catalog.rs
// nebius: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod nlp_cloud;
// novita: Tier 1 -> registry/catalog.rs
// nscale: Tier 1 -> registry/catalog.rs
// nvidia_nim: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod oci;
#[cfg(feature = "providers-extended")]
pub mod ollama;
// oobabooga: Tier 1 -> registry/catalog.rs
pub mod openai;
pub mod openai_like;
// openrouter: Tier 1 -> registry/catalog.rs
// ovhcloud: Tier 1 -> registry/catalog.rs
// perplexity: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod petals;
#[cfg(feature = "providers-extended")]
pub mod pg_vector;
// poe: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod predibase;
// qwen: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod ragflow;
#[cfg(feature = "providers-extended")]
pub mod recraft;
#[cfg(feature = "providers-extended")]
pub mod replicate;
#[cfg(feature = "providers-extended")]
pub mod runwayml;
#[cfg(feature = "providers-extended")]
pub mod sagemaker;
// sambanova: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod sap_ai;
#[cfg(feature = "providers-extended")]
pub mod searxng;
// siliconflow: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod snowflake;
#[cfg(feature = "providers-extended")]
pub mod spark;
#[cfg(feature = "providers-extended")]
pub mod stability;
#[cfg(feature = "providers-extended")]
pub mod tavily;
// together: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod topaz;
#[cfg(feature = "providers-extended")]
pub mod triton;
#[cfg(feature = "providers-extra")]
pub mod v0;
#[cfg(feature = "providers-extended")]
pub mod vercel_ai;
#[cfg(feature = "providers-extra")]
pub mod vertex_ai;
// vllm: Tier 1 -> registry/catalog.rs
// volcengine: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod voyage;
// wandb: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod watsonx;
// xai: Tier 1 -> registry/catalog.rs
// xiaomi_mimo: Tier 1 -> registry/catalog.rs
// xinference: Tier 1 -> registry/catalog.rs
// yi: Tier 1 -> registry/catalog.rs
// zhipu: Tier 1 -> registry/catalog.rs

// Shared utilities and architecture
pub mod macros; // Macros for reducing boilerplate
pub mod shared; // Shared utilities for all providers // Compile-time capability verification
pub mod thinking; // Thinking/reasoning provider trait (modular)
pub mod transform; // Request/Response transformation engine // Request/Response context and metadata

// Provider type enumeration (extracted from this module)
pub mod provider_type;
pub use provider_type::ProviderType;

// Factory: create_provider, from_config_async, config builders
pub mod factory;
pub use factory::{create_provider, is_provider_selector_supported};

// Registry and unified provider
pub mod contextual_error;
pub mod provider_error_conversions;
pub mod provider_registry;
pub mod registry; // Data-driven Tier 1 provider catalog
pub mod unified_provider;

// Test modules (only compiled during tests)
#[cfg(test)]
mod unified_provider_tests;

// Export main types
pub use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::responses::{
    ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse,
};
use crate::core::types::{
    chat::ChatRequest, embedding::EmbeddingRequest, image::ImageGenerationRequest,
};
use crate::core::types::{context::RequestContext, model::ProviderCapability};
pub use contextual_error::ContextualError;
pub use provider_registry::ProviderRegistry;
pub use unified_provider::ProviderError;

// ==================== Provider Dispatch Macros ====================
//
// Consolidated into a single `dispatch_provider!` macro with 4 dispatch kinds,
// selected by the first token.  The Provider variant list appears once per
// `@expand` arm (4 arms total).  To add or remove a variant, update only the
// `@expand` arms below.
//
// Former macros -> new calling convention:
//   dispatch_provider!(self, method)              -> dispatch_provider!(sync, self, method)
//   dispatch_provider!(self, method, arg)         -> dispatch_provider!(sync, self, method, arg)
//   dispatch_provider_async!(self, m, a, b)       -> dispatch_provider!(async_err, self, m, a, b)
//   dispatch_provider_value!(self, method)        -> dispatch_provider!(value, self, method)
//   dispatch_provider_value!(self, method, arg)   -> dispatch_provider!(value, self, method, arg)
//   dispatch_provider_async_direct!(self, method) -> dispatch_provider!(async_direct, self, method)

macro_rules! dispatch_provider {
    // -- sync: p.$method(args...) --
    (sync, $self:expr, $method:ident) => {
        dispatch_provider!(@expand sync, $self, $method,)
    };
    (sync, $self:expr, $method:ident, $($arg:expr),+ $(,)?) => {
        dispatch_provider!(@expand sync, $self, $method, $($arg),+)
    };

    // -- async_err: LLMProvider::$method(p, args...).await.map_err(ProviderError::from) --
    (async_err, $self:expr, $method:ident $(, $arg:expr)* $(,)?) => {
        dispatch_provider!(@expand async_err, $self, $method, $($arg),*)
    };

    // -- value: LLMProvider::$method(p, args...) --
    (value, $self:expr, $method:ident) => {
        dispatch_provider!(@expand value, $self, $method,)
    };
    (value, $self:expr, $method:ident, $($arg:expr),+ $(,)?) => {
        dispatch_provider!(@expand value, $self, $method, $($arg),+)
    };

    // -- async_direct: LLMProvider::$method(p).await --
    (async_direct, $self:expr, $method:ident) => {
        dispatch_provider!(@expand async_direct, $self, $method,)
    };

    // ================================================================
    // @expand arms: single source of truth for the Provider variant list.
    // To add/remove a variant, update these 4 arms.
    // ================================================================

    (@expand sync, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => p.$method($($arg),*),
            Provider::Anthropic(p) => p.$method($($arg),*),
            Provider::Mistral(p) => p.$method($($arg),*),
            Provider::Cloudflare(p) => p.$method($($arg),*),
            Provider::OpenAILike(p) => p.$method($($arg),*),
        }
    };

    (@expand async_err, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::OpenAILike(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
        }
    };

    (@expand value, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),*),
            Provider::OpenAILike(p) => LLMProvider::$method(p, $($arg),*),
        }
    };

    (@expand async_direct, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p).await,
            Provider::Anthropic(p) => LLMProvider::$method(p).await,
            Provider::Mistral(p) => LLMProvider::$method(p).await,
            Provider::Cloudflare(p) => LLMProvider::$method(p).await,
            Provider::OpenAILike(p) => LLMProvider::$method(p).await,
        }
    };
}

/// Selective provider dispatch with default fallback.
/// Already parametric over its own provider list, kept separate.
#[allow(unused_macros)]
macro_rules! dispatch_provider_selective {
    ($self:expr, $method:ident, { $($provider:ident),+ }, $default:expr) => {
        match $self {
            $(Provider::$provider(p) => p.$method()),+,
            _ => $default,
        }
    };

    ($self:expr, $method:ident($($arg:expr),+), { $($provider:ident),+ }, $default:expr) => {
        match $self {
            $(Provider::$provider(p) => p.$method($($arg),+)),+,
            _ => $default,
        }
    };
}

/// Unified Provider Enum (Rust-idiomatic design)
///
/// This enum provides zero-cost abstractions and type safety for all providers.
/// Each variant contains a concrete provider implementation.
#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Mistral(mistral::MistralProvider),
    Cloudflare(cloudflare::CloudflareProvider),
    /// Tier 1: data-driven OpenAI-compatible providers (groq, together, fireworks, etc.)
    OpenAILike(openai_like::OpenAILikeProvider),
}

impl Provider {
    /// Get provider name
    pub fn name(&self) -> &'static str {
        match self {
            Provider::OpenAI(_) => "openai",
            Provider::Anthropic(_) => "anthropic",
            Provider::Mistral(_) => "mistral",
            Provider::Cloudflare(_) => "cloudflare",
            Provider::OpenAILike(p) => {
                use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
                p.name()
            }
        }
    }

    /// Get provider type
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Provider::OpenAI(_) => ProviderType::OpenAI,
            Provider::Anthropic(_) => ProviderType::Anthropic,
            Provider::Mistral(_) => ProviderType::Mistral,
            Provider::Cloudflare(_) => ProviderType::Cloudflare,
            Provider::OpenAILike(_) => ProviderType::OpenAICompatible,
        }
    }

    /// Single source of truth for factory branches currently wired in `from_config_async`.
    pub fn factory_supported_provider_types() -> &'static [ProviderType] {
        registry::dispatchable_provider_types_slice()
    }

    /// Check if provider supports a specific model
    pub fn supports_model(&self, model: &str) -> bool {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(value, self, supports_model, model)
    }

    /// Get provider capabilities
    pub fn capabilities(&self) -> &'static [ProviderCapability] {
        dispatch_provider!(sync, self, capabilities)
    }

    /// Execute chat completion
    pub async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_err, self, chat_completion, request, context)
    }

    /// Execute health check
    pub async fn health_check(&self) -> crate::core::types::health::HealthStatus {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_direct, self, health_check)
    }

    /// List available models
    pub fn list_models(&self) -> &[crate::core::types::model::ModelInfo] {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(value, self, models)
    }

    /// Calculate cost using unified pricing database
    pub async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(crate::core::providers::base::get_pricing_db().calculate(model, &usage))
    }

    /// Execute streaming chat completion
    pub async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<ChatChunk, ProviderError>> + Send + 'static>,
        >,
        ProviderError,
    > {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_err, self, chat_completion_stream, request, context)
    }

    /// Create embeddings
    pub async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_err, self, embeddings, request, context)
    }

    /// Create images
    pub async fn create_images(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_err, self, image_generation, request, context)
    }

    /// Get model information by ID
    pub async fn get_model(
        &self,
        model_id: &str,
    ) -> Result<Option<crate::core::types::model::ModelInfo>, ProviderError> {
        let models = self.list_models();
        for model in models {
            if model.id == model_id || model.name == model_id {
                return Ok(Some(model.clone()));
            }
        }

        Ok(None)
    }
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_enum_is_send_sync() {
        assert!(matches!(ProviderType::from("openai"), ProviderType::OpenAI));
    }

    #[tokio::test]
    async fn test_provider_capabilities_embeddings_error_names_real_provider() {
        let provider = Provider::Anthropic(
            anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
                .unwrap(),
        );

        let err = provider
            .create_embeddings(
                crate::core::types::embedding::EmbeddingRequest {
                    model: "claude-3-opus-20240229".to_string(),
                    input: crate::core::types::embedding::EmbeddingInput::Text("hello".to_string()),
                    user: None,
                    encoding_format: None,
                    dimensions: None,
                    task_type: None,
                },
                crate::core::types::context::RequestContext::default(),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                ProviderError::NotSupported {
                    provider: "anthropic",
                    ..
                }
            ),
            "expected provider-specific NotSupported, got {err}"
        );
    }

    #[tokio::test]
    async fn test_provider_capabilities_image_error_names_real_provider() {
        let provider = Provider::Anthropic(
            anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
                .unwrap(),
        );

        let err = provider
            .create_images(
                crate::core::types::image::ImageGenerationRequest {
                    prompt: "a small test image".to_string(),
                    model: Some("claude-3-opus-20240229".to_string()),
                    n: None,
                    size: None,
                    quality: None,
                    response_format: None,
                    style: None,
                    user: None,
                },
                crate::core::types::context::RequestContext::default(),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                ProviderError::NotSupported {
                    provider: "anthropic",
                    ..
                }
            ),
            "expected provider-specific NotSupported, got {err}"
        );
    }
}
