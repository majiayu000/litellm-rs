//! Main Triton Provider Implementation
//!
//! Implements the LLMProvider trait for NVIDIA Triton Inference Server.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::TritonConfig;
use super::error::TritonError;
use super::models::{
    ModelMetadataResponse, TritonInferRequest, TritonInferResponse, TritonModelInfo, TritonTensor,
};
use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, header, header_owned,
};
use crate::core::traits::{
    provider::ProviderConfig, provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    message::MessageContent,
    message::MessageRole,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChoice, ChatChunk, ChatResponse, EmbeddingResponse, FinishReason, Usage},
};

const PROVIDER_NAME: &str = "triton";

/// Static capabilities for Triton provider
const TRITON_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ChatCompletion];

/// Triton provider implementation
#[derive(Debug, Clone)]
pub struct TritonProvider {
    config: TritonConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl TritonProvider {
    /// Create a new Triton provider instance
    pub async fn new(config: TritonConfig) -> Result<Self, TritonError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| TritonError::configuration(PROVIDER_NAME, e))?;

        // Create pool manager
        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            TritonError::configuration(
                PROVIDER_NAME,
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        // Initialize with empty models list - will be populated from server
        let models = Vec::new();

        Ok(Self {
            config,
            pool_manager,
            models,
        })
    }

    /// Create provider with server URL only
    pub async fn with_server_url(server_url: impl Into<String>) -> Result<Self, TritonError> {
        let config = TritonConfig::new(server_url);
        Self::new(config).await
    }

    /// Create provider from environment variables
    pub async fn from_env() -> Result<Self, TritonError> {
        let config = TritonConfig::default();
        Self::new(config).await
    }

    /// Get the base URL for API requests
    fn get_base_url(&self) -> String {
        self.config.get_server_url()
    }

    /// Build the model endpoint URL
    fn get_model_url(&self, model: &str, version: Option<&str>) -> String {
        let base = self.get_base_url();
        match version {
            Some(v) => format!("{}/v2/models/{}/versions/{}", base, model, v),
            None => format!("{}/v2/models/{}", base, model),
        }
    }

