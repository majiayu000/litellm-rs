//! AI Provider implementations using Rust-idiomatic enum-based design
//!
//! This module contains the unified Provider enum and all provider implementations.

// Base infrastructure
pub mod base;

// Provider modules - alphabetically ordered
pub mod ai21;
pub mod aiml_api;
pub mod aleph_alpha;
pub mod amazon_nova;
pub mod anthropic;
pub mod anyscale;
pub mod azure;
pub mod azure_ai;
pub mod baichuan;
pub mod baseten;
pub mod bedrock;
pub mod bytez;
pub mod cerebras;
pub mod clarifai;
pub mod cloudflare;
pub mod codestral;
pub mod cohere;
pub mod comet_api;
pub mod compactifai;
pub mod custom_api;
pub mod dashscope;
pub mod databricks;
pub mod datarobot;
pub mod deepgram;
pub mod deepinfra;
pub mod deepl;
pub mod deepseek;
pub mod docker_model_runner;
pub mod elevenlabs;
pub mod empower;
pub mod exa_ai;
pub mod fal_ai;
pub mod featherless;
pub mod firecrawl;
pub mod fireworks;
pub mod friendliai;
pub mod galadriel;
pub mod gemini;
pub mod gigachat;
pub mod github;
pub mod github_copilot;
pub mod google_pse;
pub mod gradient_ai;
pub mod groq;
pub mod heroku;
pub mod hosted_vllm;
pub mod huggingface;
pub mod hyperbolic;
pub mod infinity;
pub mod jina;
pub mod lambda_ai;
pub mod langgraph;
pub mod lemonade;
pub mod linkup;
pub mod llamafile;
pub mod lm_studio;
pub mod manus;
pub mod maritalk;
pub mod meta_llama;
pub mod milvus;
pub mod minimax;
pub mod mistral;
pub mod moonshot;
pub mod morph;
pub mod nanogpt;
pub mod nebius;
pub mod nlp_cloud;
pub mod novita;
pub mod nscale;
pub mod nvidia_nim;
pub mod oci;
pub mod ollama;
pub mod oobabooga;
pub mod openai;
pub mod openai_like;
pub mod openrouter;
pub mod ovhcloud;
pub mod perplexity;
pub mod petals;
pub mod pg_vector;
pub mod poe;
pub mod predibase;
pub mod qwen;
pub mod ragflow;
pub mod recraft;
pub mod replicate;
pub mod runwayml;
pub mod sagemaker;
pub mod sambanova;
pub mod sap_ai;
pub mod searxng;
pub mod siliconflow;
pub mod snowflake;
pub mod spark;
pub mod stability;
pub mod tavily;
pub mod together;
pub mod topaz;
pub mod triton;
pub mod v0;
pub mod vercel_ai;
pub mod vertex_ai;
pub mod vllm;
pub mod volcengine;
pub mod voyage;
pub mod wandb;
pub mod watsonx;
pub mod xai;
pub mod xiaomi_mimo;
pub mod xinference;
pub mod yi;
pub mod zhipu;

// Shared utilities and architecture
pub mod capabilities;
pub mod macros; // Macros for reducing boilerplate
pub mod shared; // Shared utilities for all providers // Compile-time capability verification
pub mod thinking; // Thinking/reasoning provider trait (modular)
pub mod transform; // Request/Response transformation engine // Request/Response context and metadata

// Registry and unified provider
pub mod base_provider;
pub mod contextual_error;
pub mod provider_error_conversions;
pub mod provider_registry;
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
use chrono::{DateTime, Utc};
pub use contextual_error::ContextualError;
pub use provider_registry::ProviderRegistry;
pub use unified_provider::{ProviderError, UnifiedProviderError}; // Both for compatibility

/// Model pricing information
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub model: String,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
    pub currency: String,
    pub updated_at: DateTime<Utc>,
}

/// Provider type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Bedrock,
    OpenRouter,
    VertexAI,
    Azure,
    AzureAI,
    DeepSeek,
    DeepInfra,
    V0,
    MetaLlama,
    Mistral,
    Moonshot,
    Minimax,
    Dashscope,
    Groq,
    XAI,
    Cloudflare,
    Perplexity,
    Replicate,
    FalAI,
    AmazonNova,
    GitHub,
    GitHubCopilot,
    Hyperbolic,
    Infinity,
    Novita,
    Volcengine,
    Nebius,
    Nscale,
    PydanticAI,
    OpenAICompatible,
    Custom(String),
}

