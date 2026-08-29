//! Vertex AI Client Implementation

use reqwest::Response;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
#[cfg(test)]
use crate::core::providers::shared::strict_vertex_usage_metadata;
use crate::core::{
    traits::{
        error_mapper::trait_def::ErrorMapper,
        provider::{LLMProvider, ProviderConfig},
    },
    types::{
        chat::ChatRequest,
        context::RequestContext,
        embedding::EmbeddingRequest,
        health::HealthStatus,
        image::ImageGenerationRequest,
        model::ModelInfo,
        responses::{ChatResponse, EmbeddingResponse, ImageGenerationResponse},
    },
};
use std::collections::HashMap;

use super::{
    VertexAIProviderConfig,
    auth::VertexAuth,
    error::VertexAIError,
    transformers::{GeminiTransformer, PartnerModelTransformer},
};
use crate::ProviderError;

mod error_mapper;
mod health;
mod url;

pub use self::error_mapper::VertexAIErrorMapper;

// Cost calculation removed - integrated in provider implementation

/// Vertex AI Provider implementation
#[derive(Debug, Clone)]
pub struct VertexAIProvider {
    config: Box<VertexAIProviderConfig>,
    auth: Arc<VertexAuth>,
    http_client: BaseHttpClient,
    // Cost calculation integrated internally
    gemini_transformer: GeminiTransformer,
    partner_transformer: PartnerModelTransformer,
    model_listings: crate::core::providers::gemini::models::GeminiModelListings,
    clock: crate::core::providers::gemini::models::GeminiUtcClock,
}

impl VertexAIProvider {
    /// Create a new Vertex AI provider
    pub async fn new(config: VertexAIProviderConfig) -> Result<Self, VertexAIError> {
        Self::new_with_clock(
            config,
            crate::core::providers::gemini::models::GeminiUtcClock::system(),
        )
        .await
    }

    pub(crate) async fn new_with_clock(
        config: VertexAIProviderConfig,
        clock: crate::core::providers::gemini::models::GeminiUtcClock,
    ) -> Result<Self, VertexAIError> {
        config
            .validate()
            .map_err(|error| ProviderError::configuration("vertex_ai", error))?;
        let auth = Arc::new(VertexAuth::new(config.credentials.clone()));
        let http_client = BaseHttpClient::new_for_provider(
            "vertex_ai",
            BaseConfig {
                api_base: config.api_base.clone(),
                endpoint_access: config.endpoint_access,
                timeout: config.timeout_seconds,
                ..Default::default()
            },
        )?;

        let surface = if config.enable_experimental {
            crate::core::providers::gemini::GoogleGeminiApiSurface::VertexAiExperimental
        } else {
            crate::core::providers::gemini::GoogleGeminiApiSurface::VertexAi
        };
        let model_listings = crate::core::providers::gemini::models::GeminiModelListings::new(
            crate::core::providers::gemini::get_gemini_registry(),
            surface,
        );

        Ok(Self {
            config: Box::new(config),
            auth,
            http_client,
            gemini_transformer: GeminiTransformer::new(),
            partner_transformer: PartnerModelTransformer::new(),
            model_listings,
            clock,
        })
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
            .post(url)?
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("vertex_ai", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "failed to read upstream error body".to_string());

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
        let is_catalog_gemini =
            super::is_vertex_gemini_catalog_model(&request.model, self.config.enable_experimental);

        // Transform request based on model type
        let (endpoint, body) = if is_catalog_gemini {
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

        let url = if is_catalog_gemini {
            self.build_google_catalog_model_url(&request.model, endpoint, request.stream)
        } else {
            self.build_url(&model, endpoint, request.stream)
        };
        let response = self.make_request(&url, body).await?;

        // Parse response
        let response_body: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("vertex_ai", e.to_string()))?;

        // Transform response back to standard format
        if is_catalog_gemini {
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
        let url = self.build_google_model_url(&model_name, endpoint);

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
        self.model_listings.at(self.clock.now())
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

        let url = self.build_google_model_url(model, endpoint);

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
        super::calculate_vertex_cost(model, input_tokens, output_tokens)
    }

    /// Model
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        if crate::core::providers::gemini::models::uses_fixed_sampling_contract(model) {
            return &[
                "messages",
                "model",
                "max_tokens",
                "stop",
                "stream",
                "tools",
                "tool_choice",
                "response_format",
                "user",
            ];
        }
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

        let fixed_sampling =
            crate::core::providers::gemini::models::uses_fixed_sampling_contract(model);
        if !fixed_sampling && let Some(temperature) = params.get("temperature") {
            generation_config.insert("temperature".to_string(), temperature.clone());
        }

        if !fixed_sampling && let Some(top_p) = params.get("top_p") {
            generation_config.insert("topP".to_string(), top_p.clone());
        }

        if !fixed_sampling && let Some(top_k) = params.get("top_k") {
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
        let model = super::parse_vertex_model(&request.model);
        if super::is_vertex_gemini_catalog_model(&request.model, self.config.enable_experimental) {
            self.gemini_transformer
                .transform_chat_request(&request, &model)
        } else if model.is_partner_model() {
            self.partner_transformer
                .transform_chat_request(&request, &model)
        } else {
            Err(ProviderError::model_not_found("vertex_ai", &request.model))
        }
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

        let raw_model = model;
        let model = super::parse_vertex_model(raw_model);
        if super::is_vertex_gemini_catalog_model(raw_model, self.config.enable_experimental) {
            self.gemini_transformer
                .transform_chat_response(response_json, &model)
        } else if model.is_partner_model() {
            self.partner_transformer
                .transform_chat_response(response_json, &model)
        } else {
            Err(ProviderError::model_not_found(
                "vertex_ai",
                model.model_id(),
            ))
        }
    }

    /// Error
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(VertexAIErrorMapper)
    }
}

#[cfg(test)]
fn parse_vertex_usage(response: &Value) -> Option<crate::core::types::responses::Usage> {
    response
        .get("usageMetadata")
        .and_then(strict_vertex_usage_metadata)
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
