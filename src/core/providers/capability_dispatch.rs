use crate::core::providers::Provider;
use crate::core::providers::model_identity::ModelIdentity;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

impl Provider {
    /// Check whether a deployment's concrete model supports `capability`.
    ///
    /// Provider-level capabilities answer whether a provider family has an
    /// implementation. When the provider has a registry entry for the concrete
    /// deployment model, route selection also respects that model-specific
    /// capability list. This provider-only API intentionally has no deployment
    /// configuration context; router callers must use `Deployment` instead.
    pub fn supports_capability_for_model(
        &self,
        model: &str,
        capability: &ProviderCapability,
    ) -> bool {
        self.supports_capability_for_identity(model, capability, false)
    }

    pub(crate) fn supports_capability_for_deployment(
        &self,
        model: &str,
        capability: &ProviderCapability,
    ) -> bool {
        self.supports_capability_for_identity(model, capability, true)
    }

    fn supports_capability_for_identity(
        &self,
        model: &str,
        capability: &ProviderCapability,
        deployment_context: bool,
    ) -> bool {
        let identity = || {
            if deployment_context {
                self.resolve_model_identity(model)
            } else {
                self.resolve_exact_model_identity(model)
            }
        };
        match self {
            Provider::OpenAI(provider) => match identity() {
                ModelIdentity::CatalogCallable {
                    capability_catalog_model: catalog_model,
                    ..
                }
                | ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: Some(catalog_model),
                    ..
                } => provider.model_supports_capability(catalog_model, capability),
                ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: None,
                    ..
                }
                | ModelIdentity::PricingOnly { .. }
                | ModelIdentity::Invalid { .. } => false,
            },
            #[cfg(feature = "providers-extra")]
            Provider::Azure(_provider) => match identity() {
                ModelIdentity::CatalogCallable {
                    capability_catalog_model: catalog_model,
                    ..
                }
                | ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: Some(catalog_model),
                    ..
                } => crate::core::providers::openai::models::get_openai_registry()
                    .get_model_spec(catalog_model)
                    .is_some_and(|spec| spec.model_info.capabilities.contains(capability)),
                ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: None,
                    ..
                } => false,
                ModelIdentity::PricingOnly { .. } | ModelIdentity::Invalid { .. } => false,
            },
            #[cfg(feature = "providers-extra")]
            Provider::AzureAI(provider) => match identity() {
                ModelIdentity::CatalogCallable {
                    capability_catalog_model: catalog_model,
                    ..
                }
                | ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: Some(catalog_model),
                    ..
                } => provider
                    .get_model_registry()
                    .supports_capability(catalog_model, capability),
                ModelIdentity::ConfiguredDeployment {
                    capability_catalog_model: None,
                    ..
                } => false,
                ModelIdentity::PricingOnly { .. } | ModelIdentity::Invalid { .. } => false,
            },
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
    use crate::core::providers::Provider;
    use crate::core::providers::openai::{OpenAIConfig, OpenAIProvider};
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
    use crate::core::providers::{GeminiNativeRequest, ProviderError};
    use crate::core::router::Deployment;
    use crate::core::types::model::ProviderCapability;

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
    async fn openai_routing_uses_exact_config_backed_identity() {
        async fn provider() -> Provider {
            let mut config = OpenAIConfig::default();
            config.base.api_key = Some("sk-test-identity".to_string());
            Provider::OpenAI(OpenAIProvider::new(config).await.unwrap())
        }

        let provider = provider().await;
        let chat = Deployment::new(
            "chat".into(),
            provider.clone(),
            "shared-deployment".into(),
            "shared".into(),
        )
        .with_model_identity(Some("gpt-4".into()), None);
        let embedding = Deployment::new(
            "embedding".into(),
            provider.clone(),
            "shared-deployment".into(),
            "shared".into(),
        )
        .with_model_identity(Some("text-embedding-3-small".into()), None);

        assert!(chat.supports_capability(&ProviderCapability::ChatCompletion));
        assert!(!embedding.supports_capability(&ProviderCapability::ChatCompletion));
        assert!(embedding.supports_capability(&ProviderCapability::Embeddings));
        assert!(!provider.supports_capability_for_model(
            "shared-deployment",
            &ProviderCapability::ChatCompletion
        ));
        for model in [
            "shared-deployment",
            "1024-x-1024/dall-e-2",
            "openai/fake-gpt-5",
            "openai/openai/gpt-4",
            "anthropic/gpt-4",
            "unknown/a/b",
        ] {
            assert!(
                !provider.supports_capability_for_model(model, &ProviderCapability::ChatCompletion)
            );
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
