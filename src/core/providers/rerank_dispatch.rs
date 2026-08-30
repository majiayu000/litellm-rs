use super::enterprise::EnterpriseProvider;
use super::{Provider, ProviderError};
use crate::core::rerank::{CohereRerankProvider, JinaRerankProvider, RerankProvider};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use std::sync::Arc;

impl Provider {
    pub(crate) fn rerank_adapter(&self) -> Result<Arc<dyn RerankProvider>, ProviderError> {
        match self {
            Provider::Enterprise(EnterpriseProvider::Oci(provider)) => {
                Ok(Arc::new(provider.rerank_adapter()?))
            }
            Provider::Enterprise(EnterpriseProvider::Watsonx(provider)) => {
                Ok(Arc::new(provider.rerank_adapter()))
            }
            Provider::OpenAILike(provider) => {
                let config = provider.config();
                let api_key = config
                    .base
                    .api_key
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        ProviderError::configuration("rerank_proxy", "rerank requires an API key")
                    })?;
                let normalized = provider
                    .name()
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                if normalized.contains("cohere") {
                    return CohereRerankProvider::new_with_endpoint(
                        api_key,
                        config.get_api_base(),
                        config.base.endpoint_access,
                        config.base.timeout,
                    )
                    .map(|adapter| Arc::new(adapter) as Arc<dyn RerankProvider>)
                    .map_err(|error| ProviderError::configuration("cohere", error.to_string()));
                }
                if normalized.contains("jina") {
                    return JinaRerankProvider::new_with_endpoint(
                        api_key,
                        config.get_api_base(),
                        config.base.endpoint_access,
                        config.base.timeout,
                    )
                    .map(|adapter| Arc::new(adapter) as Arc<dyn RerankProvider>)
                    .map_err(|error| ProviderError::configuration("jina", error.to_string()));
                }
                Err(ProviderError::not_supported("rerank_proxy", "rerank"))
            }
            #[cfg(feature = "providers-extended")]
            Provider::Cohere(provider) => {
                let config = provider.config();
                CohereRerankProvider::new_with_endpoint(
                    config.api_key.clone(),
                    format!("{}/v1", config.api_base.trim_end_matches('/')),
                    config.endpoint_access,
                    config.timeout_seconds,
                )
                .map(|adapter| Arc::new(adapter) as Arc<dyn RerankProvider>)
                .map_err(|error| ProviderError::configuration("cohere", error.to_string()))
            }
            _ => Err(ProviderError::not_supported("provider", "rerank")),
        }
    }
}