    /// Build default headers for requests
    fn build_headers(&self) -> Vec<HeaderPair> {
        let mut headers = vec![header("Content-Type", "application/json".to_string())];

        // Add custom headers from config
        for (key, value) in &self.config.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Check if the Triton server is healthy
    pub async fn is_healthy(&self) -> bool {
        let url = format!("{}/v2/health/ready", self.get_base_url());
        let headers = self.build_headers();

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Check if a specific model is ready
    pub async fn is_model_ready(&self, model: &str) -> Result<bool, TritonError> {
        let url = format!("{}/v2/models/{}/ready", self.get_base_url(), model);
        let headers = self.build_headers();

        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
        {
            Ok(response) => Ok(response.status().is_success()),
            Err(e) => Err(TritonError::network(PROVIDER_NAME, e.to_string())),
        }
    }

    /// Get model metadata from Triton server
    pub async fn get_model_metadata(
        &self,
        model: &str,
    ) -> Result<ModelMetadataResponse, TritonError> {
        let url = self.get_model_url(model, self.config.get_model_version().as_deref());
        let headers = self.build_headers();

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None::<serde_json::Value>)
            .await
            .map_err(|e| TritonError::network(PROVIDER_NAME, e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(self.map_http_error(
                status,
                &format!("Failed to get model metadata for {}", model),
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| TritonError::network(PROVIDER_NAME, e.to_string()))?;

        serde_json::from_slice(&bytes).map_err(|e| {
            TritonError::response_parsing(
                PROVIDER_NAME,
                format!("Failed to parse model metadata: {}", e),
            )
        })
    }

    /// Get detailed model info from Triton server
    pub async fn get_triton_model_info(&self, model: &str) -> Result<TritonModelInfo, TritonError> {
        let metadata = self.get_model_metadata(model).await?;

        Ok(TritonModelInfo {
            name: metadata.name,
            version: metadata.versions.first().cloned(),
            state: Some("READY".to_string()),
            platform: metadata.platform,
            max_batch_size: None,
            inputs: metadata
                .inputs
                .into_iter()
                .map(|t| super::models::TensorInfo {
                    name: t.name,
                    datatype: t.datatype,
                    shape: t.shape,
                })
                .collect(),
            outputs: metadata
                .outputs
                .into_iter()
                .map(|t| super::models::TensorInfo {
                    name: t.name,
                    datatype: t.datatype,
                    shape: t.shape,
                })
                .collect(),
            parameters: HashMap::new(),
        })
    }

    /// Execute inference request on Triton server
    async fn infer(
        &self,
        model: &str,
        request: TritonInferRequest,
    ) -> Result<TritonInferResponse, TritonError> {
        let url = format!(
            "{}/infer",
            self.get_model_url(model, self.config.get_model_version().as_deref())
        );
        let headers = self.build_headers();

        debug!("Triton inference request: model={}, url={}", model, url);

        let request_body = serde_json::to_value(&request)
            .map_err(|e| TritonError::invalid_request(PROVIDER_NAME, e.to_string()))?;

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(request_body))
            .await
            .map_err(|e| TritonError::network(PROVIDER_NAME, e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(self.map_http_error(status, &body));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| TritonError::network(PROVIDER_NAME, e.to_string()))?;

        serde_json::from_slice(&bytes).map_err(|e| {
            TritonError::response_parsing(
                PROVIDER_NAME,
                format!("Failed to parse inference response: {}", e),
            )
        })
    }

    /// Map HTTP error status to ProviderError
    fn map_http_error(&self, status: u16, body: &str) -> TritonError {
        match status {
            400 => TritonError::invalid_request(PROVIDER_NAME, body),
            401 | 403 => TritonError::authentication(PROVIDER_NAME, "Authentication failed"),
            404 => TritonError::model_not_found(PROVIDER_NAME, body),
            408 => TritonError::timeout(PROVIDER_NAME, "Request timeout"),
            429 => TritonError::rate_limit(PROVIDER_NAME, None),
            500..=599 => TritonError::provider_unavailable(PROVIDER_NAME, body),
            _ => TritonError::api_error(PROVIDER_NAME, status, body),
        }
    }

    /// Convert chat messages to Triton inference request
    fn build_inference_request(&self, request: &ChatRequest) -> TritonInferRequest {
        // Serialize messages to a prompt string
        // This is a simple implementation - actual format depends on model
        let prompt = request
            .messages
            .iter()
            .map(|m| {
                let role = format!("{:?}", m.role).to_lowercase();
                format!(
                    "{}: {}",
                    role,
                    m.content
                        .as_ref()
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut parameters = HashMap::new();

        // Add generation parameters
        if let Some(temp) = request.temperature {
            parameters.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tokens) = request.max_tokens {
            parameters.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
        }
        if let Some(top_p) = request.top_p {
            parameters.insert("top_p".to_string(), serde_json::json!(top_p));
        }

        TritonInferRequest {
            id: Some(uuid::Uuid::new_v4().to_string()),
            inputs: vec![TritonTensor {
                name: "text_input".to_string(),
                datatype: "BYTES".to_string(),
                shape: vec![1],
                data: serde_json::json!([prompt]),
                parameters: None,
            }],
            outputs: Some(vec![super::models::TritonOutputRequest {
                name: "text_output".to_string(),
                parameters: None,
            }]),
            parameters: if parameters.is_empty() {
                None
            } else {
                Some(parameters)
            },
        }
    }

    /// Convert Triton response to ChatResponse
    fn build_chat_response(
        &self,
        model: &str,
        response: TritonInferResponse,
        request_id: &str,
    ) -> Result<ChatResponse, TritonError> {
        // Extract text output from response
        let text_output = response
            .outputs
            .iter()
            .find(|o| o.name == "text_output" || o.name.contains("output"))
            .ok_or_else(|| {
                TritonError::response_parsing(PROVIDER_NAME, "No output tensor found in response")
            })?;

        // Parse the output data
        let content = match &text_output.data {
            serde_json::Value::Array(arr) => arr
                .first()
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            serde_json::Value::String(s) => s.clone(),
            _ => text_output.data.to_string(),
        };

        Ok(ChatResponse {
            id: request_id.to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 0,     // Triton doesn't typically return token counts
                completion_tokens: 0, // Would need tokenizer to calculate
                total_tokens: 0,
                completion_tokens_details: None,
                prompt_tokens_details: None,
                thinking_usage: None,
            }),
            system_fingerprint: None,
        })
    }
}

#[async_trait]
impl LLMProvider for TritonProvider {
    type Config = TritonConfig;
    type Error = TritonError;
    type ErrorMapper = crate::core::traits::error_mapper::DefaultErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        TRITON_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        // Triton models may support various parameters depending on deployment
        &[
            "temperature",
            "max_tokens",
            "top_p",
            "top_k",
            "stop",
            "stream",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, Self::Error> {
        // Triton uses similar parameters, pass through
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, Self::Error> {
        let triton_request = self.build_inference_request(&request);
        serde_json::to_value(&triton_request)
            .map_err(|e| TritonError::invalid_request(PROVIDER_NAME, e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let triton_response: TritonInferResponse = serde_json::from_slice(raw_response)
            .map_err(|e| TritonError::response_parsing(PROVIDER_NAME, e.to_string()))?;

        self.build_chat_response(model, triton_response, request_id)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        crate::core::traits::error_mapper::DefaultErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let model = if request.model.is_empty() {
            self.config.get_model_name().ok_or_else(|| {
                TritonError::configuration(PROVIDER_NAME, "Model name not specified")
            })?
        } else {
            request.model.clone()
        };

        debug!("Triton chat request: model={}", model);

        // Check if model is ready
        let is_ready = self.is_model_ready(&model).await?;
        if !is_ready {
            return Err(TritonError::model_not_found(
                PROVIDER_NAME,
                format!("Model {} is not ready", model),
            ));
        }

        // Build and execute inference request
        let triton_request = self.build_inference_request(&request);
        let request_id = triton_request.id.clone().unwrap_or_default();

        let triton_response = self.infer(&model, triton_request).await?;

        self.build_chat_response(&model, triton_response, &request_id)
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        // Triton streaming support depends on model deployment
        // For now, return not supported
        Err(TritonError::not_supported(
            PROVIDER_NAME,
            "Streaming is not yet implemented for Triton provider",
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error> {
        Err(TritonError::not_supported(
            PROVIDER_NAME,
            "Embeddings support depends on deployed model. Use infer() method directly.",
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        if self.is_healthy().await {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unhealthy
        }
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // Self-hosted Triton doesn't have per-request costs
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triton_provider_build_headers() {
        let config =
            TritonConfig::new("http://localhost:8000").header("Authorization", "Bearer test-token");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let headers = provider.build_headers();
        assert!(headers.iter().any(|(k, _)| k.as_ref() == "Content-Type"));
        assert!(
            headers
                .iter()
                .any(|(k, v)| k.as_ref() == "Authorization" && v.as_ref() == "Bearer test-token")
        );
    }

    #[test]
    fn test_triton_provider_get_model_url() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let url = provider.get_model_url("llama-7b", None);
        assert_eq!(url, "http://localhost:8000/v2/models/llama-7b");

        let url_with_version = provider.get_model_url("llama-7b", Some("1"));
        assert_eq!(
            url_with_version,
            "http://localhost:8000/v2/models/llama-7b/versions/1"
        );
    }

    #[test]
    fn test_triton_provider_build_inference_request() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let chat_request = ChatRequest {
            model: "llama-7b".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello, world!".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            ..Default::default()
        };

        let triton_request = provider.build_inference_request(&chat_request);

        assert!(triton_request.id.is_some());
        assert_eq!(triton_request.inputs.len(), 1);
        assert_eq!(triton_request.inputs[0].name, "text_input");
        assert_eq!(triton_request.inputs[0].datatype, "BYTES");

        let params = triton_request.parameters.unwrap();
        let temp = params.get("temperature").unwrap().as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
        assert_eq!(params.get("max_tokens").unwrap(), &serde_json::json!(100));
    }

    #[test]
    fn test_triton_provider_map_http_error() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let err = provider.map_http_error(400, "Bad request");
        assert!(matches!(err, TritonError::InvalidRequest { .. }));

        let err = provider.map_http_error(401, "Unauthorized");
        assert!(matches!(err, TritonError::Authentication { .. }));

        let err = provider.map_http_error(404, "Model not found");
        assert!(matches!(err, TritonError::ModelNotFound { .. }));

        let err = provider.map_http_error(429, "Rate limited");
        assert!(matches!(err, TritonError::RateLimit { .. }));

        let err = provider.map_http_error(500, "Internal error");
        assert!(matches!(err, TritonError::ProviderUnavailable { .. }));
    }

    #[test]
    fn test_triton_provider_name() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        assert_eq!(provider.name(), "triton");
    }

    #[test]
    fn test_triton_provider_capabilities() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
    }

    #[test]
    fn test_build_chat_response() {
        let config = TritonConfig::new("http://localhost:8000");

        let provider = TritonProvider {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new().unwrap()),
            models: Vec::new(),
        };

        let triton_response = TritonInferResponse {
            id: Some("resp-123".to_string()),
            model_name: "llama-7b".to_string(),
            model_version: Some("1".to_string()),
            outputs: vec![TritonTensor {
                name: "text_output".to_string(),
                datatype: "BYTES".to_string(),
                shape: vec![1],
                data: serde_json::json!(["Hello! How can I help you?"]),
                parameters: None,
            }],
            parameters: None,
        };

        let chat_response = provider
            .build_chat_response("llama-7b", triton_response, "req-123")
            .unwrap();

        assert_eq!(chat_response.id, "req-123");
        assert_eq!(chat_response.model, "llama-7b");
        assert_eq!(chat_response.choices.len(), 1);
        assert_eq!(
            chat_response.choices[0].message.role,
            MessageRole::Assistant
        );
    }
}