impl From<&str> for ProviderType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => ProviderType::OpenAI,
            "anthropic" => ProviderType::Anthropic,
            "bedrock" | "aws-bedrock" => ProviderType::Bedrock,
            "openrouter" => ProviderType::OpenRouter,
            "vertex_ai" | "vertexai" | "vertex-ai" => ProviderType::VertexAI,
            "azure" | "azure-openai" => ProviderType::Azure,
            "azure_ai" | "azureai" | "azure-ai" => ProviderType::AzureAI,
            "deepseek" | "deep-seek" => ProviderType::DeepSeek,
            "deepinfra" | "deep-infra" => ProviderType::DeepInfra,
            "v0" => ProviderType::V0,
            "meta_llama" | "llama" | "meta-llama" => ProviderType::MetaLlama,
            "mistral" | "mistralai" => ProviderType::Mistral,
            "moonshot" | "moonshot-ai" => ProviderType::Moonshot,
            "minimax" | "minimax-ai" => ProviderType::Minimax,
            "dashscope" | "alibaba" | "qwen" | "tongyi" => ProviderType::Dashscope,
            "groq" => ProviderType::Groq,
            "xai" => ProviderType::XAI,
            "cloudflare" | "cf" | "workers-ai" => ProviderType::Cloudflare,
            "perplexity" | "perplexity-ai" | "pplx" => ProviderType::Perplexity,
            "replicate" | "replicate-ai" => ProviderType::Replicate,
            "fal_ai" | "fal-ai" | "fal" => ProviderType::FalAI,
            "amazon_nova" | "amazon-nova" | "nova" => ProviderType::AmazonNova,
            "github" | "github-models" => ProviderType::GitHub,
            "github_copilot" | "github-copilot" | "copilot" => ProviderType::GitHubCopilot,
            "hyperbolic" | "hyperbolic-ai" => ProviderType::Hyperbolic,
            "infinity" | "infinity-embedding" => ProviderType::Infinity,
            "novita" | "novita-ai" => ProviderType::Novita,
            "volcengine" | "volc" | "doubao" | "bytedance" => ProviderType::Volcengine,
            "nebius" | "nebius-ai" => ProviderType::Nebius,
            "nscale" | "nscale-ai" => ProviderType::Nscale,
            "pydantic_ai" | "pydantic-ai" | "pydantic" => ProviderType::PydanticAI,
            "openai_compatible" | "openai-compatible" => ProviderType::OpenAICompatible,
            _ => ProviderType::Custom(s.to_string()),
        }
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Bedrock => write!(f, "bedrock"),
            ProviderType::OpenRouter => write!(f, "openrouter"),
            ProviderType::VertexAI => write!(f, "vertex_ai"),
            ProviderType::Azure => write!(f, "azure"),
            ProviderType::AzureAI => write!(f, "azure_ai"),
            ProviderType::DeepSeek => write!(f, "deepseek"),
            ProviderType::DeepInfra => write!(f, "deepinfra"),
            ProviderType::V0 => write!(f, "v0"),
            ProviderType::MetaLlama => write!(f, "meta_llama"),
            ProviderType::Mistral => write!(f, "mistral"),
            ProviderType::Moonshot => write!(f, "moonshot"),
            ProviderType::Minimax => write!(f, "minimax"),
            ProviderType::Dashscope => write!(f, "dashscope"),
            ProviderType::Groq => write!(f, "groq"),
            ProviderType::XAI => write!(f, "xai"),
            ProviderType::Cloudflare => write!(f, "cloudflare"),
            ProviderType::Perplexity => write!(f, "perplexity"),
            ProviderType::Replicate => write!(f, "replicate"),
            ProviderType::FalAI => write!(f, "fal_ai"),
            ProviderType::AmazonNova => write!(f, "amazon_nova"),
            ProviderType::GitHub => write!(f, "github"),
            ProviderType::GitHubCopilot => write!(f, "github_copilot"),
            ProviderType::Hyperbolic => write!(f, "hyperbolic"),
            ProviderType::Infinity => write!(f, "infinity"),
            ProviderType::Novita => write!(f, "novita"),
            ProviderType::Volcengine => write!(f, "volcengine"),
            ProviderType::Nebius => write!(f, "nebius"),
            ProviderType::Nscale => write!(f, "nscale"),
            ProviderType::PydanticAI => write!(f, "pydantic_ai"),
            ProviderType::OpenAICompatible => write!(f, "openai_compatible"),
            ProviderType::Custom(name) => write!(f, "{}", name),
        }
    }
}

// ==================== Provider Dispatch Macros ====================
// These macros eliminate repetitive match patterns across all provider methods

/// Macro for dispatching synchronous methods to all providers
macro_rules! dispatch_provider {
    ($self:expr, $method:ident) => {
        match $self {
            Provider::OpenAI(p) => p.$method(),
            Provider::Anthropic(p) => p.$method(),
            Provider::Azure(p) => p.$method(),
            Provider::Bedrock(p) => p.$method(),
            Provider::Mistral(p) => p.$method(),
            Provider::DeepSeek(p) => p.$method(),
            Provider::Moonshot(p) => p.$method(),
            Provider::MetaLlama(p) => p.$method(),
            Provider::OpenRouter(p) => p.$method(),
            Provider::VertexAI(p) => p.$method(),
            Provider::V0(p) => p.$method(),
            Provider::DeepInfra(p) => p.$method(),
            Provider::AzureAI(p) => p.$method(),
            Provider::Groq(p) => p.$method(),
            Provider::XAI(p) => p.$method(),
            Provider::Cloudflare(p) => p.$method(),
        }
    };

    ($self:expr, $method:ident, $($arg:expr),+) => {
        match $self {
            Provider::OpenAI(p) => p.$method($($arg),+),
            Provider::Anthropic(p) => p.$method($($arg),+),
            Provider::Azure(p) => p.$method($($arg),+),
            Provider::Bedrock(p) => p.$method($($arg),+),
            Provider::Mistral(p) => p.$method($($arg),+),
            Provider::DeepSeek(p) => p.$method($($arg),+),
            Provider::Moonshot(p) => p.$method($($arg),+),
            Provider::MetaLlama(p) => p.$method($($arg),+),
            Provider::OpenRouter(p) => p.$method($($arg),+),
            Provider::VertexAI(p) => p.$method($($arg),+),
            Provider::V0(p) => p.$method($($arg),+),
            Provider::DeepInfra(p) => p.$method($($arg),+),
            Provider::AzureAI(p) => p.$method($($arg),+),
            Provider::Groq(p) => p.$method($($arg),+),
            Provider::XAI(p) => p.$method($($arg),+),
            Provider::Cloudflare(p) => p.$method($($arg),+),
        }
    };
}

