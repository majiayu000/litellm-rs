//! Embedding methods backed by core embedding capabilities.

use super::llm_client::LLMClient;
use crate::core::embedding::{
    EmbeddingOptions as CoreEmbeddingOptions, embedding as core_embedding,
};
use crate::sdk::config::{ProviderType, SdkProviderConfig};
use crate::sdk::errors::*;
use crate::utils::net::ClientUtils;

impl LLMClient {
    /// Generate embeddings for a single text via the core embedding path.
    pub async fn embedding(&self, text: &str, model: Option<&str>) -> Result<Vec<f32>> {
        let (resolved_model, options) = self.prepare_embedding_request(model)?;
        let response = core_embedding(&resolved_model, text, Some(options))
            .await
            .map_err(SDKError::from)?;

        response
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .ok_or_else(|| SDKError::Internal("No embedding data in response".to_string()))
    }

    /// Generate embeddings for multiple texts in batch via the core embedding path.
    pub async fn batch_embedding(
        &self,
        texts: &[String],
        model: Option<&str>,
    ) -> Result<Vec<Vec<f32>>> {
        let (resolved_model, options) = self.prepare_embedding_request(model)?;
        let response = core_embedding(&resolved_model, texts.to_vec(), Some(options))
            .await
            .map_err(SDKError::from)?;

        let mut embeddings: Vec<(u32, Vec<f32>)> = response
            .data
            .into_iter()
            .map(|item| (item.index, item.embedding))
            .collect();
        embeddings.sort_by_key(|(idx, _)| *idx);

        Ok(embeddings
            .into_iter()
            .map(|(_, embedding)| embedding)
            .collect())
    }

    pub(crate) fn prepare_embedding_request(
        &self,
        model: Option<&str>,
    ) -> Result<(String, CoreEmbeddingOptions)> {
        let provider = self.embedding_provider(model)?;
        let provider_prefix = self.embedding_provider_prefix(provider)?;
        let resolved_model = match model {
            Some(model) => self.qualify_embedding_model(model, provider, provider_prefix),
            None => {
                let default_model = provider.models.first().ok_or_else(|| {
                    SDKError::NotSupported(format!(
                        "Embedding requires an explicit embedding model for provider '{}'",
                        provider.id
                    ))
                })?;
                self.qualify_embedding_model(default_model, provider, provider_prefix)
            }
        };
        let api_base = self.embedding_api_base(provider)?;

        if model.is_none()
            && matches!(
                provider.provider_type,
                ProviderType::OpenAI | ProviderType::Azure
            )
            && !resolved_model.to_ascii_lowercase().contains("embedding")
        {
            return Err(SDKError::NotSupported(format!(
                "Embedding default model for provider '{}' must be an embedding model, got '{}'",
                provider.id, resolved_model
            )));
        }

        let options = CoreEmbeddingOptions::new()
            .with_api_key(provider.api_key.clone())
            .with_api_base(api_base)
            .with_timeout(self.config.settings.timeout);

        Ok((resolved_model, options))
    }

    fn embedding_provider(&self, model: Option<&str>) -> Result<&SdkProviderConfig> {
        if let Some(model) = model {
            if let Some((prefix, _)) = model.split_once('/') {
                if let Some(provider) = self
                    .default_enabled_provider()
                    .filter(|provider| {
                        self.embedding_prefix_matches_provider_for_model(prefix, provider)
                    })
                    .or_else(|| {
                        self.config.providers.iter().find(|provider| {
                            provider.enabled
                                && self
                                    .embedding_prefix_matches_provider_for_model(prefix, provider)
                        })
                    })
                {
                    self.ensure_embedding_supported(provider)?;
                    return Ok(provider);
                }

                if let Ok(provider) = self.provider_for_model(model) {
                    self.ensure_embedding_supported(provider)?;
                    return Ok(provider);
                }

                return Err(SDKError::ProviderNotFound(prefix.to_string()));
            }

            if let Ok(provider) = self.provider_for_model(model) {
                self.ensure_embedding_supported(provider)?;
                return Ok(provider);
            }
        }

        let provider = self
            .default_enabled_provider()
            .or_else(|| {
                self.config
                    .providers
                    .iter()
                    .find(|provider| provider.enabled)
            })
            .ok_or(SDKError::NoDefaultProvider)?;

        self.ensure_embedding_supported(provider)?;
        Ok(provider)
    }

