// Default router implementation.

use super::{
    CompletionOptions, CompletionResponse, CompletionStream, Message, Router,
    convert_from_chat_completion_response, convert_messages_to_chat_messages,
    convert_to_chat_completion_request, stream,
};

use crate::core::providers::{Provider, ProviderRegistry, ProviderType};
use crate::core::types::{chat::ChatRequest, context::RequestContext};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::debug;

mod dynamic_providers;
mod router_impl;

/// Default router implementation using the provider registry
pub struct DefaultRouter {
    provider_registry: Arc<ProviderRegistry>,
}

impl DefaultRouter {
    /// Helper function to find and select a provider by name with model prefix stripping
    fn select_provider_by_name<'a>(
        providers: &'a [&'a crate::core::providers::Provider],
        provider_name: &str,
        original_model: &str,
        prefix: &str,
        chat_request: &ChatRequest,
    ) -> Option<(&'a crate::core::providers::Provider, ChatRequest)> {
        if !original_model.starts_with(prefix) {
            return None;
        }

        let actual_model = original_model
            .strip_prefix(prefix)
            .unwrap_or(original_model);

        debug!(
            provider = provider_name,
            model = %actual_model,
            "Using static {} provider", provider_name
        );

        for provider in providers.iter() {
            if provider.name() == provider_name {
                let mut updated_request = chat_request.clone();
                updated_request.model = actual_model.to_string();
                return Some((provider, updated_request));
            }
        }

        None
    }

    pub async fn new() -> Result<Self> {
        let mut provider_registry = ProviderRegistry::new();

        // Add OpenAI provider if API key is available
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            use crate::core::providers::base::BaseConfig;
            use crate::core::providers::openai::OpenAIProvider;
            use crate::core::providers::openai::config::OpenAIConfig;

            let config = OpenAIConfig {
                base: BaseConfig {
                    api_key: Some(api_key),
                    api_base: Some("https://api.openai.com/v1".to_string()),
                    timeout: 60,
                    max_retries: 3,
                    headers: Default::default(),
                    organization: std::env::var("OPENAI_ORGANIZATION").ok(),
                    api_version: None,
                },
                organization: std::env::var("OPENAI_ORGANIZATION").ok(),
                project: None,
                model_mappings: Default::default(),
                features: Default::default(),
            };

            if let Ok(openai_provider) = OpenAIProvider::new(config).await {
                provider_registry.register(Provider::OpenAI(openai_provider));
            }
        }

        // Add OpenRouter provider if API key is available
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY")
            && let Some(def) = crate::core::providers::registry::get_definition("openrouter")
        {
            let config = def.to_openai_like_config(Some(&api_key), None);
            if let Ok(provider) =
                crate::core::providers::openai_like::OpenAILikeProvider::new(config).await
            {
                provider_registry.register(Provider::OpenAILike(provider));
            }
        }

        // Add Anthropic provider if API key is available
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};

            let config = AnthropicConfig::new(api_key)
                .with_base_url("https://api.anthropic.com")
                .with_experimental(false);

            let anthropic_provider = AnthropicProvider::new(config)?;
            provider_registry.register(Provider::Anthropic(anthropic_provider));
        }

        // Add DeepSeek provider if API key is available
        if let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY")
            && let Some(def) = crate::core::providers::registry::get_definition("deepseek")
        {
            let config = def.to_openai_like_config(Some(&api_key), None);
            if let Ok(provider) =
                crate::core::providers::openai_like::OpenAILikeProvider::new(config).await
            {
                provider_registry.register(Provider::OpenAILike(provider));
            }
        }

        // Add Groq provider if API key is available
        if let Ok(api_key) = std::env::var("GROQ_API_KEY")
            && let Some(def) = crate::core::providers::registry::get_definition("groq")
        {
            let config = def.to_openai_like_config(Some(&api_key), None);
            if let Ok(provider) =
                crate::core::providers::openai_like::OpenAILikeProvider::new(config).await
            {
                provider_registry.register(Provider::OpenAILike(provider));
            }
        }

        Ok(Self {
            provider_registry: Arc::new(provider_registry),
        })
    }
}

/// Fallback router for when initialization fails
pub struct ErrorRouter {
    error: String,
}

#[async_trait]
impl Router for ErrorRouter {
    async fn complete(
        &self,
        _model: &str,
        _messages: Vec<Message>,
        _options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        Err(GatewayError::internal(format!(
            "Router initialization failed: {}",
            self.error
        )))
    }

    async fn complete_stream(
        &self,
        _model: &str,
        _messages: Vec<Message>,
        _options: CompletionOptions,
    ) -> Result<CompletionStream> {
        Err(GatewayError::internal(format!(
            "Router initialization failed: {}",
            self.error
        )))
    }
}

/// Global router instance
static GLOBAL_ROUTER: OnceCell<Box<dyn Router>> = OnceCell::const_new();

/// Get or initialize the global router
async fn get_global_router() -> &'static Box<dyn Router> {
    GLOBAL_ROUTER
        .get_or_init(|| async {
            match DefaultRouter::new().await {
                Ok(router) => Box::new(router) as Box<dyn Router>,
                Err(e) => Box::new(ErrorRouter {
                    error: e.to_string(),
                }) as Box<dyn Router>,
            }
        })
        .await
}

/// Core completion function - the main entry point for all LLM calls
pub async fn completion(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionResponse> {
    let router = get_global_router().await;
    router
        .complete(model, messages, options.unwrap_or_default())
        .await
}

/// Async version of completion (though all is async in Rust)
pub async fn acompletion(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionResponse> {
    completion(model, messages, options).await
}

/// Streaming completion function
pub async fn completion_stream(
    model: &str,
    messages: Vec<Message>,
    options: Option<CompletionOptions>,
) -> Result<CompletionStream> {
    let router = get_global_router().await;
    router
        .complete_stream(model, messages, options.unwrap_or_default())
        .await
}

/// Convert ChatChunk (from provider) to CompletionChunk (for streaming API)
fn convert_chat_chunk_to_completion_chunk(
    chunk: crate::core::types::responses::ChatChunk,
) -> stream::CompletionChunk {
    stream::CompletionChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created,
        model: chunk.model,
        choices: chunk
            .choices
            .into_iter()
            .map(|c| stream::StreamChoice {
                index: c.index,
                delta: stream::StreamDelta {
                    role: c.delta.role.map(|r| r.to_string()),
                    content: c.delta.content,
                    tool_calls: None,
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
    }
}
