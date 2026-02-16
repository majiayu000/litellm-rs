//! Poe Provider Implementation

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use super::config::PoeConfig;
use super::model_info::get_models;
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig as _,
    provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};

const PROVIDER_NAME: &str = "poe";
type Error = ProviderError;

static POE_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

#[derive(Debug, Clone)]
pub struct PoeProvider {
    config: PoeConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl PoeProvider {
    pub fn new(config: PoeConfig) -> Result<Self, Error> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                PROVIDER_NAME,
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        Ok(Self {
            config,
            pool_manager,
            models: get_models(),
        })
    }

    pub fn from_env() -> Result<Self, Error> {
        Self::new(PoeConfig::from_env())
    }

    async fn execute_request(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let url = format!("{}{}", self.config.get_api_base(), endpoint);

        let mut headers = Vec::with_capacity(2);
        if let Some(api_key) = &self.config.get_api_key() {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }
        headers.push(header("Content-Type", "application/json".to_string()));

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        let status = response.status().as_u16();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !(200..300).contains(&status) {
            let error_body = String::from_utf8_lossy(&response_bytes);
            return Err(self.map_http_error(status, &error_body));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))
    }

    fn map_http_error(&self, status: u16, body: &str) -> Error {
        match status {
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Permission denied"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, body),
            429 => ProviderError::rate_limit(PROVIDER_NAME, None),
            400 => ProviderError::invalid_request(PROVIDER_NAME, body),
            500..=599 => ProviderError::api_error(PROVIDER_NAME, status, body),
            _ => ProviderError::api_error(PROVIDER_NAME, status, body),
        }
    }
}

pub struct PoeErrorMapper;

impl ErrorMapper<Error> for PoeErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> Error {
        match status_code {
            401 => ProviderError::authentication(
                PROVIDER_NAME,
                format!("Invalid API key: {}", response_body),
            ),
            403 => ProviderError::authentication(
                PROVIDER_NAME,
                format!("Permission denied: {}", response_body),
            ),
            404 => ProviderError::model_not_found(PROVIDER_NAME, response_body),
            429 => ProviderError::rate_limit(PROVIDER_NAME, None),
            500..=599 => ProviderError::api_error(
                PROVIDER_NAME,
                status_code,
                format!("Server error: {}", response_body),
            ),
            _ => ProviderError::api_error(
                PROVIDER_NAME,
                status_code,
                format!("HTTP {}: {}", status_code, response_body),
            ),
        }
    }
}

#[async_trait]
impl LLMProvider for PoeProvider {
    type Config = PoeConfig;
    type Error = Error;
    type ErrorMapper = PoeErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        POE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_p", "stream"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        use serde_json::json;

        let mut body = json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if request.stream {
            body["stream"] = json!(true);
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let response_text = std::str::from_utf8(raw_response).map_err(|e| {
            ProviderError::serialization(PROVIDER_NAME, format!("Invalid UTF-8: {}", e))
        })?;

        let response_json: ChatResponse = serde_json::from_str(response_text).map_err(|e| {
            ProviderError::serialization(PROVIDER_NAME, format!("Invalid JSON: {}", e))
        })?;

        Ok(ChatResponse {
            id: request_id.to_string(),
            model: model.to_string(),
            ..response_json
        })
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        PoeErrorMapper
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        Ok(0.0)
    }

    fn supports_model(&self, model: &str) -> bool {
        self.models.iter().any(|m| m.id == model || m.name == model)
    }

    async fn health_check(&self) -> HealthStatus {
        if self.config.get_api_key().is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let body = self.transform_request(request.clone(), context).await?;
        let response = self.execute_request("/chat/completions", body).await?;

        let chat_response: ChatResponse = serde_json::from_value(response).map_err(|e| {
            ProviderError::serialization(
                PROVIDER_NAME,
                format!("Failed to parse ChatResponse: {}", e),
            )
        })?;

        Ok(chat_response)
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        Err(ProviderError::not_implemented(
            PROVIDER_NAME,
            "Streaming not yet implemented",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(ProviderError::not_supported(PROVIDER_NAME, "Embeddings"))
    }

    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, Self::Error> {
        Err(ProviderError::not_supported(
            PROVIDER_NAME,
            "Image generation",
        ))
    }
}
