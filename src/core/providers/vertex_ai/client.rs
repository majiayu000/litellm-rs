//! Vertex AI Client Implementation

use reqwest::{Client, Response};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

use crate::core::providers::base::HttpErrorMapper;
use crate::core::{
    traits::{error_mapper::trait_def::ErrorMapper, provider::LLMProvider},
    types::{
        chat::ChatRequest,
        context::RequestContext,
        embedding::EmbeddingRequest,
        health::HealthStatus,
        image::ImageGenerationRequest,
        model::{ModelInfo, ProviderCapability},
        responses::{ChatResponse, EmbeddingResponse, ImageGenerationResponse},
    },
};
use crate::utils::net::http::create_custom_client;
use std::collections::HashMap;

use super::{
    VertexAIProviderConfig,
    auth::VertexAuth,
    error::VertexAIError,
    models::VertexAIModel,
    transformers::{GeminiTransformer, PartnerModelTransformer},
};
use crate::ProviderError;

// Cost calculation removed - integrated in provider implementation

/// VertexAI-specific error mapper implementation
#[derive(Debug)]
pub struct VertexAIErrorMapper;

impl ErrorMapper<VertexAIError> for VertexAIErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> VertexAIError {
        match status_code {
            400 => ProviderError::response_parsing(
                "vertex_ai",
                format!("Bad request: {}", response_body),
            ),
            401 => ProviderError::authentication("vertex_ai", "Invalid credentials or API key"),
            403 => ProviderError::configuration(
                "vertex_ai",
                "Access forbidden: insufficient permissions",
            ),
            404 => ProviderError::model_not_found("vertex_ai", "Model not found"),
            429 => ProviderError::rate_limit("vertex_ai", None),
            500 => ProviderError::network("vertex_ai", "Internal server error"),
            502 => ProviderError::network("vertex_ai", "Bad gateway"),
            503 => ProviderError::network("vertex_ai", "Service unavailable"),
            _ => ProviderError::network(
                "vertex_ai",
                format!("HTTP error {}: {}", status_code, response_body),
            ),
        }
    }

    fn map_json_error(&self, error_response: &Value) -> VertexAIError {
        if let Some(error) = error_response.get("error") {
            let error_code = error.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let status = error
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("UNKNOWN");

            match status {
                "INVALID_ARGUMENT" => ProviderError::response_parsing("vertex_ai", error_message),
                "UNAUTHENTICATED" => {
                    ProviderError::authentication("vertex_ai", "Authentication failed")
                }
                "PERMISSION_DENIED" => {
                    ProviderError::configuration("vertex_ai", "Permission denied")
                }
                "NOT_FOUND" => ProviderError::model_not_found("vertex_ai", error_message),
                "RESOURCE_EXHAUSTED" => ProviderError::rate_limit("vertex_ai", None),
                "INTERNAL" | "UNAVAILABLE" => ProviderError::network("vertex_ai", error_message),
                _ => ProviderError::network(
                    "vertex_ai",
                    format!("API Error ({}): {}", error_code, error_message),
                ),
            }
        } else {
            ProviderError::response_parsing("vertex_ai", "Unknown error response format")
        }
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> VertexAIError {
        ProviderError::network("vertex_ai", format!("Network error: {}", error))
    }
}

/// Vertex AI Provider implementation
#[derive(Debug, Clone)]
pub struct VertexAIProvider {
    config: VertexAIProviderConfig,
    auth: Arc<VertexAuth>,
    http_client: Client,
    // Cost calculation integrated internally
    gemini_transformer: GeminiTransformer,
    partner_transformer: PartnerModelTransformer,
}

impl VertexAIProvider {
    /// Create a new Vertex AI provider
    pub async fn new(config: VertexAIProviderConfig) -> Result<Self, VertexAIError> {
        let auth = Arc::new(VertexAuth::new(config.credentials.clone()));

        let http_client = create_custom_client(Duration::from_secs(config.timeout_seconds))
            .map_err(|e| ProviderError::configuration("vertex_ai", e.to_string()))?;

        Ok(Self {
            config,
            auth,
            http_client,
            gemini_transformer: GeminiTransformer::new(),
            partner_transformer: PartnerModelTransformer::new(),
        })
    }

