use crate::core::providers::Provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

impl Provider {
    /// Check whether a deployment's concrete model supports `capability`.
    ///
    /// Provider-level capabilities answer whether a provider family has an
    /// implementation. When the provider has a registry entry for the concrete
    /// deployment model, route selection also respects that model-specific
    /// capability list.
    pub fn supports_capability_for_model(
        &self,
        model: &str,
        capability: &ProviderCapability,
    ) -> bool {
        if let Some(identity) = self.deployment_model_identity() {
            let Some(catalog_provider) = identity.capability_catalog_provider() else {
                return false;
            };
            let Some(catalog_model) = identity.capability_catalog_model() else {
                return false;
            };
            return match self {
                Provider::OpenAI(provider) => {
                    catalog_provider == "openai"
                        && provider.model_supports_capability(catalog_model, capability)
                }
                #[cfg(feature = "providers-extra")]
                Provider::Azure(provider) => {
                    catalog_provider == "openai"
                        && LLMProvider::supports_capability(provider, capability)
                        && crate::core::providers::openai::models::get_openai_registry()
                            .get_model_spec(catalog_model)
                            .is_some_and(|model| model.model_info.capabilities.contains(capability))
                }
                #[cfg(feature = "providers-extra")]
                Provider::AzureAI(provider) => {
                    LLMProvider::supports_capability(provider, capability)
                        && match catalog_provider {
                            "openai" => {
                                crate::core::providers::openai::models::get_openai_registry()
                                    .get_model_spec(catalog_model)
                                    .is_some_and(|model| {
                                        model.model_info.capabilities.contains(capability)
                                    })
                            }
                            "azure_ai" => {
                                provider
                                    .get_model_registry()
                                    .supports_capability(catalog_model, capability)
                                    || provider.model_identity.as_ref().is_some_and(|binding| {
                                        binding
                                            .pricing()
                                            .get_model_info_for_provider("azure_ai", catalog_model)
                                            .is_some_and(|(_, metadata)| match capability {
                                                ProviderCapability::ChatCompletion => {
                                                    metadata.mode == "chat"
                                                }
                                                ProviderCapability::ChatCompletionStream => {
                                                    metadata.mode == "chat"
                                                        && metadata.supports_streaming
                                                            != Some(false)
                                                }
                                                ProviderCapability::Embeddings => {
                                                    metadata.mode == "embedding"
                                                }
                                                ProviderCapability::ImageGeneration => {
                                                    metadata.mode == "image_generation"
                                                }
                                                _ => false,
                                            })
                                    })
                            }
                            _ => false,
                        }
                }
                Provider::OpenAILike(provider) => {
                    catalog_provider == "xai"
                        && LLMProvider::supports_capability(provider, capability)
                        && provider
                            .get_model_info(catalog_model)
                            .capabilities
                            .contains(capability)
                }
                _ => false,
            };
        }
        match self {
            Provider::OpenAI(provider) => {
                if provider.get_model_config(model).is_some() {
                    provider.model_supports_capability(model, capability)
                } else {
                    LLMProvider::supports_capability(provider, capability)
                }
            }
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(provider) => {
                LLMProvider::supports_capability(provider, capability)
                    && provider
                        .get_model_registry()
                        .supports_capability(model, capability)
            }
            Provider::OpenAILike(provider) if provider.name() == "xai" => {
                LLMProvider::supports_model(provider, model)
                    && LLMProvider::supports_capability(provider, capability)
            }
            Provider::OpenAILike(provider) if capability == &ProviderCapability::Rerank => {
                openai_like_provider_supports_rerank(provider.name())
            }
            Provider::OpenAILike(provider)
                if capability == &ProviderCapability::GeminiGenerateContent =>
            {
                openai_like_provider_supports_gemini(provider.name())
            }
            _ => self.supports_capability(capability),
        }
    }
}

pub(crate) fn openai_like_provider_supports_gemini(provider_name: &str) -> bool {
    if !provider_name
        .chars()
        .all(|ch| matches!(ch, '_' | '-') || ch.is_ascii_alphanumeric())
    {
        return false;
    }
    matches!(
        normalize_provider_name(provider_name).as_str(),
        "gemini" | "googleai" | "googleaistudio"
    )
}

fn openai_like_provider_supports_rerank(provider_name: &str) -> bool {
    let normalized = normalize_provider_name(provider_name);
    normalized.contains("cohere") || normalized.contains("jina")
}

