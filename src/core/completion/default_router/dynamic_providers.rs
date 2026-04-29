// Dynamic provider creation methods for DefaultRouter.

use super::*;

struct DynamicProviderRoute<'a> {
    provider_type: &'static str,
    provider_label: &'static str,
    actual_model: &'a str,
    api_base: String,
}

struct DynamicProviderPrefix {
    prefix: &'static str,
    provider_type: &'static str,
    provider_label: &'static str,
    default_api_base: &'static str,
}

const DYNAMIC_PROVIDER_PREFIXES: &[DynamicProviderPrefix] = &[
    DynamicProviderPrefix {
        prefix: "openrouter/",
        provider_type: "openrouter",
        provider_label: "OpenRouter",
        default_api_base: "https://openrouter.ai/api/v1",
    },
    DynamicProviderPrefix {
        prefix: "anthropic/",
        provider_type: "anthropic",
        provider_label: "Anthropic",
        default_api_base: "https://api.anthropic.com",
    },
    DynamicProviderPrefix {
        prefix: "deepseek/",
        provider_type: "deepseek",
        provider_label: "DeepSeek",
        default_api_base: "https://api.deepseek.com",
    },
    DynamicProviderPrefix {
        prefix: "moonshot/",
        provider_type: "moonshot",
        provider_label: "Moonshot",
        default_api_base: "https://api.moonshot.cn/v1",
    },
    DynamicProviderPrefix {
        prefix: "minimax/",
        provider_type: "minimax",
        provider_label: "MiniMax",
        default_api_base: "https://api.minimax.chat/v1",
    },
    DynamicProviderPrefix {
        prefix: "zhipu/",
        provider_type: "zhipu",
        provider_label: "Zhipu",
        default_api_base: "https://open.bigmodel.cn/api/paas/v4",
    },
    DynamicProviderPrefix {
        prefix: "glm/",
        provider_type: "zhipu",
        provider_label: "Zhipu",
        default_api_base: "https://open.bigmodel.cn/api/paas/v4",
    },
    DynamicProviderPrefix {
        prefix: "zai/",
        provider_type: "zhipu",
        provider_label: "Zhipu",
        default_api_base: "https://open.bigmodel.cn/api/paas/v4",
    },
    DynamicProviderPrefix {
        prefix: "openai/",
        provider_type: "openai",
        provider_label: "OpenAI",
        default_api_base: "https://api.openai.com/v1",
    },
];

fn resolve_dynamic_provider_route<'a>(
    model: &'a str,
    options: &CompletionOptions,
) -> Option<DynamicProviderRoute<'a>> {
    for config in DYNAMIC_PROVIDER_PREFIXES {
        if let Some(actual_model) = model.strip_prefix(config.prefix) {
            let api_base = options
                .api_base
                .clone()
                .unwrap_or_else(|| config.default_api_base.to_string());
            return Some(DynamicProviderRoute {
                provider_type: config.provider_type,
                provider_label: config.provider_label,
                actual_model,
                api_base,
            });
        }
    }

    if model.starts_with("azure_ai/") || model.starts_with("azure-ai/") {
        let actual_model = model
            .strip_prefix("azure_ai/")
            .or_else(|| model.strip_prefix("azure-ai/"))
            .unwrap_or(model);
        let api_base = options
            .api_base
            .clone()
            .or_else(|| std::env::var("AZURE_AI_API_BASE").ok())
            .unwrap_or_else(|| "https://api.azure.com".to_string());
        return Some(DynamicProviderRoute {
            provider_type: "azure_ai",
            provider_label: "Azure AI",
            actual_model,
            api_base,
        });
    }

    options
        .api_base
        .clone()
        .map(|api_base| DynamicProviderRoute {
            provider_type: "openai-compatible",
            provider_label: "OpenAI-Compatible",
            actual_model: model,
            api_base,
        })
}

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

        let Some(route) = resolve_dynamic_provider_route(model, options) else {
            return Ok(None);
        };

        debug!(
            provider_type = %route.provider_type,
            model = %route.actual_model,
            "Creating dynamic provider for model"
        );

        // Create dynamic provider based on type
        let response = match route.provider_type {
            "anthropic" => {
                self.create_dynamic_anthropic(
                    route.actual_model,
                    &api_key,
                    &route.api_base,
                    chat_request,
                    context,
                )
                .await?
            }
            "azure_ai" => {
                self.create_dynamic_azure_ai(
                    route.actual_model,
                    &api_key,
                    &route.api_base,
                    chat_request,
                    context,
                )
                .await?
            }
            _ => {
                self.create_dynamic_openai_compatible(
                    route.actual_model,
                    &api_key,
                    &route.api_base,
                    chat_request,
                    context,
                    route.provider_label,
                )
                .await?
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_dynamic_route_for_moonshot() {
        let options = CompletionOptions::default();
        let route = resolve_dynamic_provider_route("moonshot/kimi-k2.5", &options).unwrap();

        assert_eq!(route.provider_type, "moonshot");
        assert_eq!(route.provider_label, "Moonshot");
        assert_eq!(route.actual_model, "kimi-k2.5");
        assert_eq!(route.api_base, "https://api.moonshot.cn/v1");
    }

    #[test]
    fn test_resolve_dynamic_route_for_minimax() {
        let options = CompletionOptions::default();
        let route =
            resolve_dynamic_provider_route("minimax/MiniMax-M2.5-lightning", &options).unwrap();

        assert_eq!(route.provider_type, "minimax");
        assert_eq!(route.provider_label, "MiniMax");
        assert_eq!(route.actual_model, "MiniMax-M2.5-lightning");
        assert_eq!(route.api_base, "https://api.minimax.chat/v1");
    }

    #[test]
    fn test_resolve_dynamic_route_for_glm_alias() {
        let options = CompletionOptions::default();
        let route = resolve_dynamic_provider_route("glm/glm-5", &options).unwrap();

        assert_eq!(route.provider_type, "zhipu");
        assert_eq!(route.provider_label, "Zhipu");
        assert_eq!(route.actual_model, "glm-5");
        assert_eq!(route.api_base, "https://open.bigmodel.cn/api/paas/v4");
    }

    #[test]
    fn test_resolve_dynamic_route_for_zai_alias() {
        let options = CompletionOptions::default();
        let route = resolve_dynamic_provider_route("zai/glm-5", &options).unwrap();

        assert_eq!(route.provider_type, "zhipu");
        assert_eq!(route.provider_label, "Zhipu");
        assert_eq!(route.actual_model, "glm-5");
        assert_eq!(route.api_base, "https://open.bigmodel.cn/api/paas/v4");
    }

    #[test]
    fn test_resolve_dynamic_route_with_custom_api_base() {
        let options = CompletionOptions {
            api_base: Some("http://localhost:5567/v1".to_string()),
            ..CompletionOptions::default()
        };

        let route = resolve_dynamic_provider_route("my-custom-model", &options).unwrap();
        assert_eq!(route.provider_type, "openai-compatible");
        assert_eq!(route.provider_label, "OpenAI-Compatible");
        assert_eq!(route.actual_model, "my-custom-model");
        assert_eq!(route.api_base, "http://localhost:5567/v1");
    }

    #[test]
    fn test_resolve_dynamic_route_without_prefix_or_api_base() {
        let options = CompletionOptions::default();
        let route = resolve_dynamic_provider_route("my-custom-model", &options);
        assert!(route.is_none());
    }
}