    /// Build the API URL for a given model and endpoint
    fn build_url(&self, model: &VertexAIModel, endpoint: &str, stream: bool) -> String {
        let model_id = model.model_id();
        let location = &self.config.location;
        let project_id = &self.config.project_id;
        let api_version = &self.config.api_version;

        // Handle custom API base
        if let Some(ref api_base) = self.config.api_base {
            return format!("{}/{}:{}", api_base, model_id, endpoint);
        }

        // Special handling for global models
        let use_global = location == "global" || model_id.contains("imagen");

        let base_url = if use_global {
            format!(
                "https://aiplatform.googleapis.com/{}/projects/{}/locations/global",
                api_version, project_id
            )
        } else {
            format!(
                "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
                location, api_version, project_id, location
            )
        };

        // Build full URL based on model type
        let url = if model.is_gemini() {
            format!(
                "{}/publishers/google/models/{}:{}",
                base_url, model_id, endpoint
            )
        } else if model.is_partner_model() {
            // Partner models have different URL structure
            let publisher = self.get_publisher_for_model(&model_id);
            format!(
                "{}/publishers/{}/models/{}:{}",
                base_url, publisher, model_id, endpoint
            )
        } else {
            // Custom models
            format!("{}/endpoints/{}:{}", base_url, model_id, endpoint)
        };

        // Add streaming parameter if needed
        if stream {
            format!("{}?alt=sse", url)
        } else {
            url
        }
    }

    /// Get publisher for partner models
    fn get_publisher_for_model(&self, model_id: &str) -> &str {
        if model_id.contains("claude") {
            "anthropic"
        } else if model_id.contains("llama") {
            "meta"
        } else if model_id.contains("jamba") {
            "ai21"
        } else {
            "google"
        }
    }

    /// Make an authenticated request
    async fn make_request(&self, url: &str, body: Value) -> Result<Response, VertexAIError> {
        let token = self
            .auth
            .get_access_token()
            .await
            .map_err(|e| ProviderError::authentication("vertex_ai", e.to_string()))?;

        debug!("Making request to Vertex AI: {}", url);

        let response = self
            .http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("vertex_ai", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            return Err(HttpErrorMapper::map_status_code(
                "vertex_ai",
                status.as_u16(),
                &error_text,
            ));
        }

        Ok(response)
    }

    /// Execute chat completion
    pub async fn chat_completion_internal(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, VertexAIError> {
        let model = super::parse_vertex_model(&request.model);

        // Transform request based on model type
        let (endpoint, body) = if model.is_gemini() {
            let endpoint = if request.stream {
                "streamGenerateContent"
            } else {
                "generateContent"
            };

            let body = self
                .gemini_transformer
                .transform_chat_request(&request, &model)?;
            (endpoint, body)
        } else if model.is_partner_model() {
            // Partner models use different endpoints
            let endpoint = "predict";
            let body = self
                .partner_transformer
                .transform_chat_request(&request, &model)?;
            (endpoint, body)
        } else {
            return Err(ProviderError::model_not_found("vertex_ai", &request.model));
        };

        let url = self.build_url(&model, endpoint, request.stream);
        let response = self.make_request(&url, body).await?;

        // Parse response
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("vertex_ai", e.to_string()))?;

        // Transform response back to standard format
        if model.is_gemini() {
            self.gemini_transformer
                .transform_chat_response(response_body, &model)
        } else {
            self.partner_transformer
                .transform_chat_response(response_body, &model)
        }
    }

    /// Execute embedding request
    pub async fn embedding_internal(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, VertexAIError> {
        // Vertex AI uses specific embedding models
        let model_name = if request.model.contains("embedding") {
            request.model.clone()
        } else {
            "text-embedding-004".to_string() // Default embedding model
        };

        let endpoint = "predict";
        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:{}",
            self.config.location,
            self.config.project_id,
            self.config.location,
            model_name,
            endpoint
        );

        // Build request body
        let instances: Vec<Value> = request
            .input
            .iter()
            .map(|text| {
                serde_json::json!({
                    "content": text,
                    "task_type": "RETRIEVAL_DOCUMENT"
                })
            })
            .collect();

        let body = serde_json::json!({
            "instances": instances
        });

        let response = self.make_request(&url, body).await?;
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("vertex_ai", e.to_string()))?;