    fn ensure_embedding_supported(&self, provider: &SdkProviderConfig) -> Result<()> {
        self.embedding_provider_prefix(provider).map(|_| ())
    }

    fn qualify_embedding_model(
        &self,
        model: &str,
        provider: &SdkProviderConfig,
        provider_prefix: &str,
    ) -> String {
        if let Some((prefix, _)) = model.split_once('/')
            && self.embedding_prefix_matches_provider(prefix, provider, provider_prefix)
        {
            return model.to_string();
        }

        format!("{}/{}", provider_prefix, model)
    }

    fn embedding_prefix_matches_provider_for_model(
        &self,
        prefix: &str,
        provider: &SdkProviderConfig,
    ) -> bool {
        let Ok(provider_prefix) = self.embedding_provider_prefix(provider) else {
            return false;
        };

        self.embedding_prefix_matches_provider(prefix, provider, provider_prefix)
    }

    fn embedding_prefix_matches_provider(
        &self,
        prefix: &str,
        provider: &SdkProviderConfig,
        provider_prefix: &str,
    ) -> bool {
        prefix == provider.id
            || prefix == provider_prefix
            || (matches!(provider.provider_type, ProviderType::Azure)
                && matches!(prefix, "azure_ai" | "azure-ai"))
    }

    fn embedding_provider_prefix<'a>(&self, provider: &'a SdkProviderConfig) -> Result<&'a str> {
        match &provider.provider_type {
            ProviderType::OpenAI => Ok("openai"),
            ProviderType::Azure => Ok("azure"),
            ProviderType::Custom(name) => Ok(name.as_str()),
            _ => Err(SDKError::NotSupported(format!(
                "Embedding is not supported for SDK provider type {:?}",
                provider.provider_type
            ))),
        }
    }

    fn embedding_api_base(&self, provider: &SdkProviderConfig) -> Result<String> {
        match &provider.provider_type {
            ProviderType::OpenAI => {
                let base_url = self.provider_base_url(provider, "https://api.openai.com");
                let normalized = if base_url.contains("/v1") {
                    base_url.trim_end_matches('/').to_string()
                } else {
                    ClientUtils::add_path_to_api_base(base_url, "v1")
                };
                Ok(normalized)
            }
            ProviderType::Azure | ProviderType::Custom(_) => {
                provider.base_url.clone().ok_or_else(|| {
                    SDKError::NotSupported(format!(
                        "Embedding requires an explicit base_url for SDK provider type {:?}",
                        provider.provider_type
                    ))
                })
            }
            _ => Err(SDKError::NotSupported(format!(
                "Embedding is not supported for SDK provider type {:?}",
                provider.provider_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::config::{ConfigBuilder, ProviderType, SdkProviderConfig};
    use std::collections::HashMap;

    fn test_provider_config(
        id: &str,
        provider_type: ProviderType,
        model: &str,
    ) -> SdkProviderConfig {
        SdkProviderConfig {
            id: id.to_string(),
            provider_type,
            name: format!("{id} provider"),
            api_key: "test-key".to_string(),
            base_url: None,
            models: vec![model.to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        }
    }

    #[test]
    fn test_prepare_embedding_request_uses_explicit_model() {
        let config = ConfigBuilder::new()
            .add_provider(test_provider_config(
                "openai",
                ProviderType::OpenAI,
                "text-embedding-3-small",
            ))
            .build();

        let client = LLMClient::new(config).unwrap();
        let (model, options) = client
            .prepare_embedding_request(Some("text-embedding-3-large"))
            .unwrap();

        assert_eq!(model, "openai/text-embedding-3-large");
        assert_eq!(options.api_key.as_deref(), Some("test-key"));
        assert_eq!(
            options.api_base.as_deref(),
            Some("https://api.openai.com/v1")
        );
        assert_eq!(options.timeout, Some(30));
    }

    #[test]
    fn test_prepare_embedding_request_uses_default_provider_model() {
        let config = ConfigBuilder::new()
            .default_provider("openai")
            .add_provider(test_provider_config(
                "openai",
                ProviderType::OpenAI,
                "text-embedding-3-small",
            ))
            .build();

        let client = LLMClient::new(config).unwrap();
        let (model, _options) = client.prepare_embedding_request(None).unwrap();

        assert_eq!(model, "openai/text-embedding-3-small");
    }

    #[test]
    fn test_prepare_embedding_request_rejects_unsupported_provider_types() {
        let config = ConfigBuilder::new()
            .default_provider("anthropic")
            .add_provider(test_provider_config(
                "anthropic",
                ProviderType::Anthropic,
                "claude-sonnet-4-5",
            ))
            .build();

        let client = LLMClient::new(config).unwrap();
        let err = client.prepare_embedding_request(None).unwrap_err();

        assert!(matches!(err, SDKError::NotSupported(_)));
    }

    #[test]
    fn test_prepare_embedding_request_supports_custom_provider_with_explicit_base_url() {
        let config = ConfigBuilder::new()
            .default_provider("custom-embed")
            .add_provider(SdkProviderConfig {
                base_url: Some("https://embeddings.example.com/v1".to_string()),
                ..test_provider_config(
                    "custom-embed",
                    ProviderType::Custom("custom-embed".to_string()),
                    "embed-1",
                )
            })
            .build();

        let client = LLMClient::new(config).unwrap();
        let (model, options) = client.prepare_embedding_request(None).unwrap();

        assert_eq!(model, "custom-embed/embed-1");
        assert_eq!(
            options.api_base.as_deref(),
            Some("https://embeddings.example.com/v1")
        );
    }

    #[test]
    fn test_prepare_embedding_request_rejects_unknown_provider_prefix() {
        let config = ConfigBuilder::new()
            .default_provider("openai")
            .add_provider(test_provider_config(
                "openai",
                ProviderType::OpenAI,
                "text-embedding-3-small",
            ))
            .build();

        let client = LLMClient::new(config).unwrap();
        let err = client
            .prepare_embedding_request(Some("unknown/text-embedding-3-small"))
            .unwrap_err();

        assert!(matches!(err, SDKError::ProviderNotFound(ref provider) if provider == "unknown"));
    }

    #[test]
    fn test_prepare_embedding_request_supports_models_with_slashes_when_configured() {
        let config = ConfigBuilder::new()
            .default_provider("openrouter")
            .add_provider(SdkProviderConfig {
                base_url: Some("https://openrouter.example.com/v1".to_string()),
                ..test_provider_config(
                    "openrouter",
                    ProviderType::Custom("openrouter".to_string()),
                    "google/text-embedding-004",
                )
            })
            .build();

        let client = LLMClient::new(config).unwrap();
        let (model, options) = client
            .prepare_embedding_request(Some("google/text-embedding-004"))
            .unwrap();

        assert_eq!(model, "openrouter/google/text-embedding-004");
        assert_eq!(
            options.api_base.as_deref(),
            Some("https://openrouter.example.com/v1")
        );
    }

    #[test]
    fn test_prepare_embedding_request_without_model_rejects_provider_without_embedding_model() {
        let config = ConfigBuilder::new()
            .default_provider("openai")
            .add_provider(test_provider_config(
                "openai",
                ProviderType::OpenAI,
                "gpt-5.2-chat",
            ))
            .build();

        let client = LLMClient::new(config).unwrap();
        let err = client.prepare_embedding_request(None).unwrap_err();

        assert!(matches!(err, SDKError::NotSupported(_)));
    }

    #[test]
    fn test_prepare_embedding_request_accepts_openai_prefixed_model_for_custom_provider_id() {
        let config = ConfigBuilder::new()
            .default_provider("primary-openai")
            .add_provider(SdkProviderConfig {
                id: "primary-openai".to_string(),
                base_url: Some("https://api.openai.com".to_string()),
                ..test_provider_config(
                    "primary-openai",
                    ProviderType::OpenAI,
                    "text-embedding-3-small",
                )
            })
            .build();

        let client = LLMClient::new(config).unwrap();
        let (model, options) = client
            .prepare_embedding_request(Some("openai/text-embedding-3-large"))
            .unwrap();

        assert_eq!(model, "openai/text-embedding-3-large");
        assert_eq!(
            options.api_base.as_deref(),
            Some("https://api.openai.com/v1")
        );
    }

    #[test]
    fn test_prepare_embedding_request_rejects_azure_non_embedding_default_model() {
        let config = ConfigBuilder::new()
            .default_provider("azure")
            .add_provider(SdkProviderConfig {
                base_url: Some("https://azure.example.com/openai/deployments/foo".to_string()),
                ..test_provider_config("azure", ProviderType::Azure, "gpt-5.2-chat")
            })
            .build();

        let client = LLMClient::new(config).unwrap();
        let err = client.prepare_embedding_request(None).unwrap_err();

        assert!(matches!(err, SDKError::NotSupported(_)));
    }
}