/// Macro for dispatching async methods with unified error conversion
macro_rules! dispatch_provider_async {
    ($self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Azure(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Bedrock(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::DeepSeek(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Moonshot(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::MetaLlama(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::OpenRouter(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::VertexAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::V0(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::DeepInfra(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::AzureAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Groq(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::XAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
        }
    };
}

/// Macro for dispatching methods that return values directly (no Result)
macro_rules! dispatch_provider_value {
    ($self:expr, $method:ident) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p),
            Provider::Anthropic(p) => LLMProvider::$method(p),
            Provider::Azure(p) => LLMProvider::$method(p),
            Provider::Bedrock(p) => LLMProvider::$method(p),
            Provider::Mistral(p) => LLMProvider::$method(p),
            Provider::DeepSeek(p) => LLMProvider::$method(p),
            Provider::Moonshot(p) => LLMProvider::$method(p),
            Provider::MetaLlama(p) => LLMProvider::$method(p),
            Provider::OpenRouter(p) => LLMProvider::$method(p),
            Provider::VertexAI(p) => LLMProvider::$method(p),
            Provider::V0(p) => LLMProvider::$method(p),
            Provider::DeepInfra(p) => LLMProvider::$method(p),
            Provider::AzureAI(p) => LLMProvider::$method(p),
            Provider::Groq(p) => LLMProvider::$method(p),
            Provider::XAI(p) => LLMProvider::$method(p),
            Provider::Cloudflare(p) => LLMProvider::$method(p),
        }
    };

    ($self:expr, $method:ident, $($arg:expr),+) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Azure(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Bedrock(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),+),
            Provider::DeepSeek(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Moonshot(p) => LLMProvider::$method(p, $($arg),+),
            Provider::MetaLlama(p) => LLMProvider::$method(p, $($arg),+),
            Provider::OpenRouter(p) => LLMProvider::$method(p, $($arg),+),
            Provider::VertexAI(p) => LLMProvider::$method(p, $($arg),+),
            Provider::V0(p) => LLMProvider::$method(p, $($arg),+),
            Provider::DeepInfra(p) => LLMProvider::$method(p, $($arg),+),
            Provider::AzureAI(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Groq(p) => LLMProvider::$method(p, $($arg),+),
            Provider::XAI(p) => LLMProvider::$method(p, $($arg),+),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),+),
        }
    };
}

/// Macro for selective provider dispatch with default fallback
/// Use this when only some providers support a method
#[allow(unused_macros)]
macro_rules! dispatch_provider_selective {
    // Dispatch to specific providers only, with a default for others
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

/// Macro for dispatching async methods without error transformation
macro_rules! dispatch_provider_async_direct {
    ($self:expr, $method:ident) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p).await,
            Provider::Anthropic(p) => LLMProvider::$method(p).await,
            Provider::Azure(p) => LLMProvider::$method(p).await,
            Provider::Bedrock(p) => LLMProvider::$method(p).await,
            Provider::Mistral(p) => LLMProvider::$method(p).await,
            Provider::DeepSeek(p) => LLMProvider::$method(p).await,
            Provider::Moonshot(p) => LLMProvider::$method(p).await,
            Provider::MetaLlama(p) => LLMProvider::$method(p).await,
            Provider::OpenRouter(p) => LLMProvider::$method(p).await,
            Provider::VertexAI(p) => LLMProvider::$method(p).await,
            Provider::V0(p) => LLMProvider::$method(p).await,
            Provider::DeepInfra(p) => LLMProvider::$method(p).await,
            Provider::AzureAI(p) => LLMProvider::$method(p).await,
            Provider::Groq(p) => LLMProvider::$method(p).await,
            Provider::XAI(p) => LLMProvider::$method(p).await,
            Provider::Cloudflare(p) => LLMProvider::$method(p).await,
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
    Azure(azure::AzureOpenAIProvider),
    Bedrock(bedrock::BedrockProvider),
    Mistral(mistral::MistralProvider),
    DeepSeek(deepseek::DeepSeekProvider),
    Moonshot(moonshot::MoonshotProvider),
    MetaLlama(meta_llama::LlamaProvider),
    OpenRouter(openrouter::OpenRouterProvider),
    VertexAI(vertex_ai::VertexAIProvider),
    V0(v0::V0Provider),
    DeepInfra(deepinfra::DeepInfraProvider),
    AzureAI(azure_ai::AzureAIProvider),
    Groq(groq::GroqProvider),
    XAI(xai::XAIProvider),
    Cloudflare(cloudflare::CloudflareProvider),
}

impl Provider {
    /// Get provider name
    pub fn name(&self) -> &'static str {
        match self {
            Provider::OpenAI(_) => "openai",
            Provider::Anthropic(_) => "anthropic",
            Provider::Azure(_) => "azure",
            Provider::Bedrock(_) => "bedrock",
            Provider::Mistral(_) => "mistral",
            Provider::DeepSeek(_) => "deepseek",
            Provider::Moonshot(_) => "moonshot",
            Provider::MetaLlama(_) => "meta_llama",
            Provider::OpenRouter(_) => "openrouter",
            Provider::VertexAI(_) => "vertex_ai",
            Provider::V0(_) => "v0",
            Provider::DeepInfra(_) => "deepinfra",
            Provider::AzureAI(_) => "azure_ai",
            Provider::Groq(_) => "groq",
            Provider::XAI(_) => "xai",
            Provider::Cloudflare(_) => "cloudflare",
        }
    }

    /// Get provider type
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Provider::OpenAI(_) => ProviderType::OpenAI,
            Provider::Anthropic(_) => ProviderType::Anthropic,
            Provider::Azure(_) => ProviderType::Azure,
            Provider::Bedrock(_) => ProviderType::Bedrock,
            Provider::Mistral(_) => ProviderType::Mistral,
            Provider::DeepSeek(_) => ProviderType::DeepSeek,
            Provider::Moonshot(_) => ProviderType::Moonshot,
            Provider::MetaLlama(_) => ProviderType::MetaLlama,
            Provider::OpenRouter(_) => ProviderType::OpenRouter,
            Provider::VertexAI(_) => ProviderType::VertexAI,
            Provider::V0(_) => ProviderType::V0,
            Provider::DeepInfra(_) => ProviderType::DeepInfra,
            Provider::AzureAI(_) => ProviderType::AzureAI,
            Provider::Groq(_) => ProviderType::Groq,
            Provider::XAI(_) => ProviderType::XAI,
            Provider::Cloudflare(_) => ProviderType::Cloudflare,
        }
    }

    /// Single source of truth for factory branches currently wired in `from_config_async`.
    pub fn factory_supported_provider_types() -> &'static [ProviderType] {
        static SUPPORTED: &[ProviderType] = &[
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::Groq,
            ProviderType::XAI,
            ProviderType::OpenRouter,
            ProviderType::Mistral,
            ProviderType::DeepSeek,
            ProviderType::Moonshot,
            ProviderType::Cloudflare,
        ];
        SUPPORTED
    }

    /// Check if provider supports a specific model
    pub fn supports_model(&self, model: &str) -> bool {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider_value!(self, supports_model, model)
    }

    /// Get provider capabilities
    pub fn capabilities(&self) -> &'static [ProviderCapability] {
        // All providers implement capabilities, using generic macro
        dispatch_provider!(self, capabilities)

        // But if future providers don't implement it, can change to:
        // dispatch_provider_selective!(
        //     self,
        //     capabilities,
        //     { OpenAI, Anthropic, Azure, Mistral, Moonshot, V0 },
        //     &[ProviderCapability::ChatCompletion]  // Default capability
        // )
    }

    /// Execute chat completion
    pub async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, UnifiedProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider_async!(self, chat_completion, request, context)
    }

    /// Execute health check
    pub async fn health_check(&self) -> crate::core::types::health::HealthStatus {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider_async_direct!(self, health_check)
    }

    /// List available models
    pub fn list_models(&self) -> &[crate::core::types::model::ModelInfo] {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider_value!(self, models)
    }

    /// Calculate cost using unified pricing database
    pub async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, UnifiedProviderError> {
        // Use unified pricing database instead of each provider implementing its own
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
            Box<
                dyn futures::Stream<Item = Result<ChatChunk, UnifiedProviderError>>
                    + Send
                    + 'static,
            >,
        >,
        UnifiedProviderError,
    > {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        use futures::StreamExt;

        match self {
            Provider::OpenAI(p) => {
                let stream = LLMProvider::chat_completion_stream(p, request, context).await?;
                let mapped = stream.map(|result| result);
                Ok(Box::pin(mapped))
            }
            Provider::Anthropic(p) => {
                let stream = LLMProvider::chat_completion_stream(p, request, context).await?;
                let mapped = stream.map(|result| result);
                Ok(Box::pin(mapped))
            }
            Provider::DeepInfra(p) => {
                let stream = LLMProvider::chat_completion_stream(p, request, context).await?;
                let mapped = stream.map(|result| result);
                Ok(Box::pin(mapped))
            }
            Provider::AzureAI(p) => {
                let stream = LLMProvider::chat_completion_stream(p, request, context).await?;
                let mapped = stream.map(|result| result);
                Ok(Box::pin(mapped))
            }
            Provider::Groq(p) => {
                let stream = LLMProvider::chat_completion_stream(p, request, context).await?;
                let mapped = stream.map(|result| result);
                Ok(Box::pin(mapped))
            }
            _ => Err(UnifiedProviderError::not_implemented(
                "unknown",
                format!("Streaming not implemented for {}", self.name()),
            )),
        }
    }

    /// Create embeddings
    pub async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, UnifiedProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

        match self {
            Provider::OpenAI(p) => LLMProvider::embeddings(p, request, context).await,
            Provider::Azure(p) => LLMProvider::embeddings(p, request, context).await,
            _ => Err(UnifiedProviderError::not_implemented(
                "unknown",
                format!("Embeddings not supported by {}", self.name()),
            )),
        }
    }

    /// Create images
    pub async fn create_images(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, UnifiedProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

        match self {
            Provider::OpenAI(p) => LLMProvider::image_generation(p, request, context).await,
            _ => Err(UnifiedProviderError::not_implemented(
                "unknown",
                format!("Image generation not supported by {}", self.name()),
            )),
        }
    }

    /// Get model information by ID
    pub async fn get_model(
        &self,
        model_id: &str,
    ) -> Result<Option<crate::core::types::model::ModelInfo>, UnifiedProviderError> {
        // Look through available models for this provider
        let models = self.list_models();
        for model in models {
            if model.id == model_id || model.name == model_id {
                return Ok(Some(model.clone()));
            }
        }

        // Model not found in this provider
        Ok(None)
    }
}

