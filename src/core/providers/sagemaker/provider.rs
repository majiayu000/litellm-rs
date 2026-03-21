//! AWS Sagemaker Provider Implementation
//!
//! Implements the LLMProvider trait for AWS Sagemaker endpoints.

use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

use super::config::SagemakerConfig;
use super::error::{SagemakerError, SagemakerErrorMapper};
use super::sigv4::SagemakerSigV4Signer;
use crate::core::providers::base::{GlobalPoolManager, HttpErrorMapper};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::ProviderConfig as _;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::health::HealthStatus;
use crate::core::types::responses::{ChatChunk, ChatResponse, EmbeddingResponse};
use crate::core::types::{chat::ChatRequest, embedding::EmbeddingRequest};
use crate::core::types::{context::RequestContext, model::ModelInfo, model::ProviderCapability};

/// Static capabilities for Sagemaker provider
const SAGEMAKER_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
];

/// AWS Sagemaker provider implementation
#[derive(Debug, Clone)]
pub struct SagemakerProvider {
    config: SagemakerConfig,
    _pool_manager: Arc<GlobalPoolManager>,
    signer: SagemakerSigV4Signer,
    models: Vec<ModelInfo>,
}

impl SagemakerProvider {
    /// Create a new Sagemaker provider instance
    pub async fn new(config: SagemakerConfig) -> Result<Self, SagemakerError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration("sagemaker", e))?;

        let pool_manager = Arc::new(GlobalPoolManager::new().map_err(|e| {
            ProviderError::configuration(
                "sagemaker",
                format!("Failed to create pool manager: {}", e),
            )
        })?);

        let signer = SagemakerSigV4Signer::new(
            config.get_access_key_id().unwrap_or_default(),
            config.get_secret_access_key().unwrap_or_default(),
            config.get_session_token(),
            config.get_region(),
        );

        // Sagemaker doesn't have a fixed model list - models are custom endpoints
        let models = vec![ModelInfo {
            id: "sagemaker-endpoint".to_string(),
            name: "Sagemaker Endpoint".to_string(),
            provider: "sagemaker".to_string(),
            max_context_length: 4096,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        }];

        Ok(Self {
            config,
            _pool_manager: pool_manager,
            signer,
            models,
        })
    }

    /// Create provider with AWS credentials
    pub async fn with_credentials(
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        region: impl Into<String>,
    ) -> Result<Self, SagemakerError> {
        let config = SagemakerConfig {
            aws_access_key_id: Some(access_key_id.into()),
            aws_secret_access_key: Some(secret_access_key.into()),
            aws_region: Some(region.into()),
            ..Default::default()
        };
        Self::new(config).await
    }
}

