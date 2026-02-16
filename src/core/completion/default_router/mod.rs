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
        if let Ok(api_key) = std::env::var("OPENROUTER_API_KEY") {
            use crate::core::providers::openrouter::{OpenRouterConfig, OpenRouterProvider};

            let api_key = api_key.trim().to_string();

            let config = OpenRouterConfig {
                api_key,
                base_url: "https://openrouter.ai/api/v1".to_string(),
                site_url: std::env::var("OPENROUTER_HTTP_REFERER").ok(),
                site_name: std::env::var("OPENROUTER_X_TITLE").ok(),
                timeout_seconds: 60,
                max_retries: 3,
                extra_params: Default::default(),
            };

            if let Ok(openrouter_provider) = OpenRouterProvider::new(config) {
                provider_registry.register(Provider::OpenRouter(openrouter_provider));
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

        // Add VertexAI provider if service account is available
        if std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok() {
            use crate::core::providers::vertex_ai::{
                VertexAIProvider, VertexAIProviderConfig, VertexCredentials,
            };

            let config = VertexAIProviderConfig {
                project_id: std::env::var("GOOGLE_PROJECT_ID")
                    .unwrap_or_else(|_| "default-project".to_string()),
                location: std::env::var("GOOGLE_LOCATION")
                    .unwrap_or_else(|_| "us-central1".to_string()),
                api_version: "v1".to_string(),
                credentials: VertexCredentials::ApplicationDefault,
                api_base: None,
                timeout_seconds: 60,
                max_retries: 3,
                enable_experimental: false,
            };

            if let Ok(vertex_provider) = VertexAIProvider::new(config).await {
                provider_registry.register(Provider::VertexAI(vertex_provider));
            }
        }

        // Add DeepSeek provider if API key is available
        if let Ok(_api_key) = std::env::var("DEEPSEEK_API_KEY") {
            use crate::core::providers::deepseek::{DeepSeekConfig, DeepSeekProvider};

            let config = DeepSeekConfig::from_env();

            if let Ok(deepseek_provider) = DeepSeekProvider::new(config) {
                provider_registry.register(Provider::DeepSeek(deepseek_provider));
            }
        }

        // Add Groq provider if API key is available
        if let Ok(api_key) = std::env::var("GROQ_API_KEY") {
            use crate::core::providers::groq::{GroqConfig, GroqProvider};

            let config = GroqConfig::from_env().with_api_key(api_key);

            if let Ok(groq_provider) = GroqProvider::new(config).await {
                provider_registry.register(Provider::Groq(groq_provider));
            }
        }

        // Add Bedrock provider if AWS credentials are available
        if let (Ok(access_key), Ok(secret_key)) = (
            std::env::var("AWS_ACCESS_KEY_ID"),
            std::env::var("AWS_SECRET_ACCESS_KEY"),
        ) {
            use crate::core::providers::bedrock::{BedrockConfig, BedrockProvider};

            let region = std::env::var("AWS_REGION")
                .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| "us-east-1".to_string());

            let config = BedrockConfig {
                aws_access_key_id: access_key,
                aws_secret_access_key: secret_key,
                aws_session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
                aws_region: region,
                timeout_seconds: 30,
                max_retries: 3,
            };

            if let Ok(bedrock_provider) = BedrockProvider::new(config).await {
                provider_registry.register(Provider::Bedrock(bedrock_provider));
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