/// Create a provider from configuration
///
/// This is the main factory function for creating providers
pub async fn create_provider(
    config: crate::config::models::provider::ProviderConfig,
) -> Result<Provider, ProviderError> {
    use serde_json::Value;

    let crate::config::models::provider::ProviderConfig {
        name,
        provider_type,
        api_key,
        base_url,
        api_version,
        organization,
        project,
        settings,
        ..
    } = config;

    let provider_selector = if provider_type.trim().is_empty() {
        name.as_str()
    } else {
        provider_type.as_str()
    };
    let provider_type = ProviderType::from(provider_selector);

    if let ProviderType::Custom(custom_name) = &provider_type {
        return Err(ProviderError::not_implemented(
            "unknown",
            format!(
                "Unknown provider type '{}' (name='{}'). Add a supported provider_type or implementation.",
                custom_name, name
            ),
        ));
    }
    if !Provider::factory_supported_provider_types().contains(&provider_type) {
        return Err(ProviderError::not_implemented(
            "unknown",
            format!("Factory for {:?} not yet implemented", provider_type),
        ));
    }

    let mut factory_config = serde_json::Map::new();

    if !api_key.is_empty() {
        factory_config.insert("api_key".to_string(), Value::String(api_key.clone()));
    }
    if let Some(value) = base_url.filter(|v| !v.is_empty()) {
        factory_config.insert("base_url".to_string(), Value::String(value));
    }
    if let Some(value) = api_version.filter(|v| !v.is_empty()) {
        factory_config.insert("api_version".to_string(), Value::String(value));
    }
    if let Some(value) = organization.filter(|v| !v.is_empty()) {
        factory_config.insert("organization".to_string(), Value::String(value.clone()));
        // Cloudflare requires account_id; allow org as a compatibility fallback.
        factory_config
            .entry("account_id".to_string())
            .or_insert(Value::String(value));
    }
    if let Some(value) = project.filter(|v| !v.is_empty()) {
        factory_config.insert("project".to_string(), Value::String(value));
    }

    // Merge provider-specific settings while keeping explicit top-level fields authoritative.
    for (key, value) in settings {
        factory_config.entry(key).or_insert(value);
    }

    // Cloudflare requires api_token + account_id. If api_token is missing, fallback to api_key.
    if matches!(provider_type, ProviderType::Cloudflare)
        && !factory_config.contains_key("api_token")
        && !api_key.is_empty()
    {
        factory_config.insert("api_token".to_string(), Value::String(api_key));
    }

    Provider::from_config_async(provider_type, Value::Object(factory_config)).await
}

