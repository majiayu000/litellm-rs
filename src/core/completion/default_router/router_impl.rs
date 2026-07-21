// Router trait implementation for DefaultRouter.

use super::*;
use crate::core::providers::registry::{
    ProviderRouteSurface, canonical_selector, provider_surface_matrix,
};
use crate::core::router::RuntimeHandle;
use crate::core::router::execution::router_error_to_provider_error;

pub(super) async fn complete_with_runtime_handle(
    handle: &RuntimeHandle,
    model: &str,
    messages: Vec<Message>,
    options: CompletionOptions,
) -> Result<CompletionResponse> {
    let chat_messages = convert_messages_to_chat_messages(messages);
    let chat_request = convert_to_chat_completion_request(model, chat_messages, options.clone())?;

    if let Some(selector) = DefaultRouter::unsupported_explicit_completion_selector(
        model,
        ProviderRouteSurface::CompletionChat,
        false,
    ) {
        return Err(GatewayError::invalid_request(format!(
            "completion() chat is not supported for provider route '{}'",
            selector
        )));
    }

    if options.api_key.is_some()
        || options.api_base.is_some()
        || options.api_version.is_some()
        || options.organization.is_some()
        || options.headers.is_some()
        || options.timeout.is_some()
    {
        return Err(GatewayError::from(
            crate::core::providers::ProviderError::invalid_request(
                "completion",
                "request-level provider overrides require a matching canonical runtime deployment",
            ),
        ));
    }

    let context = RequestContext::new();
    let execution = handle
        .execute_with_selected_deployment(model, move |deployment| {
            let mut request = chat_request.clone();
            let context = context.clone();
            async move {
                request.model = deployment.model.clone();
                let response = deployment
                    .provider
                    .chat_completion(request, context)
                    .await?;
                let tokens = response
                    .usage
                    .as_ref()
                    .map(|usage| u64::from(usage.total_tokens))
                    .unwrap_or_default();
                Ok((response, tokens))
            }
        })
        .await
        .map_err(|error| GatewayError::from(router_error_to_provider_error(error)))?;

    convert_from_chat_completion_response(execution.result)
}

fn bedrock_execution_model_id(model: &str) -> String {
    crate::core::providers::bedrock::parse_bedrock_model_id(model).execution_model_id
}

impl DefaultRouter {
    fn unsupported_explicit_completion_selector(
        model: &str,
        surface: ProviderRouteSurface,
        has_custom_api_base: bool,
    ) -> Option<&'static str> {
        if has_custom_api_base {
            return None;
        }

        let (selector, _) = model.split_once('/')?;
        let canonical = canonical_selector(selector);
        provider_surface_matrix()
            .iter()
            .find(|entry| entry.selector == canonical)
            .filter(|entry| !entry.support_for(surface).is_available_in_current_build())
            .map(|entry| entry.selector)
    }

    fn select_static_provider<'a>(
        providers: &'a [&'a Provider],
        model: &str,
        chat_request: &ChatRequest,
    ) -> Option<(&'a Provider, ChatRequest)> {
        Self::select_provider_by_name(providers, "openrouter", model, "openrouter/", chat_request)
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "deepseek",
                    model,
                    "deepseek/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "anthropic",
                    model,
                    "anthropic/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "bedrock", model, "bedrock/", chat_request)
                    .map(|(provider, mut request)| {
                        request.model = bedrock_execution_model_id(&request.model);
                        (provider, request)
                    })
            })
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "azure_ai",
                    model,
                    "azure_ai/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "groq", model, "groq/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "xiaomi_mimo",
                    model,
                    "xiaomi_mimo/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "xiaomi_mimo",
                    model,
                    "mimo/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "xai", model, "xai/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_name(
                    providers,
                    "moonshot",
                    model,
                    "moonshot/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "minimax", model, "minimax/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "zhipu", model, "zhipu/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "zhipu", model, "glm/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_name(providers, "zai", model, "zai/", chat_request)
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["together_ai", "together"],
                    model,
                    "together_ai/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["together", "together_ai"],
                    model,
                    "together/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["fireworks_ai", "fireworks"],
                    model,
                    "fireworks_ai/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["fireworks", "fireworks_ai"],
                    model,
                    "fireworks/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["aiml", "aiml_api"],
                    model,
                    "aiml/",
                    chat_request,
                )
            })
            .or_else(|| {
                Self::select_provider_by_any_name(
                    providers,
                    &["aiml_api", "aiml"],
                    model,
                    "aiml_api/",
                    chat_request,
                )
            })
    }
}

