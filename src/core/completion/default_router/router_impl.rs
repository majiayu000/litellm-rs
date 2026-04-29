// Router trait implementation for DefaultRouter.

use super::*;

#[cfg(feature = "providers-extra")]
fn normalize_bedrock_model_id(model: &str) -> String {
    crate::core::providers::bedrock::normalize_bedrock_model_id(model)
}

#[cfg(not(feature = "providers-extra"))]
fn normalize_bedrock_model_id(model: &str) -> String {
    model.to_string()
}

#[async_trait]
impl Router for DefaultRouter {
    async fn complete(
        &self,
        model: &str,
        messages: Vec<Message>,
        options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        // Convert to internal types
        let chat_messages = convert_messages_to_chat_messages(messages);
        let chat_request =
            convert_to_chat_completion_request(model, chat_messages, options.clone())?;

        // Create request context with override parameters from options
        let mut context = RequestContext::new();

        // Check for dynamic provider configuration overrides
        if let Some(api_base) = &options.api_base {
            context.metadata.insert(
                "api_base_override".to_string(),
                serde_json::Value::String(api_base.clone()),
            );
        }

        if let Some(api_key) = &options.api_key {
            context.metadata.insert(
                "api_key_override".to_string(),
                serde_json::Value::String(api_key.clone()),
            );
        }

        if let Some(organization) = &options.organization {
            context.metadata.insert(
                "organization_override".to_string(),
                serde_json::Value::String(organization.clone()),
            );
        }

        if let Some(api_version) = &options.api_version {
            context.metadata.insert(
                "api_version_override".to_string(),
                serde_json::Value::String(api_version.clone()),
            );
        }

        if let Some(headers) = &options.headers {
            context.metadata.insert(
                "headers_override".to_string(),
                serde_json::to_value(headers).unwrap_or_default(),
            );
        }

        if let Some(timeout) = options.timeout {
            context.metadata.insert(
                "timeout_override".to_string(),
                serde_json::Value::Number(serde_json::Number::from(timeout)),
            );
        }

        // Check if user provided custom api_base (Python LiteLLM compatibility)
        if let Some(api_base) = &options.api_base {
            use crate::core::providers::base::BaseConfig;
            use crate::core::providers::openai::OpenAIProvider;
            use crate::core::providers::openai::config::OpenAIConfig;
            use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

            let api_key = options
                .api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .unwrap_or_else(|| "dummy-key-for-local".to_string());

            let config = OpenAIConfig {
                base: BaseConfig {
                    api_key: Some(api_key),
                    api_base: Some(api_base.clone()),
                    timeout: options.timeout.unwrap_or(60),
                    max_retries: 3,
                    headers: options.headers.clone().unwrap_or_default(),
                    organization: options.organization.clone(),
                    api_version: None,
                },
                organization: options.organization.clone(),
                project: None,
                model_mappings: Default::default(),
                features: Default::default(),
            };

            match OpenAIProvider::new(config).await {
                Ok(temp_provider) => {
                    let response = temp_provider
                        .chat_completion(chat_request, context)
                        .await
                        .map_err(|e| GatewayError::internal(format!("Provider error: {}", e)))?;
                    return convert_from_chat_completion_response(response);
                }
                Err(e) => {
                    return Err(GatewayError::internal(format!(
                        "Failed to create provider with custom api_base: {}",
                        e
                    )));
                }
            }
        }

        // Dynamic provider creation (Python LiteLLM style)
        if let Some(response) = self
            .try_dynamic_provider_creation(&chat_request, context.clone(), &options)
            .await?
        {
            return Ok(response);
        }

        // Fallback to static provider registry
        let providers = self.provider_registry.all();

        // Check if model explicitly specifies a provider
        let mut selected_provider = Self::select_provider_by_name(
            &providers,
            "openrouter",
            model,
            "openrouter/",
            &chat_request,
        )
        .or_else(|| {
            Self::select_provider_by_name(&providers, "deepseek", model, "deepseek/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(
                &providers,
                "anthropic",
                model,
                "anthropic/",
                &chat_request,
            )
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "bedrock", model, "bedrock/", &chat_request)
                .map(|(provider, mut request)| {
                    request.model = normalize_bedrock_model_id(&request.model);
                    (provider, request)
                })
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "azure_ai", model, "azure_ai/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "groq", model, "groq/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "moonshot", model, "moonshot/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "minimax", model, "minimax/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "zhipu/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "glm/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "zai/", &chat_request)
        });

        // Handle special cases
        if selected_provider.is_none() {
            if model.starts_with("openai/") || model.starts_with("azure/") {
                for provider in providers.iter() {
                    if provider.provider_type() == ProviderType::OpenAI
                        && provider.supports_model(model)
                    {
                        selected_provider = Some((provider, chat_request.clone()));
                        break;
                    }
                }
            } else {
                // No explicit provider, try to find one that supports the model
                for provider in providers.iter() {
                    if provider.supports_model(model) {
                        selected_provider = Some((provider, chat_request.clone()));
                        break;
                    }
                }
            }
        }

        // Use static provider if found
        if let Some((provider, request)) = selected_provider {
            let response = provider.chat_completion(request, context).await?;
            return convert_from_chat_completion_response(response);
        }

        Err(GatewayError::internal(
            "No suitable provider found for model",
        ))
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

        // Create request context
        let context = RequestContext::new();

        // Find provider
        let providers = self.provider_registry.all();

        // Check if model explicitly specifies a provider
        let selected_provider = Self::select_provider_by_name(
            &providers,
            "openrouter",
            model,
            "openrouter/",
            &chat_request,
        )
        .or_else(|| {
            Self::select_provider_by_name(&providers, "deepseek", model, "deepseek/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(
                &providers,
                "anthropic",
                model,
                "anthropic/",
                &chat_request,
            )
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "bedrock", model, "bedrock/", &chat_request)
                .map(|(provider, mut request)| {
                    request.model = normalize_bedrock_model_id(&request.model);
                    (provider, request)
                })
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "azure_ai", model, "azure_ai/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "groq", model, "groq/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "moonshot", model, "moonshot/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "minimax", model, "minimax/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "zhipu/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "glm/", &chat_request)
        })
        .or_else(|| {
            Self::select_provider_by_name(&providers, "zhipu", model, "zai/", &chat_request)
        });

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