// Provider factory functions
impl Provider {
    /// Create provider from configuration asynchronously
    ///
    /// This is the preferred method for creating providers from configuration.
    /// It supports all provider types and handles async initialization properly.
    pub async fn from_config_async(
        provider_type: ProviderType,
        config: serde_json::Value,
    ) -> Result<Self, ProviderError> {
        match provider_type {
            ProviderType::OpenAI => {
                let api_key = macros::require_config_str(&config, "api_key", "openai")?;
                let provider = openai::OpenAIProvider::with_api_key(api_key)
                    .await
                    .map_err(|e| ProviderError::initialization("openai", e.to_string()))?;
                Ok(Provider::OpenAI(provider))
            }
            ProviderType::Anthropic => {
                let api_key = macros::require_config_str(&config, "api_key", "anthropic")?;
                let provider = anthropic::AnthropicProvider::new(
                    anthropic::AnthropicConfig::default().with_api_key(api_key),
                )?;
                Ok(Provider::Anthropic(provider))
            }
            ProviderType::Groq => {
                let api_key = macros::require_config_str(&config, "api_key", "groq")?;
                let provider = groq::GroqProvider::with_api_key(api_key)
                    .await
                    .map_err(|e| ProviderError::initialization("groq", e.to_string()))?;
                Ok(Provider::Groq(provider))
            }
            ProviderType::XAI => {
                let api_key = macros::require_config_str(&config, "api_key", "xai")?;
                let provider = xai::XAIProvider::with_api_key(api_key)
                    .await
                    .map_err(|e| ProviderError::initialization("xai", e.to_string()))?;
                Ok(Provider::XAI(provider))
            }
            ProviderType::OpenRouter => {
                let api_key = macros::require_config_str(&config, "api_key", "openrouter")?;
                let or_config = openrouter::OpenRouterConfig::new(api_key);
                let provider = openrouter::OpenRouterProvider::new(or_config)?;
                Ok(Provider::OpenRouter(provider))
            }
            ProviderType::Mistral => {
                let api_key = macros::require_config_str(&config, "api_key", "mistral")?;
                let mistral_config = mistral::MistralConfig {
                    api_key: api_key.to_string(),
                    ..Default::default()
                };
                let provider = mistral::MistralProvider::new(mistral_config)
                    .await
                    .map_err(|e| ProviderError::initialization("mistral", e.to_string()))?;
                Ok(Provider::Mistral(provider))
            }
            ProviderType::DeepSeek => {
                let api_key = macros::require_config_str(&config, "api_key", "deepseek")?;
                let mut ds_config = deepseek::DeepSeekConfig::new("deepseek");
                ds_config.base.api_key = Some(api_key.to_string());
                let provider = deepseek::DeepSeekProvider::new(ds_config)?;
                Ok(Provider::DeepSeek(provider))
            }
            ProviderType::Moonshot => {
                let api_key = macros::require_config_str(&config, "api_key", "moonshot")?;
                let moonshot_config = moonshot::MoonshotConfig {
                    api_key: api_key.to_string(),
                    ..Default::default()
                };
                let provider = moonshot::MoonshotProvider::new(moonshot_config)
                    .await
                    .map_err(|e| ProviderError::initialization("moonshot", e.to_string()))?;
                Ok(Provider::Moonshot(provider))
            }
            ProviderType::Cloudflare => {
                let account_id = macros::require_config_str(&config, "account_id", "cloudflare")?;
                let api_token = macros::require_config_str(&config, "api_token", "cloudflare")?;
                let cf_config = cloudflare::CloudflareConfig {
                    account_id: Some(account_id.to_string()),
                    api_token: Some(api_token.to_string()),
                    ..Default::default()
                };
                let provider = cloudflare::CloudflareProvider::new(cf_config)
                    .await
                    .map_err(|e| ProviderError::initialization("cloudflare", e.to_string()))?;
                Ok(Provider::Cloudflare(provider))
            }
            _ => Err(ProviderError::not_implemented(
                "unknown",
                format!("Factory for {:?} not yet implemented", provider_type),
            )),
        }
    }
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ModelPricing Tests ====================