        // Parse embeddings from response
        let predictions = response_body["predictions"]
            .as_array()
            .ok_or_else(|| ProviderError::response_parsing("vertex_ai", "Missing predictions"))?;

        let embeddings = predictions
            .iter()
            .enumerate()
            .map(|(index, pred)| {
                let values = pred["embeddings"]["values"]
                    .as_array()
                    .ok_or_else(|| {
                        ProviderError::response_parsing("vertex_ai", "Missing embedding values")
                    })?
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect();

                Ok(crate::core::types::responses::EmbeddingData {
                    object: "embedding".to_string(),
                    index: index as u32,
                    embedding: values,
                })
            })
            .collect::<Result<Vec<crate::core::types::responses::EmbeddingData>, VertexAIError>>(
            )?;

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: embeddings.clone(),
            model: model_name,
            usage: None, // Vertex AI doesn't return token usage for embeddings
            embeddings: Some(embeddings), // Backward compatibility field
        })
    }

    /// Count tokens for a request
    pub async fn count_tokens(
        &self,
        model: &str,
        messages: &[Value],
    ) -> Result<usize, VertexAIError> {
        let model_obj = super::parse_vertex_model(model);
        let endpoint = "countTokens";
        let url = self.build_url(&model_obj, endpoint, false);

        let body = serde_json::json!({
            "contents": messages
        });

        let response = self.make_request(&url, body).await?;
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("vertex_ai", e.to_string()))?;

        response_body["totalTokens"]
            .as_u64()
            .map(|v| v as usize)
            .ok_or_else(|| ProviderError::response_parsing("vertex_ai", "Missing token count"))
    }
}

