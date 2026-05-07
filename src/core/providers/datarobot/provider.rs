//! DataRobot Provider Implementation

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

use super::{DataRobotClient, DataRobotConfig, DataRobotErrorMapper};

const PROVIDER_NAME: &str = "datarobot";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

crate::define_pooled_http_provider_with_hooks!(
    provider: PROVIDER_NAME,
    struct_name: DataRobotProvider,
    config: super::DataRobotConfig,
    error_mapper: super::DataRobotErrorMapper,
    model_info: DataRobotClient::supported_models,
    capabilities: CAPABILITIES,
    url_builder: |provider: &DataRobotProvider| -> String {
        format!("{}/chat/completions", provider.config.get_api_base())
    },
    http_method: HttpMethod::POST,
    supported_params: ["temperature", "max_tokens", "top_p", "stream", "stop"],
    build_headers: |provider: &DataRobotProvider| -> Vec<HeaderPair> {
        provider.get_request_headers()
    },
    with_api_key: |api_key: String| -> Result<DataRobotProvider, ProviderError> {
        let mut config = DataRobotConfig::new(PROVIDER_NAME);
        config.base.api_key = Some(api_key);
        config.base.api_base = Some("https://app.datarobot.com/api/v2".to_string());
        DataRobotProvider::new(config)
    },
    map_openai_params: |_provider: &DataRobotProvider,
                        params: HashMap<String, Value>,
                        _model: &str|
     -> Result<HashMap<String, Value>, ProviderError> { Ok(params) },
    request_transform: |_provider: &DataRobotProvider, request: ChatRequest|
     -> Result<Value, ProviderError> { Ok(DataRobotClient::transform_chat_request(request)) },
    response_transform: |_provider: &DataRobotProvider,
                         raw_response: &[u8],
                         _model: &str,
                         _request_id: &str|
     -> Result<ChatResponse, ProviderError> {
        let response: ChatResponse = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        Ok(response)
    },
    error_map: |_provider: &DataRobotProvider,
                status: u16,
                error_text: String,
                _request: &ChatRequest|
     -> ProviderError {
        ErrorMapper::map_http_error(&DataRobotErrorMapper, status, &error_text)
    },
    health_check: |provider: &DataRobotProvider| {
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
    streaming: |provider: &DataRobotProvider, request: ChatRequest, _context: RequestContext| {
        let url = format!("{}/chat/completions", provider.config.get_api_base());
        let api_key = provider.config.base.get_effective_api_key(PROVIDER_NAME);

        let mut body = DataRobotClient::transform_chat_request(request);
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

            let stream = super::streaming::create_datarobot_stream(response.bytes_stream());
            let stream: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>> =
                Box::pin(stream);
            Ok(stream)
        }
    },
    calculate_cost: |_provider: &DataRobotProvider,
                     model: &str,
                     input_tokens: u32,
                     output_tokens: u32|
     -> Result<f64, ProviderError> {
        let usage = crate::core::pricing::Usage::new(input_tokens, output_tokens);

        Ok(get_pricing_db().calculate(model, &usage))
    },
);

impl DataRobotProvider {
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = DataRobotConfig::from_env();
        Self::new(config)
    }

    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    fn create_test_config() -> DataRobotConfig {
        let mut config = DataRobotConfig::new("datarobot");
        config.base.api_key = Some("dr-test-key".to_string());
        config
    }

    #[test]
    fn test_provider_creation() {
        let config = create_test_config();
        let provider = DataRobotProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = create_test_config();
        let provider = DataRobotProvider::new(config).unwrap();
        assert_eq!(provider.name(), "datarobot");
    }

    #[test]
    fn test_provider_capabilities() {
        let config = create_test_config();
        let provider = DataRobotProvider::new(config).unwrap();
        let caps = provider.capabilities();

        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    }
}