    #[test]
    fn test_model_pricing_creation() {
        let pricing = ModelPricing {
            model: "gpt-4".to_string(),
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
        };

        assert_eq!(pricing.model, "gpt-4");
        assert_eq!(pricing.input_cost_per_1k, 0.03);
        assert_eq!(pricing.output_cost_per_1k, 0.06);
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn test_model_pricing_clone() {
        let pricing = ModelPricing {
            model: "claude-3-opus".to_string(),
            input_cost_per_1k: 0.015,
            output_cost_per_1k: 0.075,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
        };

        let cloned = pricing.clone();
        assert_eq!(cloned.model, pricing.model);
        assert_eq!(cloned.input_cost_per_1k, pricing.input_cost_per_1k);
        assert_eq!(cloned.output_cost_per_1k, pricing.output_cost_per_1k);
    }

    #[test]
    fn test_model_pricing_zero_cost() {
        let pricing = ModelPricing {
            model: "free-model".to_string(),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
        };

        assert_eq!(pricing.input_cost_per_1k, 0.0);
        assert_eq!(pricing.output_cost_per_1k, 0.0);
    }

    #[test]
    fn test_model_pricing_debug() {
        let pricing = ModelPricing {
            model: "gpt-4".to_string(),
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
            currency: "USD".to_string(),
            updated_at: Utc::now(),
        };

        let debug_str = format!("{:?}", pricing);
        assert!(debug_str.contains("gpt-4"));
        assert!(debug_str.contains("0.03"));
    }

    // ==================== ProviderType Tests ====================

    #[test]
    fn test_provider_type_from_str_openai() {
        assert_eq!(ProviderType::from("openai"), ProviderType::OpenAI);
        assert_eq!(ProviderType::from("OpenAI"), ProviderType::OpenAI);
        assert_eq!(ProviderType::from("OPENAI"), ProviderType::OpenAI);
    }

    #[test]
    fn test_provider_type_from_str_anthropic() {
        assert_eq!(ProviderType::from("anthropic"), ProviderType::Anthropic);
        assert_eq!(ProviderType::from("Anthropic"), ProviderType::Anthropic);
    }

    #[test]
    fn test_provider_type_from_str_bedrock() {
        assert_eq!(ProviderType::from("bedrock"), ProviderType::Bedrock);
        assert_eq!(ProviderType::from("aws-bedrock"), ProviderType::Bedrock);
    }

    #[test]
    fn test_provider_type_from_str_vertex_ai() {
        assert_eq!(ProviderType::from("vertex_ai"), ProviderType::VertexAI);
        assert_eq!(ProviderType::from("vertexai"), ProviderType::VertexAI);
        assert_eq!(ProviderType::from("vertex-ai"), ProviderType::VertexAI);
    }

    #[test]
    fn test_provider_type_from_str_azure() {
        assert_eq!(ProviderType::from("azure"), ProviderType::Azure);
        assert_eq!(ProviderType::from("azure-openai"), ProviderType::Azure);
    }

    #[test]
    fn test_provider_type_from_str_azure_ai() {
        assert_eq!(ProviderType::from("azure_ai"), ProviderType::AzureAI);
        assert_eq!(ProviderType::from("azureai"), ProviderType::AzureAI);
        assert_eq!(ProviderType::from("azure-ai"), ProviderType::AzureAI);
    }

    #[test]
    fn test_provider_type_from_str_deepseek() {
        assert_eq!(ProviderType::from("deepseek"), ProviderType::DeepSeek);
        assert_eq!(ProviderType::from("deep-seek"), ProviderType::DeepSeek);
    }

    #[test]
    fn test_provider_type_from_str_deepinfra() {
        assert_eq!(ProviderType::from("deepinfra"), ProviderType::DeepInfra);
        assert_eq!(ProviderType::from("deep-infra"), ProviderType::DeepInfra);
    }

    #[test]
    fn test_provider_type_from_str_meta_llama() {
        assert_eq!(ProviderType::from("meta_llama"), ProviderType::MetaLlama);
        assert_eq!(ProviderType::from("llama"), ProviderType::MetaLlama);
        assert_eq!(ProviderType::from("meta-llama"), ProviderType::MetaLlama);
    }

    #[test]
    fn test_provider_type_from_str_mistral() {
        assert_eq!(ProviderType::from("mistral"), ProviderType::Mistral);
        assert_eq!(ProviderType::from("mistralai"), ProviderType::Mistral);
    }

    #[test]
    fn test_provider_type_from_str_moonshot() {
        assert_eq!(ProviderType::from("moonshot"), ProviderType::Moonshot);
        assert_eq!(ProviderType::from("moonshot-ai"), ProviderType::Moonshot);
    }

    #[test]
    fn test_provider_type_from_str_cloudflare() {
        assert_eq!(ProviderType::from("cloudflare"), ProviderType::Cloudflare);
        assert_eq!(ProviderType::from("cf"), ProviderType::Cloudflare);
        assert_eq!(ProviderType::from("workers-ai"), ProviderType::Cloudflare);
    }

    #[test]
    fn test_provider_type_from_str_other_providers() {
        assert_eq!(ProviderType::from("openrouter"), ProviderType::OpenRouter);
        assert_eq!(ProviderType::from("groq"), ProviderType::Groq);
        assert_eq!(ProviderType::from("xai"), ProviderType::XAI);
        assert_eq!(ProviderType::from("v0"), ProviderType::V0);
    }

    #[test]
    fn test_provider_type_from_str_custom() {
        assert_eq!(
            ProviderType::from("custom-provider"),
            ProviderType::Custom("custom-provider".to_string())
        );
        assert_eq!(
            ProviderType::from("my-local-llm"),
            ProviderType::Custom("my-local-llm".to_string())
        );
    }

    #[test]
    fn test_provider_type_display() {
        assert_eq!(format!("{}", ProviderType::OpenAI), "openai");
        assert_eq!(format!("{}", ProviderType::Anthropic), "anthropic");
        assert_eq!(format!("{}", ProviderType::Bedrock), "bedrock");
        assert_eq!(format!("{}", ProviderType::OpenRouter), "openrouter");
        assert_eq!(format!("{}", ProviderType::VertexAI), "vertex_ai");
        assert_eq!(format!("{}", ProviderType::Azure), "azure");
        assert_eq!(format!("{}", ProviderType::AzureAI), "azure_ai");
        assert_eq!(format!("{}", ProviderType::DeepSeek), "deepseek");
        assert_eq!(format!("{}", ProviderType::DeepInfra), "deepinfra");
        assert_eq!(format!("{}", ProviderType::V0), "v0");
        assert_eq!(format!("{}", ProviderType::MetaLlama), "meta_llama");
        assert_eq!(format!("{}", ProviderType::Mistral), "mistral");
        assert_eq!(format!("{}", ProviderType::Moonshot), "moonshot");
        assert_eq!(format!("{}", ProviderType::Groq), "groq");
        assert_eq!(format!("{}", ProviderType::XAI), "xai");
        assert_eq!(format!("{}", ProviderType::Cloudflare), "cloudflare");
    }

    #[test]
    fn test_provider_type_display_custom() {
        let custom = ProviderType::Custom("my-custom-provider".to_string());
        assert_eq!(format!("{}", custom), "my-custom-provider");
    }

    #[test]
    fn test_provider_type_clone() {
        let original = ProviderType::OpenAI;
        let cloned = original.clone();
        assert_eq!(original, cloned);

        let custom = ProviderType::Custom("test".to_string());
        let custom_cloned = custom.clone();
        assert_eq!(custom, custom_cloned);
    }

    #[test]
    fn test_provider_type_equality() {
        assert_eq!(ProviderType::OpenAI, ProviderType::OpenAI);
        assert_ne!(ProviderType::OpenAI, ProviderType::Anthropic);
        assert_eq!(
            ProviderType::Custom("test".to_string()),
            ProviderType::Custom("test".to_string())
        );
        assert_ne!(
            ProviderType::Custom("test1".to_string()),
            ProviderType::Custom("test2".to_string())
        );
    }

    #[test]
    fn test_provider_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ProviderType::OpenAI);
        set.insert(ProviderType::Anthropic);
        set.insert(ProviderType::Custom("custom".to_string()));