#[async_trait]
impl Router for DefaultRouter {
    async fn complete(
        &self,
        model: &str,
        messages: Vec<Message>,
        options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        let handle = match &self.runtime_binding {
            Some(binding) => binding.bind(),
            None => default_runtime().map_err(GatewayError::from)?,
        };
        complete_with_runtime_handle(&handle, model, messages, options).await
    }

    async fn complete_stream(
        &self,
        model: &str,
        messages: Vec<Message>,
        options: CompletionOptions,
    ) -> Result<CompletionStream> {
        // Convert to internal types
        let chat_messages = convert_messages_to_chat_messages(messages);
        let mut chat_request =
            convert_to_chat_completion_request(model, chat_messages, options.clone())?;
        chat_request.stream = true;

        if let Some(selector) = Self::unsupported_explicit_completion_selector(
            model,
            ProviderRouteSurface::CompletionChatStream,
            options.api_base.is_some(),
        ) {
            return Err(GatewayError::invalid_request(format!(
                "completion() streaming is not supported for provider route '{}'",
                selector
            )));
        }

        // Create request context
        let context = RequestContext::new();

        if let Some(stream) = self
            .try_dynamic_provider_stream_creation(&chat_request, context.clone(), &options)
            .await?
        {
            return Ok(stream);
        }

        // Find provider
        let providers = self.provider_registry.all();

        // Check if model explicitly specifies a provider
        let selected_provider = Self::select_static_provider(&providers, model, &chat_request);

        // Get the provider and execute streaming
        if let Some((provider, request)) = selected_provider {
            let stream = provider
                .chat_completion_stream(request, context)
                .await
                .map_err(|e| GatewayError::internal(format!("Streaming error: {}", e)))?;

            // Convert ChatChunk stream to ChatCompletionChunk stream
            let converted_stream = stream.map(|result| {
                result
                    .map(convert_chat_chunk_to_completion_chunk)
                    .map_err(|e| GatewayError::internal(format!("Stream chunk error: {}", e)))
            });

            return Ok(Box::pin(converted_stream));
        }

        Err(GatewayError::internal(
            "No suitable provider found for streaming",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_completion_prefixes_are_explicit() {
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "google/gemini-pro",
                ProviderRouteSurface::CompletionChat,
                false
            ),
            Some("google")
        );
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "gemini/gemini-pro",
                ProviderRouteSurface::CompletionChatStream,
                false
            ),
            Some("gemini")
        );
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "google/gemini-pro",
                ProviderRouteSurface::CompletionChat,
                true
            ),
            None
        );
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "azure/gpt-4",
                ProviderRouteSurface::CompletionChat,
                false
            ),
            None
        );
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "openrouter/deepseek-chat",
                ProviderRouteSurface::CompletionChat,
                false
            ),
            None
        );
        assert_eq!(
            DefaultRouter::unsupported_explicit_completion_selector(
                "together/meta-llama",
                ProviderRouteSurface::CompletionChat,
                false
            ),
            None
        );
    }

    async fn tier1_provider(name: &str) -> Result<Provider> {
        let Some(def) = crate::core::providers::registry::get_definition(name) else {
            return Err(GatewayError::internal(format!(
                "Missing test provider definition: {name}"
            )));
        };
        let config = def.to_openai_like_config(Some("test-key"), None);
        let provider = crate::core::providers::openai_like::OpenAILikeProvider::new(config)
            .await
            .map_err(|error| GatewayError::internal(error.to_string()))?;
        Ok(Provider::OpenAILike(provider))
    }

    #[tokio::test]
    async fn select_static_provider_routes_xai_prefix_to_xai() -> Result<()> {
        let groq = tier1_provider("groq").await?;
        let xai = tier1_provider("xai").await?;
        let providers = vec![&groq, &xai];
        let chat_request = ChatRequest {
            model: "xai/grok-4.3".to_string(),
            ..Default::default()
        };

        let Some((provider, routed_request)) =
            DefaultRouter::select_static_provider(&providers, "xai/grok-4.3", &chat_request)
        else {
            return Err(GatewayError::internal(
                "xai-prefixed models must select xai provider",
            ));
        };

        assert_eq!(provider.name(), "xai");
        assert_eq!(routed_request.model, "grok-4.3");
        assert_eq!(chat_request.model, "xai/grok-4.3");
        Ok(())
    }

    #[tokio::test]
    async fn select_static_provider_routes_xiaomi_mimo_prefix_to_mimo() -> Result<()> {
        let xiaomi_mimo = tier1_provider("xiaomi_mimo").await?;
        let providers = vec![&xiaomi_mimo];
        let chat_request = ChatRequest {
            model: "xiaomi_mimo/mimo-v2.5-pro".to_string(),
            ..Default::default()
        };

        let Some((provider, routed_request)) = DefaultRouter::select_static_provider(
            &providers,
            "xiaomi_mimo/mimo-v2.5-pro",
            &chat_request,
        ) else {
            return Err(GatewayError::internal(
                "xiaomi_mimo-prefixed models must select Xiaomi MiMo provider",
            ));
        };

        assert_eq!(provider.name(), "xiaomi_mimo");
        assert_eq!(routed_request.model, "mimo-v2.5-pro");
        assert_eq!(chat_request.model, "xiaomi_mimo/mimo-v2.5-pro");
        Ok(())
    }

    #[tokio::test]
    async fn issue_760_static_provider_routes_litellm_alias_prefixes() -> Result<()> {
        let cases = [
            ("zai", "zai/glm-5", "zai", "glm-5"),
            (
                "together_ai",
                "together_ai/meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "together_ai",
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            ),
            (
                "together",
                "together_ai/meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "together",
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            ),
            (
                "together_ai",
                "together/meta-llama/Llama-3.3-70B-Instruct-Turbo",
                "together_ai",
                "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            ),
            (
                "fireworks_ai",
                "fireworks_ai/accounts/fireworks/models/deepseek-v3",
                "fireworks_ai",
                "accounts/fireworks/models/deepseek-v3",
            ),
            (
                "fireworks",
                "fireworks_ai/accounts/fireworks/models/deepseek-v3",
                "fireworks",
                "accounts/fireworks/models/deepseek-v3",
            ),
            ("aiml", "aiml/gpt-4o-mini", "aiml", "gpt-4o-mini"),
            ("aiml_api", "aiml/gpt-4o-mini", "aiml_api", "gpt-4o-mini"),
        ];

        for (configured_provider, model, provider_name, routed_model) in cases {
            let owned_provider = tier1_provider(configured_provider).await?;
            let providers = vec![&owned_provider];
            let chat_request = ChatRequest {
                model: model.to_string(),
                ..Default::default()
            };
            let Some((provider, routed_request)) =
                DefaultRouter::select_static_provider(&providers, model, &chat_request)
            else {
                return Err(GatewayError::internal(format!(
                    "{provider_name}-prefixed model should select {provider_name} provider"
                )));
            };

            assert_eq!(provider.name(), provider_name);
            assert_eq!(routed_request.model, routed_model);
            assert_eq!(chat_request.model, model);
        }

        Ok(())
    }
}
