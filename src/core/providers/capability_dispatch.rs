use crate::core::providers::Provider;
use crate::core::providers::openai::models::OpenAICatalogRuntimeResolution;
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
        match self {
            Provider::OpenAI(provider) => match provider.resolve_model_runtime(model) {
                OpenAICatalogRuntimeResolution::Callable(model_spec) => {
                    model_spec.model_info.capabilities.contains(capability)
                }
                OpenAICatalogRuntimeResolution::PricingOnly(_) => false,
                OpenAICatalogRuntimeResolution::UnregisteredDeployment { .. } => {
                    LLMProvider::supports_capability(provider, capability)
                }
                OpenAICatalogRuntimeResolution::Invalid(_) => false,
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
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
    use crate::core::providers::{GeminiNativeRequest, Provider, ProviderError};
    use crate::core::types::model::ProviderCapability;

    async fn openai_provider() -> Provider {
        Provider::OpenAI(
            OpenAIProvider::with_api_key("sk-test-key")
                .await
                .expect("test OpenAI provider should build"),
        )
    }

    #[tokio::test]
    async fn openai_pricing_only_catalog_entries_do_not_inherit_provider_capabilities() {
        let provider = openai_provider().await;

        for model in ["1024-x-1024/dall-e-2", "openai/1024-x-1024/dall-e-2"] {
            for capability in [
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::Embeddings,
            ] {
                assert!(
                    !provider.supports_capability_for_model(model, &capability),
                    "pricing-only catalog key {model} must not inherit {capability:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn openai_callable_and_unknown_models_keep_existing_route_contracts() {
        let provider = openai_provider().await;

        assert!(
            provider
                .supports_capability_for_model("openai/gpt-4", &ProviderCapability::ChatCompletion)
        );
        assert!(
            !provider
                .supports_capability_for_model("openai/gpt-4", &ProviderCapability::Embeddings)
        );
        assert!(provider.supports_capability_for_model(
            "text-embedding-3-small",
            &ProviderCapability::Embeddings
        ));
        assert!(!provider.supports_capability_for_model(
            "text-embedding-3-small",
            &ProviderCapability::ChatCompletion
        ));

        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
        ] {
            assert!(
                provider.supports_capability_for_model("custom-openai-deployment", &capability),
                "unresolved OpenAI deployments retain provider-level pass-through"
            );
        }
    }

    #[tokio::test]
    async fn invalid_openai_identities_fail_closed_for_all_production_routes() {
        let provider = openai_provider().await;

        for model in [
            "anthropic/gpt-4",
            "openai/openai/gpt-4",
            "openai/fake-gpt-5",
            "unknown/a/b",
        ] {
            for capability in [
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::Embeddings,
            ] {
                assert!(
                    !provider.supports_capability_for_model(model, &capability),
                    "invalid model identity {model} must not inherit {capability:?}"
                );
            }
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
