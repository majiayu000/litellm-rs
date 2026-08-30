//! AI Provider implementations using Rust-idiomatic enum-based design.
//!
//! This module contains the closed `Provider` enum used by router deployments
//! plus the built-in provider implementations wired into that enum. Implementing
//! `LLMProvider` alone does not make a provider routeable; new routed providers
//! must be added to the enum, dispatch arms, and factory wiring.
// Base infrastructure
pub mod base;
// Provider modules - alphabetically ordered
// Tier 1 providers removed in favor of registry/catalog.rs are commented with their tier.
// aiml_api: Tier 1 -> registry/catalog.rs
// aleph_alpha: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
#[cfg_attr(
    not(test),
    deprecated(since = "0.6.0", note = "use catalog amazon_nova before 0.7")
)]
pub mod amazon_nova;
pub mod anthropic;
// anyscale: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extra")]
pub mod azure;
#[cfg(feature = "providers-extra")]
pub mod azure_ai;
// baichuan: Tier 1 -> registry/catalog.rs
pub mod bedrock;
// bytez: Tier 1 -> registry/catalog.rs
// cerebras: Tier 1 -> registry/catalog.rs
pub mod cloudflare;
#[cfg(feature = "providers-extended")]
pub mod cohere;
pub mod databricks;
// comet_api: Tier 1 -> registry/catalog.rs
// compactifai: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
#[cfg_attr(
    not(test),
    deprecated(
        since = "0.6.0",
        note = "use a catalog or typed provider before 0.7.0 removal"
    )
)]
pub mod custom_api;
// dashscope: Tier 1 -> registry/catalog.rs
// deepinfra: Tier 1 -> registry/catalog.rs
// deepseek: Tier 1 -> registry/catalog.rs
// docker_model_runner: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod fal_ai;
// featherless: Tier 1 -> registry/catalog.rs
// fireworks: Tier 1 -> registry/catalog.rs
// friendliai: Tier 1 -> registry/catalog.rs
// galadriel: Tier 1 -> registry/catalog.rs
mod enterprise;
#[cfg(any(feature = "providers-extended", feature = "providers-extra"))]
pub mod gemini;
#[cfg(feature = "providers-extended")]
pub mod github;
#[cfg(feature = "providers-extended")]
pub mod github_copilot;
pub(crate) mod google_error;
pub(crate) mod google_tool_loop;
// groq: Tier 1 -> registry/catalog.rs
// heroku: Tier 1 -> registry/catalog.rs
// hosted_vllm: Tier 1 -> registry/catalog.rs
// hyperbolic: Tier 1 -> registry/catalog.rs
// infinity: Tier 1 -> registry/catalog.rs
// lambda_ai: Tier 1 -> registry/catalog.rs
// lemonade: Tier 1 -> registry/catalog.rs
// linkup: Tier 1 -> registry/catalog.rs
// llamafile: Tier 1 -> registry/catalog.rs
// lm_studio: Tier 1 -> registry/catalog.rs
// maritalk: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extra")]
pub mod meta_llama;
// minimax: Tier 1 -> registry/catalog.rs
pub mod mistral;
// moonshot: Tier 1 -> registry/catalog.rs
// nanogpt: Tier 1 -> registry/catalog.rs
// nebius: Tier 1 -> registry/catalog.rs
// novita: Tier 1 -> registry/catalog.rs
// nscale: Tier 1 -> registry/catalog.rs
// nvidia_nim: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod ollama;
// oobabooga: Tier 1 -> registry/catalog.rs
pub mod oci;
pub mod openai;
pub mod openai_like;
// openrouter: Tier 1 -> registry/catalog.rs
// ovhcloud: Tier 1 -> registry/catalog.rs
// perplexity: Tier 1 -> registry/catalog.rs
// poe: Tier 1 -> registry/catalog.rs
// qwen: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extended")]
pub mod replicate;
pub mod sagemaker;
pub mod snowflake;
// sambanova: Tier 1 -> registry/catalog.rs
// siliconflow: Tier 1 -> registry/catalog.rs
// together: Tier 1 -> registry/catalog.rs
#[cfg(feature = "providers-extra")]
pub mod v0;
#[cfg(feature = "providers-extra")]
pub mod vertex_ai;
pub mod voyage;
// vllm: Tier 1 -> registry/catalog.rs
// volcengine: Tier 1 -> registry/catalog.rs
// wandb: Tier 1 -> registry/catalog.rs
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
// Provider type enumeration (extracted from this module)
pub mod provider_type;
pub use provider_type::ProviderType;
// Factory: create_provider, from_config_async, config builders
pub mod factory;
pub use factory::{create_provider, is_provider_selector_supported};
// Registry and unified provider
mod chat_continuation;
pub mod contextual_error;
pub mod failure;
pub mod provider_error_conversions;
pub mod provider_registry;
pub mod registry; // Data-driven Tier 1 provider catalog
mod rerank_dispatch;
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
pub(crate) use chat_continuation::{
    AnthropicContentBlockOrder, ChatContinuationRequest, ChatContinuationResponse,
    ChatMessageContinuation,
};
pub use contextual_error::ContextualError;
pub use failure::{ProviderFailureFacts, ProviderRetryHint};
pub use provider_registry::ProviderRegistry;
pub use unified_provider::ProviderError;
#[derive(Debug, Clone)]
pub(crate) struct GeminiNativeRequest {
    pub(crate) api_version: String,
    pub(crate) model: String,
    pub(crate) method: &'static str,
    pub(crate) stream: bool,
    pub(crate) body: serde_json::Value,
}
pub(crate) fn gemini_native_url(
    base_url: &str,
    api_key: &str,
    request: &GeminiNativeRequest,
) -> Result<reqwest::Url, ProviderError> {
    if !matches!(request.api_version.as_str(), "v1" | "v1beta")
        || !matches!(request.method, "generateContent" | "streamGenerateContent")
        || request.model.is_empty()
        || !request.model.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(ProviderError::invalid_request(
            "gemini_proxy",
            "invalid Gemini native route segment",
        ));
    }
    let mut url = reqwest::Url::parse(&format!(
        "{}/{}/models/{}:{}",
        base_url.trim_end_matches('/'),
        request.api_version,
        request.model,
        request.method
    ))
    .map_err(|_| ProviderError::configuration("gemini_proxy", "invalid Gemini API base URL"))?;
    let mut query = url.query_pairs_mut();
    if request.stream {
        query.append_pair("alt", "sse");
    }
    query.append_pair("key", api_key);
    drop(query);
    Ok(url)
}
pub(crate) async fn gemini_response_or_provider_error(
    response: reqwest::Response,
    api_key: &str,
) -> Result<reqwest::Response, ProviderError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let header_retry = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = base::read_streaming_error_body(response)
        .await
        .map_err(|error| error.into_provider_error("gemini_proxy"))?;
    let body = redact_gemini_key(&body, api_key);
    let message = if body.trim().is_empty() {
        format!("Gemini upstream returned HTTP {status}")
    } else {
        format!("Gemini upstream returned HTTP {status}: {body}")
    };
    Err(if status == 429 {
        ProviderError::rate_limit_with_retry(
            "gemini_proxy",
            message,
            header_retry.or_else(|| shared::parse_retry_after_from_body(&body)),
        )
    } else {
        ProviderError::api_error("gemini_proxy", status, message)
    })
}
fn redact_gemini_key(body: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        return body.to_string();
    }
    let encoded: String = url::form_urlencoded::byte_serialize(api_key.as_bytes()).collect();
    body.replace(api_key, "[REDACTED]")
        .replace(&encoded, "[REDACTED]")
}
pub(crate) fn gemini_transport_error(is_timeout: bool) -> ProviderError {
    let message = "Gemini upstream request failed";
    if is_timeout {
        return ProviderError::timeout("gemini_proxy", message);
    }
    ProviderError::network("gemini_proxy", message)
}
// Keep every Provider variant in each dispatch arm below.
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
            Provider::Bedrock(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(p) => p.$method($($arg),*),
            Provider::Mistral(p) => p.$method($($arg),*),
            Provider::Cloudflare(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(p) => p.$method($($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(p) => p.$method($($arg),*),
            Provider::Enterprise(p) => p.$method($($arg),*),
            Provider::OpenAILike(p) => p.$method($($arg),*),
            Provider::Voyage(p) => p.$method($($arg),*),
        }
    };

    (@expand async_err, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Bedrock(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(p) => LLMProvider::$method(p.as_ref(), $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Enterprise(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::OpenAILike(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Voyage(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
        }
    };

    (@expand value, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Bedrock(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(p) => LLMProvider::$method(p.as_ref(), $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Mistral(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Cloudflare(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(p) => LLMProvider::$method(p, $($arg),*),
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Enterprise(p) => LLMProvider::$method(p, $($arg),*),
            Provider::OpenAILike(p) => LLMProvider::$method(p, $($arg),*),
            Provider::Voyage(p) => LLMProvider::$method(p, $($arg),*),
        }
    };

    (@expand async_direct, $self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p).await,
            Provider::Anthropic(p) => LLMProvider::$method(p).await,
            Provider::Bedrock(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extra")]
            Provider::Azure(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(p) => LLMProvider::$method(p.as_ref()).await,
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(p) => LLMProvider::$method(p).await,
            Provider::Mistral(p) => LLMProvider::$method(p).await,
            Provider::Cloudflare(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(p) => LLMProvider::$method(p).await,
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(p) => LLMProvider::$method(p).await,
            Provider::Enterprise(p) => LLMProvider::$method(p).await,
            Provider::OpenAILike(p) => LLMProvider::$method(p).await,
            Provider::Voyage(p) => LLMProvider::$method(p).await,
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

mod audio_dispatch;
mod capability_dispatch;
mod model_health_check;
pub mod model_identity;

/// Unified built-in Provider enum (Rust-idiomatic design).
///
/// This enum provides zero-cost abstractions and type safety for all providers.
/// Each variant contains a concrete provider implementation. Router
/// deployments dispatch through this closed enum; third-party `LLMProvider`
/// implementations are not routeable without crate changes that add enum,
/// dispatch, and factory support.
#[derive(Debug, Clone)]
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Bedrock(bedrock::BedrockProvider),
    #[cfg(feature = "providers-extra")]
    Azure(azure::AzureOpenAIProvider),
    #[cfg(feature = "providers-extra")]
    AzureAI(azure_ai::AzureAIProvider),
    #[cfg(feature = "providers-extra")]
    VertexAI(vertex_ai::VertexAIProvider),
    #[cfg(feature = "providers-extended")]
    Gemini(std::sync::Arc<gemini::GeminiProvider>),
    #[cfg(feature = "providers-extended")]
    GitHubCopilot(github_copilot::GitHubCopilotProvider),
    #[cfg(feature = "providers-extended")]
    Ollama(ollama::OllamaProvider),
    #[cfg(feature = "providers-extended")]
    FalAI(fal_ai::FalAIProvider),
    Mistral(mistral::MistralProvider),
    Cloudflare(cloudflare::CloudflareProvider),
    #[cfg(feature = "providers-extended")]
    Cohere(cohere::CohereProvider),
    #[cfg(feature = "providers-extended")]
    Replicate(replicate::ReplicateProvider),
    Enterprise(enterprise::EnterpriseProvider),
    /// Tier 1: data-driven OpenAI-compatible providers (groq, together, fireworks, etc.)
    OpenAILike(openai_like::OpenAILikeProvider),
    Voyage(voyage::VoyageProvider),
}

impl Provider {
    pub(crate) fn bind_deployment_model_identity(
        &mut self,
        identity: model_identity::DeploymentModelIdentity,
        pricing: std::sync::Arc<crate::core::pricing_service::PricingService>,
    ) -> Result<(), String> {
        let binding = model_identity::DeploymentProviderBinding::new(identity, pricing);
        match self {
            Provider::OpenAI(provider) => provider.model_identity = Some(binding),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(provider) => provider.model_identity = Some(binding),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(provider) => provider.model_identity = Some(binding),
            Provider::OpenAILike(provider) => provider.model_identity = Some(binding),
            _ => {
                return Err(format!(
                    "provider '{}' does not use OpenAI-family deployment identity",
                    self.name()
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn deployment_model_identity(
        &self,
    ) -> Option<&model_identity::DeploymentModelIdentity> {
        match self {
            Provider::OpenAI(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::identity),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::identity),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::identity),
            Provider::OpenAILike(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::identity),
            _ => None,
        }
    }

    pub(crate) fn runtime_pricing(
        &self,
    ) -> Option<std::sync::Arc<crate::core::pricing_service::PricingService>> {
        match self {
            Provider::OpenAI(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::pricing),
            #[cfg(feature = "providers-extra")]
            Provider::Azure(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::pricing),
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::pricing),
            Provider::OpenAILike(provider) => provider
                .model_identity
                .as_ref()
                .map(model_identity::DeploymentProviderBinding::pricing),
            _ => None,
        }
        .map(std::sync::Arc::clone)
    }

    pub(crate) fn legacy_openai_model_target<'a>(&'a self, model: &'a str) -> Option<&'a str> {
        match self {
            Provider::OpenAI(provider) => provider
                .config
                .model_mappings
                .get(model)
                .map(String::as_str),
            _ => None,
        }
    }

    pub(crate) async fn gemini_generate_content(
        &self,
        request: GeminiNativeRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        match self {
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(provider) => provider.gemini_generate_content(request).await,
            Provider::OpenAILike(provider) => provider.gemini_generate_content(request).await,
            _ => Err(ProviderError::not_supported(
                "provider",
                "Gemini native generateContent",
            )),
        }
    }

    /// Get provider name
    pub fn name(&self) -> &str {
        match self {
            Provider::OpenAI(p) => {
                use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
                p.name()
            }
            Provider::Anthropic(_) => "anthropic",
            Provider::Bedrock(_) => "bedrock",
            #[cfg(feature = "providers-extra")]
            Provider::Azure(_) => "azure",
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(_) => "azure_ai",
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(_) => "vertex_ai",
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(_) => "gemini",
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(_) => "github_copilot",
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(_) => "ollama",
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(_) => "fal_ai",
            Provider::Mistral(_) => "mistral",
            Provider::Cloudflare(_) => "cloudflare",
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(_) => "cohere",
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(_) => "replicate",
            Provider::Enterprise(p) => p.name(),
            Provider::OpenAILike(p) => {
                use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
                p.name()
            }
            Provider::Voyage(_) => "voyage",
        }
    }

    /// Get provider type
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Provider::OpenAI(_) => ProviderType::OpenAI,
            Provider::Anthropic(_) => ProviderType::Anthropic,
            Provider::Bedrock(_) => ProviderType::Bedrock,
            #[cfg(feature = "providers-extra")]
            Provider::Azure(_) => ProviderType::Azure,
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(_) => ProviderType::AzureAI,
            #[cfg(feature = "providers-extra")]
            Provider::VertexAI(_) => ProviderType::VertexAI,
            #[cfg(feature = "providers-extended")]
            Provider::Gemini(_) => ProviderType::Gemini,
            #[cfg(feature = "providers-extended")]
            Provider::GitHubCopilot(_) => ProviderType::GitHubCopilot,
            #[cfg(feature = "providers-extended")]
            Provider::Ollama(_) => ProviderType::Ollama,
            #[cfg(feature = "providers-extended")]
            Provider::FalAI(_) => ProviderType::FalAI,
            Provider::Mistral(_) => ProviderType::Mistral,
            Provider::Cloudflare(_) => ProviderType::Cloudflare,
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(_) => ProviderType::Cohere,
            #[cfg(feature = "providers-extended")]
            Provider::Replicate(_) => ProviderType::Replicate,
            Provider::Enterprise(p) => p.provider_type(),
            Provider::OpenAILike(_) => ProviderType::OpenAICompatible,
            Provider::Voyage(_) => ProviderType::Voyage,
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

    /// Check if this provider declares a runtime capability.
    pub fn supports_capability(&self, capability: &ProviderCapability) -> bool {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(value, self, supports_capability, capability)
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

    pub(crate) async fn chat_completion_with_continuation(
        &self,
        envelope: ChatContinuationRequest,
        context: RequestContext,
        opt_in: bool,
    ) -> Result<ChatContinuationResponse, ProviderError> {
        if !opt_in && !envelope.has_continuation() {
            let (request, _) = envelope.into_parts();
            let response = self.chat_completion(request, context).await?;
            let extensions = vec![ChatMessageContinuation::new(); response.choices.len()];
            return ChatContinuationResponse::new(response, extensions);
        }
        match self {
            Provider::Anthropic(provider) => provider.chat_with_continuation(envelope).await,
            _ => Err(ProviderError::not_supported(
                "router",
                "Anthropic continuation is only supported by the Anthropic provider",
            )),
        }
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
        if let Some(result) = model_identity::calculate_managed_provider_cost(
            self,
            model,
            input_tokens,
            output_tokens,
        ) {
            return result;
        }
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        let model = self.strip_provider_prefix(model);
        dispatch_provider!(
            async_err,
            self,
            calculate_cost,
            model,
            input_tokens,
            output_tokens
        )
    }

    fn strip_provider_prefix<'a>(&self, model: &'a str) -> &'a str {
        model
            .strip_prefix(self.name())
            .and_then(|model| model.strip_prefix('/'))
            .unwrap_or(model)
    }

    /// Execute streaming chat completion.
    ///
    /// Route selection must confirm `ProviderCapability::ChatCompletionStream`
    /// before calling this optional dispatch method.
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

    /// Create embeddings.
    ///
    /// Route selection must confirm `ProviderCapability::Embeddings` before
    /// calling this optional dispatch method.
    pub async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        dispatch_provider!(async_err, self, embeddings, request, context)
    }

    /// Create images.
    ///
    /// Route selection must confirm `ProviderCapability::ImageGeneration`
    /// before calling this optional dispatch method.
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

#[cfg(test)]
#[path = "provider_tests.rs"]
mod tests;
