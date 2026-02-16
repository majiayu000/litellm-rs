//! Embedding router - Routes embedding requests to appropriate providers
//!
//! This module handles parsing model strings to identify providers
//! and routes requests to the appropriate embedding implementation.

use crate::core::providers::{Provider, ProviderRegistry};
use crate::core::types::responses::EmbeddingResponse;
use crate::core::types::{
    embedding::EmbeddingInput as TypesEmbeddingInput, embedding::EmbeddingRequest,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::debug;

use super::options::EmbeddingOptions;
use super::types::EmbeddingInput;

/// Default embedding router using the provider registry
pub struct EmbeddingRouter {
    provider_registry: Arc<ProviderRegistry>,
}

impl EmbeddingRouter {
    /// Create a new embedding router
    pub async fn new() -> Result<Self> {
        let mut provider_registry = ProviderRegistry::new();

        // Add OpenAI provider if API key is available
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            use crate::core::providers::base::BaseConfig;
            use crate::core::providers::openai::OpenAIProvider;
            use crate::core::providers::openai::config::OpenAIConfig;

            let config = OpenAIConfig {
                base: BaseConfig {
                    api_key: Some(api_key),
                    api_base: Some("https://api.openai.com/v1".to_string()),
                    timeout: 60,
                    max_retries: 3,
                    headers: Default::default(),
                    organization: std::env::var("OPENAI_ORGANIZATION").ok(),
                    api_version: None,
                },
                organization: std::env::var("OPENAI_ORGANIZATION").ok(),
                project: None,
                model_mappings: Default::default(),
                features: Default::default(),
            };

            if let Ok(openai_provider) = OpenAIProvider::new(config).await {
                provider_registry.register(Provider::OpenAI(openai_provider));
            }
        }

        Ok(Self {
            provider_registry: Arc::new(provider_registry),
        })
    }

    /// Parse model string to extract provider and actual model name
    ///
    /// Supports formats like:
    /// - "openai/text-embedding-ada-002" -> ("openai", "text-embedding-ada-002")
    /// - "text-embedding-ada-002" -> ("openai", "text-embedding-ada-002") (default)
    /// - "anthropic/voyage-3" -> ("anthropic", "voyage-3")
    pub fn parse_model(model: &str) -> (&str, &str) {
        if let Some(idx) = model.find('/') {
            let (provider, rest) = model.split_at(idx);
            (provider, &rest[1..]) // Skip the '/'
        } else {
            // Default to OpenAI for models without prefix
            ("openai", model)
        }
    }

    /// Route an embedding request to the appropriate provider
    pub async fn embed(
        &self,
        model: &str,
        input: EmbeddingInput,
        options: EmbeddingOptions,
    ) -> Result<EmbeddingResponse> {
        let (provider_name, actual_model) = Self::parse_model(model);

        debug!(
            provider = %provider_name,
            model = %actual_model,
            "Routing embedding request"
        );

        // Check if we can use dynamic provider creation
        if let Some(response) = self
            .try_dynamic_provider_embed(provider_name, actual_model, &input, &options)
            .await?
        {
            return Ok(response);
        }

        // Try to find a registered provider
        let providers = self.provider_registry.get_all_providers();

        // Find matching provider from registry
        for provider in providers.iter() {
            if provider.name() == provider_name {
                return self
                    .execute_embedding(provider, actual_model, &input, &options)
                    .await;
            }
        }

        // No matching provider found
        Err(GatewayError::not_found(format!(
            "No embedding provider found for '{}'. Make sure the API key is set.",
            provider_name
        )))
    }

    /// Execute embedding using a specific provider
    async fn execute_embedding(
        &self,
        provider: &Provider,
        model: &str,
        input: &EmbeddingInput,
        options: &EmbeddingOptions,
    ) -> Result<EmbeddingResponse> {
        let request = self.build_request(model, input, options);

        match provider {
            Provider::OpenAI(p) => p
                .embeddings(request)
                .await
                .map_err(|e| GatewayError::internal(format!("OpenAI embedding error: {}", e))),
            // Add other providers as they support embeddings
            _ => Err(GatewayError::not_implemented(format!(
                "Provider '{}' does not support embeddings",
                provider.name()
            ))),
        }
    }

    /// Build an EmbeddingRequest from input and options
    fn build_request(
        &self,
        model: &str,
        input: &EmbeddingInput,
        options: &EmbeddingOptions,
    ) -> EmbeddingRequest {
        let types_input = match input {
            EmbeddingInput::Text(text) => TypesEmbeddingInput::Text(text.clone()),
            EmbeddingInput::TextArray(texts) => TypesEmbeddingInput::Array(texts.clone()),
        };

        EmbeddingRequest {
            model: model.to_string(),
            input: types_input,
            user: options.user.clone(),
            encoding_format: options.encoding_format.clone(),
            dimensions: options.dimensions,
            task_type: options.task_type.clone(),
        }
    }

    /// Try to create a dynamic provider for the embedding request
    async fn try_dynamic_provider_embed(
        &self,
        provider_name: &str,
        model: &str,
        input: &EmbeddingInput,
        options: &EmbeddingOptions,
    ) -> Result<Option<EmbeddingResponse>> {
        // Only proceed if user provided an API key
        let api_key = match &options.api_key {
            Some(key) => key.clone(),
            None => return Ok(None),
        };

        let api_base = match provider_name {
            "openai" => options
                .api_base
                .clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            "azure" | "azure_ai" | "azure-ai" => options
                .api_base
                .clone()
                .or_else(|| std::env::var("AZURE_AI_API_BASE").ok())
                .unwrap_or_else(|| "https://api.azure.com".to_string()),
            _ => match &options.api_base {
                Some(base) => base.clone(),
                None => return Ok(None),
            },
        };

        debug!(
            provider = %provider_name,
            model = %model,
            "Creating dynamic embedding provider"
        );

        // Create dynamic OpenAI-compatible provider
        let response = self
            .create_dynamic_openai_embedding(&api_key, &api_base, model, input, options)
            .await?;

        Ok(Some(response))
    }

    /// Create a dynamic OpenAI-compatible provider for embeddings
    async fn create_dynamic_openai_embedding(
        &self,
        api_key: &str,
        api_base: &str,
        model: &str,
        input: &EmbeddingInput,
        options: &EmbeddingOptions,
    ) -> Result<EmbeddingResponse> {
        use crate::core::providers::base::BaseConfig;
        use crate::core::providers::openai::OpenAIProvider;
        use crate::core::providers::openai::config::OpenAIConfig;

        let timeout = options.timeout.unwrap_or(60);

        let config = OpenAIConfig {
            base: BaseConfig {
                api_key: Some(api_key.to_string()),
                api_base: Some(api_base.to_string()),
                timeout,
                max_retries: 3,
                headers: options.headers.clone().unwrap_or_default(),
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
                "Failed to create dynamic embedding provider: {}",
                e
            ))
        })?;

        let request = self.build_request(model, input, options);

        provider
            .embeddings(request)
            .await
            .map_err(|e| GatewayError::internal(format!("Dynamic embedding error: {}", e)))
    }
}