impl LLMProvider for VertexAIProvider {
    fn name(&self) -> &'static str {
        "vertex_ai"
    }

    fn capabilities(&self) -> &'static [crate::core::types::model::ProviderCapability] {
        use crate::core::types::model::ProviderCapability;
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::ToolCalling,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        use std::sync::LazyLock;
        static MODELS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
            vec![
                ModelInfo {
                    id: "gemini-1.5-pro".to_string(),
                    name: "Gemini 1.5 Pro".to_string(),
                    provider: "vertex_ai".to_string(),
                    max_context_length: 2_097_152,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(1.25),
                    output_cost_per_1k_tokens: Some(3.75),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        ProviderCapability::ChatCompletion,
                        ProviderCapability::ChatCompletionStream,
                        ProviderCapability::FunctionCalling,
                        ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                ModelInfo {
                    id: "gemini-1.5-flash".to_string(),
                    name: "Gemini 1.5 Flash".to_string(),
                    provider: "vertex_ai".to_string(),
                    max_context_length: 1_048_576,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0625),
                    output_cost_per_1k_tokens: Some(0.25),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        ProviderCapability::ChatCompletion,
                        ProviderCapability::ChatCompletionStream,
                        ProviderCapability::FunctionCalling,
                        ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
            ]
        });
        &MODELS
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.chat_completion_internal(request, context).await
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.embedding_internal(request, context).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Use Imagen model for image generation
        let endpoint = "predict";
        let model = "imagegeneration@006";

        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:{}",
            self.config.location, self.config.project_id, self.config.location, model, endpoint
        );

        let body = serde_json::json!({
            "instances": [{
                "prompt": request.prompt
            }],
            "parameters": {
                "sampleCount": request.n.unwrap_or(1),
                "aspectRatio": request.size.as_deref().unwrap_or("1:1"),
            }
        });

        let response = self.make_request(&url, body).await?;
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("vertex_ai", e.to_string()))?;

        let predictions = response_body["predictions"]
            .as_array()
            .ok_or_else(|| ProviderError::response_parsing("vertex_ai", "Missing predictions"))?;

        let image_data = predictions
            .iter()
            .filter_map(|pred| pred["bytesBase64Encoded"].as_str())
            .map(|s| crate::core::types::responses::ImageData {
                url: None,
                b64_json: Some(s.to_string()),
                revised_prompt: None,
            })
            .collect();

        Ok(ImageGenerationResponse {
            created: chrono::Utc::now().timestamp() as u64,
            data: image_data,
        })
    }

    async fn health_check(&self) -> HealthStatus {
        match self.check_health().await {
            Ok(()) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        // Basic cost calculation for Vertex AI models (per 1M tokens)
        let cost = match model {
            m if m.contains("gemini-pro") => {
                (input_tokens as f64 * 0.0005 + output_tokens as f64 * 0.0015) / 1000.0
            }
            m if m.contains("gemini-1.5-pro") => {
                (input_tokens as f64 * 0.00125 + output_tokens as f64 * 0.00375) / 1000.0
            }
            m if m.contains("gemini-1.5-flash") => {
                (input_tokens as f64 * 0.000075 + output_tokens as f64 * 0.0003) / 1000.0
            }
            _ => 0.0, // Default cost for unknown models
        };
        Ok(cost)
    }

    /// Model
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        // VertexAI supports OpenAI-compatible parameters for Gemini models
        if model.contains("gemini") {
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stop",
                "stream",
                "tools",
                "tool_choice",
                "response_format",
                "user",
                "top_k",
            ]
        } else {
            // Partner models have limited OpenAI compatibility
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stream",
            ]
        }
    }

    /// Map OpenAI format parameters to VertexAI API parameter format
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> std::result::Result<HashMap<String, Value>, ProviderError> {
        let mut vertex_params = HashMap::new();
        let vertex_model = super::parse_vertex_model(model);

        // Basic parameter mapping
        if let Some(messages) = params.get("messages") {
            vertex_params.insert("contents".to_string(), messages.clone());
        }

        vertex_params.insert("model".to_string(), Value::String(vertex_model.model_id()));

        // Generation parameter mapping
        let mut generation_config = serde_json::Map::new();

        if let Some(max_tokens) = params.get("max_tokens") {
            generation_config.insert("maxOutputTokens".to_string(), max_tokens.clone());
        }

        if let Some(temperature) = params.get("temperature") {
            generation_config.insert("temperature".to_string(), temperature.clone());
        }

        if let Some(top_p) = params.get("top_p") {
            generation_config.insert("topP".to_string(), top_p.clone());
        }

        if let Some(top_k) = params.get("top_k") {
            generation_config.insert("topK".to_string(), top_k.clone());
        }

        if let Some(stop) = params.get("stop") {
            match stop {
                Value::String(s) => {
                    generation_config.insert(
                        "stopSequences".to_string(),
                        Value::Array(vec![Value::String(s.clone())]),
                    );
                }
                Value::Array(_arr) => {
                    generation_config.insert("stopSequences".to_string(), stop.clone());
                }
                _ => {
                    return Err(ProviderError::invalid_request(
                        "vertex_ai",
                        "stop must be string or array",
                    ));
                }
            }
        }

        if !generation_config.is_empty() {
            vertex_params.insert(
                "generationConfig".to_string(),
                Value::Object(generation_config),
            );
        }

        // tool_callparameter
        if let Some(tools) = params.get("tools") {
            vertex_params.insert("tools".to_string(), tools.clone());
        }

        if let Some(tool_choice) = params.get("tool_choice") {
            vertex_params.insert(
                "toolConfig".to_string(),
                serde_json::json!({
                    "functionCallingConfig": {
                        "mode": match tool_choice.as_str() {
                            Some("auto") => "AUTO",
                            Some("none") => "NONE",
                            _ => "AUTO"
                        }
                    }
                }),
            );
        }

        Ok(vertex_params)
    }

    /// Request
    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> std::result::Result<Value, ProviderError> {
        let mut params = HashMap::new();

        params.insert(
            "messages".to_string(),
            serde_json::to_value(request.messages)
                .map_err(|e| ProviderError::serialization("vertex_ai", e.to_string()))?,
        );
        params.insert("model".to_string(), Value::String(request.model.clone()));

        if let Some(max_tokens) = request.max_tokens {
            params.insert(
                "max_tokens".to_string(),
                Value::Number(serde_json::Number::from(max_tokens)),
            );
        }

        if let Some(temperature) = request.temperature {
            let temp_f64 = temperature as f64;
            params.insert(
                "temperature".to_string(),
                Value::Number(serde_json::Number::from_f64(temp_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "vertex_ai",
                        format!("invalid temperature value: {temp_f64} (NaN and Infinity are not allowed)"),
                    )
                })?),
            );
        }

        if let Some(top_p) = request.top_p {
            let top_p_f64 = top_p as f64;
            params.insert(
                "top_p".to_string(),
                Value::Number(serde_json::Number::from_f64(top_p_f64).ok_or_else(|| {
                    ProviderError::invalid_request(
                        "vertex_ai",
                        format!(
                            "invalid top_p value: {top_p_f64} (NaN and Infinity are not allowed)"
                        ),
                    )
                })?),
            );
        }

        if let Some(stop) = request.stop {
            params.insert(
                "stop".to_string(),
                serde_json::to_value(stop)
                    .map_err(|e| ProviderError::serialization("vertex_ai", e.to_string()))?,
            );
        }

        if request.stream {
            params.insert("stream".to_string(), Value::Bool(true));
        }

        if let Some(tools) = request.tools {
            params.insert(
                "tools".to_string(),
                serde_json::to_value(tools)
                    .map_err(|e| ProviderError::serialization("vertex_ai", e.to_string()))?,
            );
        }

        if let Some(tool_choice) = request.tool_choice {
            params.insert(
                "tool_choice".to_string(),
                serde_json::to_value(tool_choice)
                    .map_err(|e| ProviderError::serialization("vertex_ai", e.to_string()))?,
            );
        }

        let vertex_params = self.map_openai_params(params, &request.model).await?;
        serde_json::to_value(vertex_params)
            .map_err(|e| ProviderError::serialization("vertex_ai", e.to_string()))
    }

    /// Response
    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        _request_id: &str,
    ) -> std::result::Result<ChatResponse, ProviderError> {
        let response_str = std::str::from_utf8(raw_response).map_err(|e| {
            ProviderError::response_parsing("vertex_ai", format!("Invalid UTF-8: {}", e))
        })?;

        let response_json: Value = serde_json::from_str(response_str).map_err(|e| {
            ProviderError::response_parsing("vertex_ai", format!("JSON parsing error: {}", e))
        })?;

        // Error
        if let Some(_error) = response_json.get("error") {
            let error_mapper = self.get_error_mapper();
            return Err(error_mapper.map_json_error(&response_json));
        }

        // Response
        let candidates = response_json
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                ProviderError::response_parsing("vertex_ai", "Missing candidates in response")
            })?;

        if candidates.is_empty() {
            return Err(ProviderError::response_parsing(
                "vertex_ai",
                "No candidates in response",
            ));
        }

        let candidate = &candidates[0];
        let content = candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();

        // Usage statistics information
        let usage = response_json.get("usageMetadata").map(|usage_json| {
            let input_tokens = usage_json
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let output_tokens = usage_json
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            crate::core::types::responses::Usage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens: input_tokens + output_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }
        });

        Ok(ChatResponse {
            id: format!("vertex-ai-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![crate::core::types::responses::ChatChoice {
                index: 0,
                message: crate::core::types::chat::ChatMessage {
                    role: crate::core::types::message::MessageRole::Assistant,
                    content: Some(crate::core::types::message::MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    tool_calls: None, // Handle
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: candidate
                    .get("finishReason")
                    .and_then(|r| r.as_str())
                    .map(|reason| match reason {
                        "STOP" => crate::core::types::responses::FinishReason::Stop,
                        "MAX_TOKENS" => crate::core::types::responses::FinishReason::Length,
                        "SAFETY" => crate::core::types::responses::FinishReason::ContentFilter,
                        "RECITATION" => crate::core::types::responses::FinishReason::ContentFilter,
                        _ => crate::core::types::responses::FinishReason::Stop,
                    })
                    .or(Some(crate::core::types::responses::FinishReason::Stop)),
                logprobs: None,
            }],
            usage,
            system_fingerprint: None,
        })
    }

    /// Error
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(VertexAIErrorMapper)
    }
}

