// Dynamic provider creation methods for DefaultRouter.

use super::*;

impl DefaultRouter {
    /// Dynamic provider creation (Python LiteLLM style)
    /// Creates providers on-demand based on model name and provided options
    pub(super) async fn try_dynamic_provider_creation(
        &self,
        chat_request: &ChatRequest,
        context: RequestContext,
        options: &CompletionOptions,
    ) -> Result<Option<CompletionResponse>> {
        let model = &chat_request.model;

        // Only proceed if user provided an API key
        let api_key = match &options.api_key {
            Some(key) => key.clone(),
            None => return Ok(None),
        };

        // Determine provider type from model name
        let (provider_type, actual_model, api_base) = if model.starts_with("openrouter/") {
            let actual_model = model.strip_prefix("openrouter/").unwrap_or(model);
            let api_base = options
                .api_base
                .clone()
                .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
            ("openrouter", actual_model, api_base)
        } else if model.starts_with("anthropic/") {
            let actual_model = model.strip_prefix("anthropic/").unwrap_or(model);
            let api_base = options
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            ("anthropic", actual_model, api_base)
        } else if model.starts_with("deepseek/") {
            let actual_model = model.strip_prefix("deepseek/").unwrap_or(model);
            let api_base = options
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.deepseek.com".to_string());
            ("deepseek", actual_model, api_base)
        } else if model.starts_with("azure_ai/") || model.starts_with("azure-ai/") {
            let actual_model = model
                .strip_prefix("azure_ai/")
                .or_else(|| model.strip_prefix("azure-ai/"))
                .unwrap_or(model);
            let api_base = options
                .api_base
                .clone()
                .or_else(|| std::env::var("AZURE_AI_API_BASE").ok())
                .unwrap_or_else(|| "https://api.azure.com".to_string());
            ("azure_ai", actual_model, api_base)
        } else if model.starts_with("openai/") {
            let actual_model = model.strip_prefix("openai/").unwrap_or(model);
            let api_base = options
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            ("openai", actual_model, api_base)
        } else {
            // For models without provider prefix, try to infer or use custom api_base
            if let Some(api_base) = &options.api_base {
                ("openai-compatible", model.as_str(), api_base.clone())
            } else {
                return Ok(None);
            }
        };

        debug!(
            provider_type = %provider_type,
            model = %actual_model,
            "Creating dynamic provider for model"
        );

        // Create dynamic provider based on type
        let response = match provider_type {
            "openrouter" => {
                self.create_dynamic_openai_compatible(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                    "OpenRouter",
                )
                .await?
            }
            "anthropic" => {
                self.create_dynamic_anthropic(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                )
                .await?
            }
            "deepseek" => {
                self.create_dynamic_openai_compatible(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                    "DeepSeek",
                )
                .await?
            }
            "azure_ai" => {
                self.create_dynamic_azure_ai(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                )
                .await?
            }
            "openai" => {
                self.create_dynamic_openai_compatible(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                    "OpenAI",
                )
                .await?
            }
            "openai-compatible" => {
                self.create_dynamic_openai_compatible(
                    actual_model,
                    &api_key,
                    &api_base,
                    chat_request,
                    context,
                    "OpenAI-Compatible",
                )
                .await?
            }
            _ => return Ok(None),
        };

        Ok(Some(response))
    }

    /// Create dynamic Anthropic provider
    async fn create_dynamic_anthropic(
        &self,
        model: &str,
        api_key: &str,
        api_base: &str,
        chat_request: &ChatRequest,
        context: RequestContext,
    ) -> Result<CompletionResponse> {
        use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

        let config = AnthropicConfig::new(api_key)
            .with_base_url(api_base)
            .with_experimental(false);

        let provider = AnthropicProvider::new(config)?;

        let mut updated_request = chat_request.clone();
        updated_request.model = model.to_string();

        let response = LLMProvider::chat_completion(&provider, updated_request, context)
            .await
            .map_err(|e| {
                GatewayError::internal(format!("Dynamic Anthropic provider error: {}", e))
            })?;

        convert_from_chat_completion_response(response)
    }

    /// Create dynamic OpenAI-compatible provider
    async fn create_dynamic_openai_compatible(
        &self,
        model: &str,
        api_key: &str,
        api_base: &str,
        chat_request: &ChatRequest,
        context: RequestContext,
        provider_name: &str,
    ) -> Result<CompletionResponse> {
        use crate::core::providers::base::BaseConfig;
        use crate::core::providers::openai::OpenAIProvider;
        use crate::core::providers::openai::config::OpenAIConfig;
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

        let config = OpenAIConfig {
            base: BaseConfig {
                api_key: Some(api_key.to_string()),
                api_base: Some(api_base.to_string()),
                timeout: 60,
                max_retries: 3,
                headers: Default::default(),
                organization: None,
                api_version: None,
            },
            organization: None,
            project: None,
            model_mappings: Default::default(),
            features: Default::default(),
        };

        let provider = OpenAIProvider::new(config).await.map_err(|e| {
            GatewayError::internal(format!(
                "Failed to create dynamic {} provider: {}",
                provider_name, e
            ))
        })?;

        let mut updated_request = chat_request.clone();
        updated_request.model = model.to_string();

        let response = provider
            .chat_completion(updated_request, context)
            .await
            .map_err(|e| {
                GatewayError::internal(format!("Dynamic {} provider error: {}", provider_name, e))
            })?;

        convert_from_chat_completion_response(response)
    }

    /// Create dynamic Azure AI provider
    #[cfg(feature = "providers-extra")]
    async fn create_dynamic_azure_ai(
        &self,
        model: &str,
        api_key: &str,
        api_base: &str,
        chat_request: &ChatRequest,
        context: RequestContext,
    ) -> Result<CompletionResponse> {
        use crate::core::providers::azure_ai::{AzureAIConfig, AzureAIProvider};
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some(api_key.to_string());
        config.base.api_base = Some(api_base.to_string());

        // Also check environment variables
        if config.base.api_key.is_none()
            && let Ok(key) = std::env::var("AZURE_AI_API_KEY")
        {
            config.base.api_key = Some(key);
        }
        if config.base.api_base.is_none()
            && let Ok(base) = std::env::var("AZURE_AI_API_BASE")
        {
            config.base.api_base = Some(base);
        }

        let provider = AzureAIProvider::new(config).map_err(|e| {
            GatewayError::internal(format!("Failed to create dynamic Azure AI provider: {}", e))
        })?;

        let mut updated_request = chat_request.clone();
        updated_request.model = model.to_string();

        let response = provider
            .chat_completion(updated_request, context)
            .await
            .map_err(|e| {
                GatewayError::internal(format!("Dynamic Azure AI provider error: {}", e))
            })?;

        convert_from_chat_completion_response(response)
    }

    /// Create dynamic Azure AI provider (stub when providers-extra is disabled)
    #[cfg(not(feature = "providers-extra"))]
    async fn create_dynamic_azure_ai(
        &self,
        model: &str,
        api_key: &str,
        api_base: &str,
        chat_request: &ChatRequest,
        context: RequestContext,
    ) -> Result<CompletionResponse> {
        let _ = (model, api_key, api_base, chat_request, context);
        Err(GatewayError::not_implemented(
            "dynamic azure_ai requires the `providers-extra` feature",
        ))
    }
}
