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
                            "azure_ai" => provider
                                .get_model_registry()
                                .supports_capability(catalog_model, capability),
                            _ => false,
                        }
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
