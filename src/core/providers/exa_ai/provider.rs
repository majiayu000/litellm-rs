//! ExaAi Provider Implementation

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, get_pricing_db, header, header_owned,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::{ExaAiClient, ExaAiConfig, ExaAiErrorMapper};

#[derive(Debug, Clone)]
pub struct ExaAiProvider {
    config: ExaAiConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl ExaAiProvider {
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

    pub fn new(config: ExaAiConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("exa_ai", e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration("exa_ai", e.to_string()))?,
        );
        let supported_models = ExaAiClient::supported_models();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    pub fn from_env() -> Result<Self, ProviderError> {
        let config = ExaAiConfig::from_env();
        Self::new(config)
    }

    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        let mut config = ExaAiConfig::new("exa_ai");
        config.base.api_key = Some(api_key.into());
        Self::new(config)
    }
}

#[async_trait]
impl LLMProvider for ExaAiProvider {
    type Config = ExaAiConfig;
    type Error = ProviderError;
    type ErrorMapper = ExaAiErrorMapper;

    fn name(&self) -> &'static str {
        "exa_ai"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        ExaAiClient::supported_openai_params()
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        Ok(ExaAiClient::transform_chat_request(request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response: ChatResponse = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("exa_ai", e.to_string()))?;
        Ok(response)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        ExaAiErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let url = format!("{}/chat/completions", self.config.get_api_base());
        let body = ExaAiClient::transform_chat_request(request.clone());

        let headers = self.get_request_headers();
        let body_data = Some(body);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body_data)
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("exa_ai", e.to_string()))?;

        if !status.is_success() {
            let error_text = String::from_utf8_lossy(&response_bytes);
            let mapper = self.get_error_mapper();
            return Err(
                crate::core::traits::error_mapper::trait_def::ErrorMapper::map_http_error(
                    &mapper,
                    status.as_u16(),
                    &error_text,
                ),
            );
        }

        self.transform_response(&response_bytes, &request.model, &context.request_id)
            .await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        let url = format!("{}/chat/completions", self.config.get_api_base());

        let mut body = ExaAiClient::transform_chat_request(request.clone());
        body["stream"] = serde_json::Value::Bool(true);

        let api_key = self
            .config
            .base
            .get_effective_api_key("exa_ai")
            .ok_or_else(|| ProviderError::authentication("exa_ai", "API key is required"))?;

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("exa_ai", e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error(
                "exa_ai",
                status.as_u16(),
                error_text,
            ));
        }

        let stream = response.bytes_stream();
        Ok(Box::pin(super::streaming::create_exa_ai_stream(stream)))
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.base.get_effective_api_key("exa_ai").is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ExaAiConfig {
        let mut config = ExaAiConfig::new("exa_ai");
        config.base.api_key = Some("test-key".to_string());
        config
    }

    #[test]
    fn test_provider_creation() {
        let config = create_test_config();
        let provider = ExaAiProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let config = create_test_config();
        let provider = ExaAiProvider::new(config).unwrap();
        assert_eq!(provider.name(), "exa_ai");
    }
}