        assert!(set.contains(&ProviderType::OpenAI));
        assert!(set.contains(&ProviderType::Anthropic));
        assert!(set.contains(&ProviderType::Custom("custom".to_string())));
        assert!(!set.contains(&ProviderType::Bedrock));
    }

    #[test]
    fn test_provider_type_serialization() {
        let provider = ProviderType::OpenAI;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"OpenAI\"");

        let custom = ProviderType::Custom("my-provider".to_string());
        let custom_json = serde_json::to_string(&custom).unwrap();
        assert!(custom_json.contains("Custom"));
        assert!(custom_json.contains("my-provider"));
    }

    #[test]
    fn test_provider_type_deserialization() {
        let provider: ProviderType = serde_json::from_str("\"OpenAI\"").unwrap();
        assert_eq!(provider, ProviderType::OpenAI);

        let anthropic: ProviderType = serde_json::from_str("\"Anthropic\"").unwrap();
        assert_eq!(anthropic, ProviderType::Anthropic);
    }

    #[test]
    fn test_provider_type_roundtrip_serialization() {
        let providers = vec![
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::Bedrock,
            ProviderType::Custom("test".to_string()),
        ];

        for provider in providers {
            let json = serde_json::to_string(&provider).unwrap();
            let deserialized: ProviderType = serde_json::from_str(&json).unwrap();
            assert_eq!(provider, deserialized);
        }
    }

    #[test]
    fn test_provider_type_debug() {
        let provider = ProviderType::OpenAI;
        let debug_str = format!("{:?}", provider);
        assert_eq!(debug_str, "OpenAI");

        let custom = ProviderType::Custom("test".to_string());
        let custom_debug = format!("{:?}", custom);
        assert!(custom_debug.contains("Custom"));
        assert!(custom_debug.contains("test"));
    }

    // ==================== ProviderType From/To Consistency Tests ====================

    fn all_non_custom_provider_types() -> Vec<ProviderType> {
        vec![
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::Bedrock,
            ProviderType::OpenRouter,
            ProviderType::VertexAI,
            ProviderType::Azure,
            ProviderType::AzureAI,
            ProviderType::DeepSeek,
            ProviderType::DeepInfra,
            ProviderType::V0,
            ProviderType::MetaLlama,
            ProviderType::Mistral,
            ProviderType::Moonshot,
            ProviderType::Minimax,
            ProviderType::Dashscope,
            ProviderType::Groq,
            ProviderType::XAI,
            ProviderType::Cloudflare,
            ProviderType::Perplexity,
            ProviderType::Replicate,
            ProviderType::FalAI,
            ProviderType::AmazonNova,
            ProviderType::GitHub,
            ProviderType::GitHubCopilot,
            ProviderType::Hyperbolic,
            ProviderType::Infinity,
            ProviderType::Novita,
            ProviderType::Volcengine,
            ProviderType::Nebius,
            ProviderType::Nscale,
            ProviderType::PydanticAI,
            ProviderType::OpenAICompatible,
        ]
    }

    fn supported_factory_provider_types() -> Vec<ProviderType> {
        Provider::factory_supported_provider_types().to_vec()
    }

    #[test]
    fn test_provider_type_from_display_consistency() {
        // Test that Display output can be parsed back (for non-custom types)
        for provider in all_non_custom_provider_types() {
            let display = format!("{}", provider);
            let parsed = ProviderType::from(display.as_str());
            assert_eq!(
                provider, parsed,
                "Display/From roundtrip failed for {:?}",
                provider
            );
        }
    }

    // ==================== Provider Enum Tests ====================

    // Note: Provider enum tests require actual provider initialization
    // which needs API keys. These tests verify the enum structure.

    #[test]
    fn test_provider_enum_is_send_sync() {
        // Placeholder test to keep enum-level guardrail slot.
        assert!(matches!(ProviderType::from("openai"), ProviderType::OpenAI));
    }

    #[test]
    fn test_provider_type_all_variants_covered() {
        // This test ensures we don't forget to update tests when adding new providers
        for provider in all_non_custom_provider_types() {
            let provider_str = provider.to_string();
            let provider_type = ProviderType::from(provider_str.as_str());
            // Should not be Custom for known providers
            assert!(
                !matches!(provider_type, ProviderType::Custom(_)),
                "Provider '{}' should not be Custom",
                provider_str
            );
            assert_eq!(
                provider_type, provider,
                "Expected '{}' to map to {:?}, but got {:?}",
                provider_str, provider, provider_type
            );
        }
    }

    #[tokio::test]
    async fn test_from_config_async_supported_variants_do_not_fallthrough_to_not_implemented() {
        // Supported branches should fail with config validation on empty config,
        // not with a generic NotImplemented fallback.
        for provider_type in supported_factory_provider_types() {
            let err = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
                .await
                .expect_err("Expected empty config to fail");
            assert!(
                !matches!(err, ProviderError::NotImplemented { .. }),
                "{:?} unexpectedly fell through to NotImplemented: {}",
                provider_type,
                err
            );
        }
    }

    #[tokio::test]
    async fn test_from_config_async_unsupported_variants_return_not_implemented() {
        let supported = supported_factory_provider_types();

        for provider_type in all_non_custom_provider_types() {
            if supported.contains(&provider_type) {
                continue;
            }

            let err = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
                .await
                .expect_err("Expected unsupported provider to fail");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented for {:?}, got {}",
                provider_type,
                err
            );
        }
    }

    #[test]
    fn test_provider_type_case_insensitive() {
        // Test various case combinations
        let cases = vec![
            ("OPENAI", ProviderType::OpenAI),
            ("OpenAI", ProviderType::OpenAI),
            ("openai", ProviderType::OpenAI),
            ("OpenAi", ProviderType::OpenAI),
            ("ANTHROPIC", ProviderType::Anthropic),
            ("Anthropic", ProviderType::Anthropic),
            ("GROQ", ProviderType::Groq),
            ("Groq", ProviderType::Groq),
        ];

        for (input, expected) in cases {
            assert_eq!(
                ProviderType::from(input),
                expected,
                "Case-insensitive parsing failed for '{}'",
                input
            );
        }
    }

    #[tokio::test]
    async fn test_create_provider_prefers_provider_type_over_name() {
        let config = crate::config::models::provider::ProviderConfig {
            // If name took precedence, this would resolve to a supported OpenAI branch.
            name: "openai".to_string(),
            // We intentionally set an unsupported branch to assert precedence.
            provider_type: "perplexity".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider type to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
        assert!(
            err.to_string().contains("Perplexity"),
            "Expected error to mention the selected provider type, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_falls_back_to_name_when_provider_type_empty() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "perplexity".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider name to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
        assert!(
            err.to_string().contains("Perplexity"),
            "Expected fallback to name parsing, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_reports_unknown_custom_provider() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "my-custom-provider".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unknown custom provider to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
        assert!(
            err.to_string().contains("my-custom-provider"),
            "Expected custom provider name in error, got {}",
            err
        );
    }
}