/// Global router instance
static GLOBAL_EMBEDDING_ROUTER: OnceCell<EmbeddingRouter> = OnceCell::const_new();

/// Get or initialize the global embedding router
pub async fn get_global_embedding_router() -> Result<&'static EmbeddingRouter> {
    GLOBAL_EMBEDDING_ROUTER
        .get_or_try_init(|| async {
            EmbeddingRouter::new().await.map_err(|e| {
                GatewayError::internal(format!("Failed to initialize embedding router: {}", e))
            })
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_model_with_provider() {
        let (provider, model) = EmbeddingRouter::parse_model("openai/text-embedding-ada-002");
        assert_eq!(provider, "openai");
        assert_eq!(model, "text-embedding-ada-002");
    }

    #[test]
    fn test_parse_model_without_provider() {
        let (provider, model) = EmbeddingRouter::parse_model("text-embedding-ada-002");
        assert_eq!(provider, "openai");
        assert_eq!(model, "text-embedding-ada-002");
    }

    #[test]
    fn test_parse_model_anthropic() {
        let (provider, model) = EmbeddingRouter::parse_model("anthropic/voyage-3");
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "voyage-3");
    }

    #[test]
    fn test_parse_model_azure() {
        let (provider, model) = EmbeddingRouter::parse_model("azure/text-embedding-3-small");
        assert_eq!(provider, "azure");
        assert_eq!(model, "text-embedding-3-small");
    }
}