impl LLMProvider for SagemakerProvider {
    fn name(&self) -> &'static str {
        "sagemaker"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        SAGEMAKER_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        static PARAMS: &[&str] = &["stream", "max_tokens", "temperature", "top_p", "stop"];
        PARAMS
    }

    async fn map_openai_params(
        &self,
        mut params: HashMap<String, serde_json::Value>,
        _model: &str,
    ) -> Result<HashMap<String, serde_json::Value>, ProviderError> {
        // Map max_completion_tokens to max_tokens
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }

        // HuggingFace TGI requires temperature > 0
        if !self.config.allow_zero_temp
            && let Some(temp) = params.get("temperature")
            && temp.as_f64() == Some(0.0)
        {
            params.insert("temperature".to_string(), serde_json::json!(0.01));
        }

        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<serde_json::Value, ProviderError> {
        serde_json::to_value(&request)
            .map_err(|e| ProviderError::invalid_request("sagemaker", e.to_string()))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw_response).map_err(|e| {
            ProviderError::response_parsing("sagemaker", format!("Failed to parse response: {}", e))
        })
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(SagemakerErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Sagemaker chat request: model={}", request.model);

        // Extract endpoint name from model
        let endpoint_name = request
            .model
            .strip_prefix("sagemaker/")
            .unwrap_or(&request.model);
        let url = self.config.build_endpoint_url(endpoint_name, false);

        // Build request body for HuggingFace TGI format
        let body = serde_json::json!({
            "inputs": format_messages_for_tgi(&request),
            "parameters": {
                "max_new_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "top_p": request.top_p.unwrap_or(0.9),
                "do_sample": true,
            }
        });

        // Sign the request
        let body_str = serde_json::to_string(&body)
            .map_err(|e| ProviderError::invalid_request("sagemaker", e.to_string()))?;

        let headers = self
            .signer
            .sign_request(
                "POST",
                &url,
                &std::collections::HashMap::new(),
                &body_str,
                chrono::Utc::now(),
            )
            .map_err(|e| {
                ProviderError::authentication("sagemaker", format!("Signing error: {}", e))
            })?;

        // Execute request
        let client = reqwest::Client::new();
        let mut req_builder = client.post(&url);

        for (key, value) in headers {
            req_builder = req_builder.header(key, value);
        }

        let response = req_builder
            .body(body_str)
            .send()
            .await
            .map_err(|e| ProviderError::network("sagemaker", e.to_string()))?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network("sagemaker", e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            return Err(match status.as_u16() {
                400 => ProviderError::invalid_request("sagemaker", body_str.to_string()),
                401 | 403 => ProviderError::authentication("sagemaker", body_str.to_string()),
                404 | 424 => ProviderError::model_not_found("sagemaker", body_str.to_string()),
                429 => ProviderError::rate_limit("sagemaker", None),
                502 | 503 => {
                    ProviderError::api_error("sagemaker", status.as_u16(), body_str.to_string())
                }
                _ => HttpErrorMapper::map_status_code("sagemaker", status.as_u16(), &body_str),
            });
        }

        // Parse TGI response
        parse_tgi_response(&response_bytes, &request.model)
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        Err(ProviderError::not_supported(
            "sagemaker",
            "Streaming not yet implemented for Sagemaker".to_string(),
        ))
    }

    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "sagemaker",
            "Embeddings not supported by Sagemaker provider".to_string(),
        ))
    }

    async fn health_check(&self) -> HealthStatus {
        // Sagemaker health check would require an endpoint name
        // For now, just return healthy if config is valid
        if self.config.validate().is_ok() {
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
    ) -> Result<f64, ProviderError> {
        // Sagemaker pricing is based on instance hours, not tokens
        Ok(0.0)
    }
}

/// Format messages for HuggingFace TGI format
fn format_messages_for_tgi(request: &ChatRequest) -> String {
    let mut prompt = String::new();

    for message in &request.messages {
        let role = match message.role {
            crate::core::types::message::MessageRole::System => "System",
            crate::core::types::message::MessageRole::User => "User",
            crate::core::types::message::MessageRole::Assistant => "Assistant",
            _ => "User",
        };

        if let Some(content) = &message.content {
            let text = match content {
                crate::core::types::message::MessageContent::Text(t) => t.clone(),
                crate::core::types::message::MessageContent::Parts(parts) => parts
                    .iter()
                    .filter_map(|p| {
                        if let crate::core::types::content::ContentPart::Text { text } = p {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            prompt.push_str(&format!("{}: {}\n", role, text));
        }
    }

    prompt.push_str("Assistant:");
    prompt
}

/// Parse HuggingFace TGI response
fn parse_tgi_response(response_bytes: &[u8], model: &str) -> Result<ChatResponse, SagemakerError> {
    let json: serde_json::Value = serde_json::from_slice(response_bytes).map_err(|e| {
        ProviderError::response_parsing("sagemaker", format!("Failed to parse response: {}", e))
    })?;

    // TGI returns either a single object or an array
    let generated_text = if let Some(arr) = json.as_array() {
        arr.first()
            .and_then(|v| v.get("generated_text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    } else {
        json.get("generated_text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };

    Ok(ChatResponse {
        id: format!("sagemaker-{}", uuid::Uuid::new_v4().simple()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: format!("sagemaker/{}", model),
        choices: vec![crate::core::types::responses::ChatChoice {
            index: 0,
            message: crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::Assistant,
                content: Some(crate::core::types::message::MessageContent::Text(
                    generated_text.to_string(),
                )),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            },
            finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
            logprobs: None,
        }],
        usage: None,
        system_fingerprint: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_messages_for_tgi() {
        let request = ChatRequest {
            model: "test".to_string(),
            messages: vec![crate::core::types::chat::ChatMessage {
                role: crate::core::types::message::MessageRole::User,
                content: Some(crate::core::types::message::MessageContent::Text(
                    "Hello".to_string(),
                )),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            ..Default::default()
        };

        let prompt = format_messages_for_tgi(&request);
        assert!(prompt.contains("User: Hello"));
        assert!(prompt.ends_with("Assistant:"));
    }
}