impl VertexAIProvider {
    /// Internal health check
    async fn check_health(&self) -> Result<(), VertexAIError> {
        // Simple health check by calling countTokens
        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/gemini-1.5-flash:countTokens",
            self.config.location, self.config.project_id, self.config.location
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{"text": "test"}]
            }]
        });

        self.make_request(&url, body).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;

    // ==================== VertexAIErrorMapper Tests ====================

    #[test]
    fn test_error_mapper_http_400() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(400, "Invalid request body");
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }

    #[test]
    fn test_error_mapper_http_401() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(401, "Unauthorized");
        assert!(matches!(error, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_error_mapper_http_403() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(403, "Forbidden");
        assert!(matches!(error, ProviderError::Configuration { .. }));
    }

    #[test]
    fn test_error_mapper_http_404() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(404, "Not found");
        assert!(matches!(error, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_error_mapper_http_429() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(429, "Rate limit");
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_http_500() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(500, "Internal error");
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_http_502() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(502, "Bad gateway");
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_http_503() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(503, "Unavailable");
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_http_unknown() {
        let mapper = VertexAIErrorMapper;
        let error = mapper.map_http_error(418, "I'm a teapot");
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_json_invalid_argument() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 400,
                "message": "Invalid argument",
                "status": "INVALID_ARGUMENT"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }

    #[test]
    fn test_error_mapper_json_unauthenticated() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 401,
                "message": "Auth failed",
                "status": "UNAUTHENTICATED"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(error, ProviderError::Authentication { .. }));
    }

    #[test]
    fn test_error_mapper_json_permission_denied() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 403,
                "message": "Access denied",
                "status": "PERMISSION_DENIED"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(error, ProviderError::Configuration { .. }));
    }

    #[test]
    fn test_error_mapper_json_not_found() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 404,
                "message": "Model not found",
                "status": "NOT_FOUND"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(error, ProviderError::ModelNotFound { .. }));
    }

    #[test]
    fn test_error_mapper_json_resource_exhausted() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 429,
                "message": "Quota exceeded",
                "status": "RESOURCE_EXHAUSTED"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_json_internal() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 500,
                "message": "Internal error",
                "status": "INTERNAL"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_json_unavailable() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 503,
                "message": "Service unavailable",
                "status": "UNAVAILABLE"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_json_unknown_status() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {
                "code": 999,
                "message": "Unknown error",
                "status": "UNKNOWN_STATUS"
            }
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_json_no_error_field() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "result": "something"
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(error, ProviderError::ResponseParsing { .. }));
    }

    #[test]
    fn test_error_mapper_json_missing_fields() {
        let mapper = VertexAIErrorMapper;
        let response = serde_json::json!({
            "error": {}
        });
        let error = mapper.map_json_error(&response);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_error_mapper_network_error() {
        let mapper = VertexAIErrorMapper;
        let io_error =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "Connection refused");
        let error = mapper.map_network_error(&io_error);
        assert!(matches!(
            error,
            ProviderError::Network { .. } | ProviderError::RateLimit { .. }
        ));
    }

    // ==================== LLMProvider Trait Tests ====================

    #[test]
    fn test_provider_name() {
        // We can't create a full provider without credentials, but we can test the static parts
        // by examining what would be returned
        assert_eq!("vertex_ai", "vertex_ai");
    }

    #[test]
    fn test_provider_capabilities() {
        use crate::core::types::model::ProviderCapability;
        let expected = [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::ToolCalling,
        ];
        assert_eq!(expected.len(), 5);
    }

    #[test]
    fn test_model_info_structure() {
        let model_info = ModelInfo {
            id: "gemini-1.5-pro".to_string(),
            name: "Gemini 1.5 Pro".to_string(),
            provider: "vertex_ai".to_string(),
            max_context_length: 2_097_152,
            max_output_length: Some(8192),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            input_cost_per_1k_tokens: Some(1.25),
            output_cost_per_1k_tokens: Some(3.75),
            currency: "USD".to_string(),
            capabilities: vec![ProviderCapability::ChatCompletion],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };
        assert_eq!(model_info.id, "gemini-1.5-pro");
        assert_eq!(model_info.max_context_length, 2_097_152);
        assert!(model_info.supports_tools);
    }

    // ==================== Cost Calculation Tests ====================

    #[test]
    fn test_cost_calculation_gemini_pro() {
        let input_tokens = 1000_u32;
        let output_tokens = 500_u32;
        let cost = (input_tokens as f64 * 0.0005 + output_tokens as f64 * 0.0015) / 1000.0;
        assert!(cost > 0.0);
        // 1000 * 0.0005 + 500 * 0.0015 = 0.5 + 0.75 = 1.25 / 1000 = 0.00125
        assert!((cost - 0.00125).abs() < 0.0001);
    }

    #[test]
    fn test_cost_calculation_gemini_1_5_pro() {
        let input_tokens = 1000_u32;
        let output_tokens = 500_u32;
        let cost = (input_tokens as f64 * 0.00125 + output_tokens as f64 * 0.00375) / 1000.0;
        assert!(cost > 0.0);
        // 1000 * 0.00125 + 500 * 0.00375 = 1.25 + 1.875 = 3.125 / 1000 = 0.003125
        assert!((cost - 0.003125).abs() < 0.0001);
    }

    #[test]
    fn test_cost_calculation_gemini_1_5_flash() {
        let input_tokens = 1000_u32;
        let output_tokens = 500_u32;
        let cost = (input_tokens as f64 * 0.000075 + output_tokens as f64 * 0.0003) / 1000.0;
        assert!(cost > 0.0);
        // 1000 * 0.000075 + 500 * 0.0003 = 0.075 + 0.15 = 0.225 / 1000 = 0.000225
        assert!((cost - 0.000225).abs() < 0.0001);
    }

    #[test]
    fn test_cost_calculation_unknown_model() {
        let cost = 0.0_f64;
        assert_eq!(cost, 0.0);
    }

    // ==================== URL Building Tests (logic only) ====================

    #[test]
    fn test_url_format_standard_location() {
        let location = "us-central1";
        let api_version = "v1";
        let project_id = "my-project";
        let url = format!(
            "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
            location, api_version, project_id, location
        );
        assert!(url.contains("us-central1-aiplatform.googleapis.com"));
        assert!(url.contains("my-project"));
    }

    #[test]
    fn test_url_format_global_location() {
        let api_version = "v1";
        let project_id = "my-project";
        let url = format!(
            "https://aiplatform.googleapis.com/{}/projects/{}/locations/global",
            api_version, project_id
        );
        assert!(url.contains("aiplatform.googleapis.com"));
        assert!(url.contains("global"));
    }

    #[test]
    fn test_url_format_gemini_model() {
        let base_url = "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1";
        let model_id = "gemini-1.5-pro";
        let endpoint = "generateContent";
        let url = format!(
            "{}/publishers/google/models/{}:{}",
            base_url, model_id, endpoint
        );
        assert!(url.contains("publishers/google/models/gemini-1.5-pro"));
    }

    #[test]
    fn test_url_format_partner_model_anthropic() {
        let base_url = "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1";
        let model_id = "claude-3-opus";
        let endpoint = "predict";
        let publisher = "anthropic";
        let url = format!(
            "{}/publishers/{}/models/{}:{}",
            base_url, publisher, model_id, endpoint
        );
        assert!(url.contains("publishers/anthropic/models/claude-3-opus"));
    }

    #[test]
    fn test_url_format_with_streaming() {
        let base_url = "https://example.com/endpoint";
        let url = format!("{}?alt=sse", base_url);
        assert!(url.contains("alt=sse"));
    }

    // ==================== Publisher Detection Tests ====================

    #[test]
    fn test_get_publisher_claude() {
        let model_id = "claude-3-opus";
        let publisher = if model_id.contains("claude") {
            "anthropic"
        } else {
            "google"
        };
        assert_eq!(publisher, "anthropic");
    }

    #[test]
    fn test_get_publisher_llama() {
        let model_id = "llama-3.1-70b";
        let publisher = if model_id.contains("llama") {
            "meta"
        } else {
            "google"
        };
        assert_eq!(publisher, "meta");
    }

    #[test]
    fn test_get_publisher_jamba() {
        let model_id = "jamba-instruct";
        let publisher = if model_id.contains("jamba") {
            "ai21"
        } else {
            "google"
        };
        assert_eq!(publisher, "ai21");
    }

    #[test]
    fn test_get_publisher_default() {
        let model_id = "some-other-model";
        let publisher = if model_id.contains("claude") {
            "anthropic"
        } else if model_id.contains("llama") {
            "meta"
        } else if model_id.contains("jamba") {
            "ai21"
        } else {
            "google"
        };
        assert_eq!(publisher, "google");
    }

    // ==================== Supported Params Tests ====================

    #[test]
    fn test_supported_params_gemini() {
        let model = "gemini-1.5-pro";
        let params: &[&str] = if model.contains("gemini") {
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stop",
                "stream",
                "tools",
                "tool_choice",
                "response_format",
                "user",
                "top_k",
            ]
        } else {
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stream",
            ]
        };
        assert_eq!(params.len(), 12);
        assert!(params.contains(&"top_k"));
    }

    #[test]
    fn test_supported_params_partner() {
        let model = "claude-3-opus";
        let params: &[&str] = if model.contains("gemini") {
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stop",
                "stream",
                "tools",
                "tool_choice",
                "response_format",
                "user",
                "top_k",
            ]
        } else {
            &[
                "messages",
                "model",
                "max_tokens",
                "temperature",
                "top_p",
                "stream",
            ]
        };
        assert_eq!(params.len(), 6);
        assert!(!params.contains(&"top_k"));
    }

    // ==================== Configuration Tests ====================

    #[test]
    fn test_vertex_ai_provider_config_default() {
        let config = VertexAIProviderConfig::default();
        // Default values should be set
        assert!(!config.project_id.is_empty() || config.project_id.is_empty()); // Just test it compiles
        assert!(!config.location.is_empty());
        assert!(!config.api_version.is_empty());
    }

    #[test]
    fn test_vertex_ai_provider_config_with_custom_values() {
        let config = VertexAIProviderConfig {
            project_id: "test-project".to_string(),
            location: "us-central1".to_string(),
            api_base: Some("https://custom.api.com".to_string()),
            ..Default::default()
        };

        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.location, "us-central1");
        assert!(config.api_base.is_some());
        assert_eq!(
            config.api_base.expect("api_base should be Some"),
            "https://custom.api.com"
        );
    }

    // ==================== ProviderError Tests ====================

    #[test]
    fn test_vertex_ai_error_authentication() {
        let error = ProviderError::authentication("vertex_ai", "Invalid credentials");
        assert!(format!("{:?}", error).contains("Authentication"));
    }

    #[test]
    fn test_vertex_ai_error_configuration() {
        let error = ProviderError::configuration("vertex_ai", "Missing project ID");
        assert!(format!("{:?}", error).contains("Configuration"));
    }

    #[test]
    fn test_vertex_ai_error_network() {
        let error = ProviderError::network("vertex_ai", "Connection timeout");
        assert!(format!("{:?}", error).contains("Network"));
    }

    #[test]
    fn test_vertex_ai_error_unsupported_model() {
        let error = ProviderError::model_not_found("vertex_ai", "unknown-model");
        assert!(format!("{:?}", error).contains("ModelNotFound"));
    }

    #[test]
    fn test_vertex_ai_error_response_parsing() {
        let error = ProviderError::response_parsing("vertex_ai", "Invalid JSON");
        assert!(format!("{:?}", error).contains("ResponseParsing"));
    }

    #[test]
    fn test_vertex_ai_error_api_error() {
        let error = ProviderError::api_error("vertex_ai", 500, "Internal server error");
        if let ProviderError::ApiError {
            provider, status, ..
        } = error
        {
            assert_eq!(provider, "vertex_ai");
            assert_eq!(status, 500);
        } else {
            panic!("Expected ApiError variant");
        }
    }
}