fn normalize_provider_name(provider_name: &str) -> String {
    provider_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{openai_like_provider_supports_gemini, openai_like_provider_supports_rerank};
    use crate::core::net::ProviderEndpointAccess;
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
    use crate::core::providers::{GeminiNativeRequest, ProviderError};

    #[cfg(feature = "providers-extra")]
    #[test]
    fn unbound_azure_ai_dispatch_uses_exact_model_registry() {
        use crate::core::providers::{
            Provider, azure_ai::AzureAIConfig, azure_ai::AzureAIProvider,
        };
        use crate::core::types::model::ProviderCapability;

        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());
        config.base.api_base = Some("https://example.ai.azure.com".to_string());
        let provider = Provider::AzureAI(
            AzureAIProvider::new(config).expect("Azure AI provider should build"),
        );

        assert!(
            provider.supports_capability_for_model("Phi-4", &ProviderCapability::ChatCompletion,)
        );
        assert!(provider.supports_capability_for_model(
            "text-embedding-3-large",
            &ProviderCapability::Embeddings,
        ));
        assert!(
            provider
                .supports_capability_for_model("dall-e-3", &ProviderCapability::ImageGeneration,)
        );
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
        ] {
            assert!(
                !provider.supports_capability_for_model("customer-unknown", &capability),
                "unknown model inherited {capability:?}"
            );
        }
    }

    #[cfg(feature = "providers-extra")]
    #[test]
    fn mapped_phi_4_stays_azure_ai_and_does_not_advertise_tool_calling() {
        use crate::core::providers::model_identity::{
            DeploymentProviderBinding, ModelIdentityMapping, validate_deployment_identity,
        };
        use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;
        use crate::core::providers::{
            Provider, azure_ai::AzureAIConfig, azure_ai::AzureAIProvider,
        };
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        use crate::core::types::model::ProviderCapability;

        let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
        let catalog = CatalogAuthority::from_embedded().expect("embedded catalog should load");
        let mapping = ModelIdentityMapping::new(Some("azure_ai/Phi-4".to_string()), None);
        let identity = validate_deployment_identity(
            "native-phi",
            "azure_ai",
            "customer-phi-deployment",
            Some(&mapping),
            None,
            &catalog,
            &pricing.snapshot(),
        )
        .expect("exact Azure AI Phi-4 mapping should validate");
        assert_eq!(identity.wire_model(), "customer-phi-deployment");
        assert_eq!(identity.capability_catalog_provider(), Some("azure_ai"));

        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());
        config.base.api_base = Some("https://example.ai.azure.com".to_string());
        let mut azure_ai = AzureAIProvider::new(config).expect("Azure AI provider should build");
        azure_ai.model_identity = Some(DeploymentProviderBinding::new(identity, pricing));
        let params = azure_ai.get_supported_openai_params("customer-phi-deployment");
        assert!(params.contains(&"temperature"));
        assert!(!params.contains(&"tools"));
        assert!(!params.contains(&"tool_choice"));
        let provider = Provider::AzureAI(azure_ai);

        assert!(provider.supports_capability_for_model(
            "customer-phi-deployment",
            &ProviderCapability::ChatCompletion,
        ));
        assert!(!provider.supports_capability_for_model(
            "customer-phi-deployment",
            &ProviderCapability::Embeddings,
        ));
    }

    #[cfg(feature = "providers-extra")]
    #[test]
    fn mapped_azure_ai_callable_models_use_exact_bound_catalog_metadata() {
        use crate::core::pricing_service::PricingService;
        use crate::core::providers::model_identity::{
            DeploymentProviderBinding, ModelIdentityMapping, validate_deployment_identity,
        };
        use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;
        use crate::core::providers::{
            Provider, azure_ai::AzureAIConfig, azure_ai::AzureAIProvider,
        };
        use crate::core::types::model::ProviderCapability;
        use std::sync::Arc;

        let pricing = Arc::new(
            PricingService::with_embedded_default().expect("embedded pricing should load"),
        );
        let catalog = CatalogAuthority::from_embedded().expect("embedded catalog should load");

        let mapped_provider = |wire_model: &str, catalog_model: &str| {
            let mapping =
                ModelIdentityMapping::new(Some(format!("azure_ai/{catalog_model}")), None);
            let identity = validate_deployment_identity(
                "mapped-azure-ai",
                "azure_ai",
                wire_model,
                Some(&mapping),
                None,
                &catalog,
                &pricing.snapshot(),
            )
            .expect("exact Azure AI callable identity should validate");
            let mut config = AzureAIConfig::new("azure_ai");
            config.base.api_key = Some("test-key".to_string());
            config.base.api_base = Some("https://example.ai.azure.com".to_string());
            let mut provider =
                AzureAIProvider::new(config).expect("Azure AI provider should build");
            provider.model_identity = Some(DeploymentProviderBinding::new(
                identity,
                Arc::clone(&pricing),
            ));
            Provider::AzureAI(provider)
        };

        let llama = mapped_provider("llama-wire", "Llama-3.3-70B-Instruct");
        assert!(
            llama.supports_capability_for_model("llama-wire", &ProviderCapability::ChatCompletion,)
        );
        assert!(llama.supports_capability_for_model(
            "llama-wire",
            &ProviderCapability::ChatCompletionStream,
        ));
        assert!(
            !llama.supports_capability_for_model("llama-wire", &ProviderCapability::Embeddings,)
        );

        let cohere = mapped_provider("cohere-wire", "Cohere-embed-v3-multilingual");
        assert!(
            cohere.supports_capability_for_model("cohere-wire", &ProviderCapability::Embeddings,)
        );
        assert!(
            !cohere
                .supports_capability_for_model("cohere-wire", &ProviderCapability::ChatCompletion,)
        );

        let mai = mapped_provider("mai-wire", "MAI-Image-2.5");
        assert!(
            mai.supports_capability_for_model("mai-wire", &ProviderCapability::ImageGeneration,)
        );
        assert!(
            !mai.supports_capability_for_model("mai-wire", &ProviderCapability::ChatCompletion,)
        );

        let mut config = AzureAIConfig::new("azure_ai");
        config.base.api_key = Some("test-key".to_string());
        config.base.api_base = Some("https://example.ai.azure.com".to_string());
        let unknown = Provider::AzureAI(
            AzureAIProvider::new(config).expect("Azure AI provider should build"),
        );
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::Embeddings,
        ] {
            assert!(!unknown.supports_capability_for_model("unknown-wire", &capability));
        }
    }

    #[test]
    fn gemini_compatibility_name_set_is_closed_and_normalized() {
        for name in ["gemini", "Google-AI", "google_ai_studio"] {
            assert!(openai_like_provider_supports_gemini(name));
        }
        for name in ["openai", "my-gemini-proxy", "google", "g.e.m.i.n.i"] {
            assert!(!openai_like_provider_supports_gemini(name));
        }
        assert!(!openai_like_provider_supports_gemini("google ai"));
        assert!(openai_like_provider_supports_rerank("cohere.ai"));
        let leaked = "raw/key+value raw%2Fkey%2Bvalue";
        for (inner, is_timeout) in [
            (ProviderError::timeout("x", leaked), true),
            (ProviderError::network("x", leaked), false),
        ] {
            let error = OpenAILikeProvider::map_gemini_stream_response::<()>(Ok(Err(inner)))
                .expect_err("transport error must remain an error");
            assert_eq!(matches!(error, ProviderError::Timeout { .. }), is_timeout);
            let text = format!("{error:?} {error}");
            assert!(!text.contains("raw/key+value") && !text.contains("raw%2Fkey%2Bvalue"));
        }
    }
    #[tokio::test]
    async fn named_gemini_stream_uses_runtime_header_timeout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("timeout server should bind");
        let address = listener.local_addr().expect("listener should have address");
        let mut config = OpenAILikeConfig::with_api_key(format!("http://{address}"), "test-key");
        config.provider_name = "Google-AI".to_string();
        config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config.base.timeout = 1;
        let provider = OpenAILikeProvider::new_openai_compatible(config)
            .await
            .expect("named provider should build");
        let error = provider
            .gemini_generate_content(GeminiNativeRequest {
                api_version: "v1beta".to_string(),
                model: "gemini-3.1-flash-lite".to_string(),
                method: "streamGenerateContent",
                stream: true,
                body: serde_json::json!({"contents": []}),
            })
            .await
            .expect_err("delayed headers should time out");
        assert!(matches!(error, ProviderError::Timeout { .. }));
        drop(listener);
    }
}
