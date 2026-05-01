//! Firecrawl Provider Implementation

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::base::{
    HeaderPair, HttpErrorMapper, HttpMethod, get_pricing_db, header, header_owned,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::{FirecrawlClient, FirecrawlConfig, FirecrawlErrorMapper};

const PROVIDER_NAME: &str = "firecrawl";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

crate::define_pooled_http_provider_with_hooks!(
    provider: PROVIDER_NAME,
    struct_name: FirecrawlProvider,
    config: super::FirecrawlConfig,
    error_mapper: super::FirecrawlErrorMapper,
    model_info: FirecrawlClient::supported_models,
    capabilities: CAPABILITIES,
    url_builder: |provider: &FirecrawlProvider| -> String {
        format!("{}/chat/completions", provider.config.get_api_base())
    },
    http_method: HttpMethod::POST,
    supported_params: ["temperature", "max_tokens", "top_p", "stream", "stop"],
    build_headers: |provider: &FirecrawlProvider| -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = &provider.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        for (key, value) in &provider.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    },
    with_api_key: |api_key: String| -> Result<FirecrawlProvider, ProviderError> {
        let mut config = FirecrawlConfig::new(PROVIDER_NAME);
        config.base.api_key = Some(api_key);
        FirecrawlProvider::new(config)
    },
    map_openai_params: |_provider: &FirecrawlProvider,
                        params: HashMap<String, Value>,
                        _model: &str|
     -> Result<HashMap<String, Value>, ProviderError> { Ok(params) },
    request_transform: |_provider: &FirecrawlProvider, request: ChatRequest|
     -> Result<Value, ProviderError> { Ok(FirecrawlClient::transform_chat_request(request)) },
    response_transform: |_provider: &FirecrawlProvider,
                         raw_response: &[u8],
                         _model: &str,
                         _request_id: &str|
     -> Result<ChatResponse, ProviderError> {
        let response: ChatResponse = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        Ok(response)
    },
    error_map: |_provider: &FirecrawlProvider,
                status: u16,
                error_text: String,
                _request: &ChatRequest|
     -> ProviderError {
        ErrorMapper::map_http_error(&FirecrawlErrorMapper, status, &error_text)
    },
    health_check: |provider: &FirecrawlProvider| {
        let has_key = provider
            .config
            .base
            .get_effective_api_key(PROVIDER_NAME)
            .is_some();
        async move {
            if has_key {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            }
        }
    },
    streaming: |provider: &FirecrawlProvider, request: ChatRequest, _context: RequestContext| {
        let url = format!("{}/chat/completions", provider.config.get_api_base());
        let api_key = provider.config.base.get_effective_api_key(PROVIDER_NAME);

        let mut body = FirecrawlClient::transform_chat_request(request);
        body["stream"] = serde_json::Value::Bool(true);

        async move {
            let api_key = api_key.ok_or_else(|| {
                ProviderError::authentication(PROVIDER_NAME, "API key is required")
            })?;

            let client = crate::core::http::outbound::default_outbound_client().clone();
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(HttpErrorMapper::map_status_code(
                    PROVIDER_NAME,
                    status.as_u16(),
                    &error_text,
                ));
            }

            let stream = super::streaming::create_firecrawl_stream(response.bytes_stream());
            let stream: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>> =
                Box::pin(stream);
            Ok(stream)
        }
    },
    calculate_cost: |_provider: &FirecrawlProvider,
                     model: &str,
                     input_tokens: u32,
                     output_tokens: u32|
     -> Result<f64, ProviderError> {
        let usage = crate::core::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    },
);

impl FirecrawlProvider {
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = FirecrawlConfig::from_env();
        Self::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    fn create_test_config() -> FirecrawlConfig {
        let mut config = FirecrawlConfig::new("firecrawl");
        config.base.api_key = Some("test-key".to_string());
        config
    }

    #[test]
    fn test_provider_creation() {
        let config = create_test_config();
        let provider = FirecrawlProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = create_test_config();
        let provider = FirecrawlProvider::new(config).unwrap();
        assert_eq!(provider.name(), "firecrawl");
    }

    #[test]
    fn test_capabilities() {
        let config = create_test_config();
        let provider = FirecrawlProvider::new(config).unwrap();

        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    }
}
