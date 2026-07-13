//! Policy-aware HTTP client for Azure AI Foundry.

use std::sync::Arc;

use reqwest::{Method, header::HeaderMap};

use super::config::AzureAIConfig;
use crate::core::providers::base::{BaseHttpClient, ProviderRequestBuilder};
use crate::core::providers::unified_provider::ProviderError;

#[derive(Debug, Clone)]
pub(crate) struct AzureAIClient {
    inner: Arc<AzureAIClientInner>,
}

#[derive(Debug)]
struct AzureAIClientInner {
    config: AzureAIConfig,
    headers: HeaderMap,
    http_client: BaseHttpClient,
    streaming_client: BaseHttpClient,
}

impl AzureAIClient {
    pub(crate) fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        config
            .validate_policy_client_settings()
            .map_err(|error| ProviderError::configuration("azure_ai", error))?;
        let headers = build_headers(&config)?;
        let http_client = BaseHttpClient::new_for_provider("azure_ai", config.base.clone())?;
        let streaming_client =
            BaseHttpClient::new_for_provider_streaming("azure_ai", config.base.clone())?;

        Ok(Self {
            inner: Arc::new(AzureAIClientInner {
                config,
                headers,
                http_client,
                streaming_client,
            }),
        })
    }

    pub(crate) fn get_config(&self) -> &AzureAIConfig {
        &self.inner.config
    }

    pub(crate) fn request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        Ok(self
            .inner
            .http_client
            .request(method, url)?
            .headers(self.inner.headers.clone()))
    }

    pub(crate) fn streaming_request(
        &self,
        method: Method,
        url: &str,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        Ok(self
            .inner
            .streaming_client
            .request(method, url)?
            .headers(self.inner.headers.clone()))
    }
}

fn build_headers(config: &AzureAIConfig) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    for (key, value) in config
        .create_default_headers()
        .map_err(|error| ProviderError::configuration("azure_ai", error))?
    {
        let name = reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|error| {
            ProviderError::configuration("azure_ai", format!("invalid header name {key}: {error}"))
        })?;
        let value = reqwest::header::HeaderValue::from_str(&value).map_err(|error| {
            ProviderError::configuration(
                "azure_ai",
                format!("invalid header value for {key}: {error}"),
            )
        })?;
        headers.insert(name, value);
    }
    Ok(headers)
}
