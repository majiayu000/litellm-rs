//! Jina AI Provider
//!
//! Jina AI provides embeddings and reranking capabilities.
//! Reference: <https://jina.ai/embeddings/> and <https://jina.ai/reranker/>

use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HttpErrorMapper, UrlBuilder, apply_headers, get_pricing_db, header,
    header_static,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::{
    error_mapper::trait_def::ErrorMapper, provider::ProviderConfig,
    provider::llm_provider::trait_definition::LLMProvider,
};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

// Static capabilities for Jina AI (embeddings and reranking focused)
const JINA_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::Embeddings];

/// Jina AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JinaConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.jina.ai/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
}

impl Default for JinaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.jina.ai/v1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

impl ProviderConfig for JinaConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Jina AI")
    }

    fn api_key(&self) -> Option<&str> {
        Some(&self.api_key)
    }

    fn api_base(&self) -> Option<&str> {
        Some(&self.api_base)
    }

    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_seconds)
    }

    fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

/// Jina AI error type (using unified ProviderError)
pub type JinaError = ProviderError;

/// Jina AI error mapper
pub struct JinaErrorMapper;

impl ErrorMapper<JinaError> for JinaErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> JinaError {
        HttpErrorMapper::map_status_code("jina", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> JinaError {
        HttpErrorMapper::parse_json_error("jina", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> JinaError {
        ProviderError::network("jina", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> JinaError {
        ProviderError::response_parsing("jina", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> JinaError {
        ProviderError::timeout(
            "jina",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Rerank request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankRequest {
    /// The model to use for reranking
    pub model: String,
    /// The query to rerank documents against
    pub query: String,
    /// The documents to rerank
    pub documents: Vec<String>,
    /// Number of top results to return (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,
    /// Whether to return documents in the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
}

/// Rerank result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    /// Index of the document in the original list
    pub index: usize,
    /// Relevance score
    pub relevance_score: f64,
    /// Document content (if return_documents was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<RerankDocument>,
}

/// Document in rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankDocument {
    /// Document text
    pub text: String,
}

/// Rerank response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResponse {
    /// Response ID
    #[serde(default)]
    pub id: String,
    /// Reranked results
    pub results: Vec<RerankResult>,
    /// Usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RerankUsage>,
}

/// Rerank usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankUsage {
    /// Total tokens used
    #[serde(default)]
    pub total_tokens: u32,
}

/// Jina AI provider implementation
///
/// Jina AI specializes in embeddings and reranking for search applications.
#[derive(Debug, Clone)]
pub struct JinaProvider {
    config: JinaConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl JinaProvider {
    /// Create a new Jina AI provider instance
    pub async fn new(config: JinaConfig) -> Result<Self, JinaError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("jina", e))?;

        // Create base HTTP client
        let base_config = BaseConfig {
            api_key: Some(config.api_key.clone()),
            api_base: Some(config.api_base.clone()),
            timeout: config.timeout_seconds,
            max_retries: config.max_retries,
            headers: HashMap::new(),
            organization: None,
            api_version: None,
        };

        let base_client = BaseHttpClient::new(base_config)?;

        // Define supported models
        // Pricing: https://jina.ai/embeddings/#pricing
        let models = vec![
            ModelInfo {
                id: "jina-embeddings-v3".to_string(),
                name: "Jina Embeddings v3".to_string(),
                provider: "jina".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: true, // Supports images
                input_cost_per_1k_tokens: Some(0.00002),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "jina-embeddings-v2-base-en".to_string(),
                name: "Jina Embeddings v2 Base English".to_string(),
                provider: "jina".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.00002),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "jina-embeddings-v2-small-en".to_string(),
                name: "Jina Embeddings v2 Small English".to_string(),
                provider: "jina".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.00001),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "jina-reranker-v2-base-multilingual".to_string(),
                name: "Jina Reranker v2 Base Multilingual".to_string(),
                provider: "jina".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                // Jina reranker pricing: $0.000000018 per token
                input_cost_per_1k_tokens: Some(0.000018),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
            ModelInfo {
                id: "jina-colbert-v2".to_string(),
                name: "Jina ColBERT v2".to_string(),
                provider: "jina".to_string(),
                max_context_length: 8192,
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(0.00002),
                output_cost_per_1k_tokens: Some(0.0),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            },
        ];

        Ok(Self {
            config,
            base_client,
            models,
        })
    }

    /// Create provider with just API key using default configuration
    pub async fn with_api_key(api_key: impl Into<String>) -> Result<Self, JinaError> {
        let config = JinaConfig {
            api_key: api_key.into(),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Check if model is a reranker model
    pub fn is_reranker_model(&self, model: &str) -> bool {
        model.contains("reranker") || model.contains("colbert")
    }

    /// Check if model is an embedding model
    pub fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embeddings") || model.contains("embedding")
    }

    /// Check if input is base64 encoded image
    fn is_base64_encoded(input: &str) -> bool {
        // Check for common base64 image prefixes
        input.starts_with("data:image/") || {
            // Check if it looks like base64 (simplified check)
            let stripped = input.trim();
            !stripped.is_empty()
                && stripped.len() > 100
                && stripped.chars().all(|c| {
                    c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c == ','
                })
        }
    }

    /// Transform embedding input for Jina AI
    /// Jina supports both text and image inputs
    fn transform_embedding_input(&self, input: &Value) -> Value {
        match input {
            Value::String(s) => {
                if Self::is_base64_encoded(s) {
                    // Extract base64 data from data URL if present
                    let img_data = if let Some(comma_pos) = s.find(',') {
                        s[comma_pos + 1..].to_string()
                    } else {
                        s.clone()
                    };
                    serde_json::json!({"image": img_data})
                } else {
                    serde_json::json!({"text": s})
                }
            }
            Value::Array(arr) => {
                let transformed: Vec<Value> = arr
                    .iter()
                    .map(|item| self.transform_embedding_input(item))
                    .collect();
                Value::Array(transformed)
            }
            _ => input.clone(),
        }
    }

    /// Execute rerank request
    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, JinaError> {
        debug!("Jina rerank request: model={}", request.model);

        let body = serde_json::json!({
            "model": request.model,
            "query": request.query,
            "documents": request.documents,
            "top_n": request.top_n,
            "return_documents": request.return_documents.unwrap_or(true),
        });

        // Build URL and headers
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/rerank")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_headers(self.base_client.inner().post(&url), headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("jina", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("jina", status, &body));
        }

        // Parse and transform response
        let raw_response: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("jina", e.to_string()))?;

        // Transform Jina's response format to match expected format
        // Jina returns: {"index": 0, "relevance_score": 0.72, "document": "hello"}
        // We need: {"index": 0, "relevance_score": 0.72, "document": {"text": "hello"}}
        let results: Vec<RerankResult> = if let Some(results) = raw_response.get("results") {
            results
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            let index = item.get("index")?.as_u64()? as usize;
                            let relevance_score = item.get("relevance_score")?.as_f64()?;
                            let document = item.get("document").and_then(|d| {
                                if d.is_string() {
                                    Some(RerankDocument {
                                        text: d.as_str()?.to_string(),
                                    })
                                } else if d.is_object() {
                                    d.get("text").and_then(|t| t.as_str()).map(|text| {
                                        RerankDocument {
                                            text: text.to_string(),
                                        }
                                    })
                                } else {
                                    None
                                }
                            });

                            Some(RerankResult {
                                index,
                                relevance_score,
                                document,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let usage = raw_response.get("usage").and_then(|u| {
            Some(RerankUsage {
                total_tokens: u.get("total_tokens")?.as_u64()? as u32,
            })
        });

        let id = raw_response
            .get("id")
            .and_then(|i| i.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        Ok(RerankResponse { id, results, usage })
    }

    /// Calculate rerank cost
    pub fn calculate_rerank_cost(&self, model: &str, total_tokens: u32) -> Result<f64, JinaError> {
        let model_info = self
            .models
            .iter()
            .find(|m| m.id == model)
            .ok_or_else(|| ProviderError::model_not_found("jina", model.to_string()))?;

        let cost_per_1k = model_info.input_cost_per_1k_tokens.unwrap_or(0.0);
        Ok((total_tokens as f64 / 1000.0) * cost_per_1k)
    }
}

impl LLMProvider for JinaProvider {
    fn name(&self) -> &'static str {
        "jina"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        JINA_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        // Jina embeddings support dimensions parameter
        &["dimensions"]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        let mut mapped = HashMap::new();

        for (key, value) in params {
            if key == "dimensions" {
                mapped.insert(key, value);
            }
            // Other parameters are filtered out
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        // Jina AI doesn't support chat completions
        Err(ProviderError::not_supported(
            "jina",
            "Chat completions are not supported. Use embeddings() or rerank() instead.",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Jina AI doesn't support chat completions
        Err(ProviderError::not_supported(
            "jina",
            "Chat completions are not supported",
        ))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(JinaErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        // Jina AI doesn't support chat completions
        Err(ProviderError::not_supported(
            "jina",
            "Chat completions are not supported. Use embeddings() or rerank() instead.",
        ))
    }

    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        // Jina AI doesn't support streaming chat
        Err(ProviderError::not_supported(
            "jina",
            "Streaming chat completions are not supported",
        ))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        debug!("Jina embedding request: model={}", request.model);

        // Check if using reranker model for embeddings
        if self.is_reranker_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "jina",
                "Use rerank() method for reranker models",
            ));
        }

        // Transform input for Jina (handle base64 images)
        let input_value = serde_json::to_value(&request.input)
            .map_err(|e| ProviderError::serialization("jina", e.to_string()))?;

        let transformed_input = self.transform_embedding_input(&input_value);

        // Check if any input is base64 encoded (multimodal)
        let has_images = match &transformed_input {
            Value::Array(arr) => arr.iter().any(|item| item.get("image").is_some()),
            Value::Object(obj) => obj.contains_key("image"),
            _ => false,
        };

        // Build request body
        let mut body = serde_json::json!({
            "model": request.model,
        });

        // Use transformed input if it has images, otherwise use original
        if has_images {
            body["input"] = transformed_input;
        } else {
            body["input"] = input_value;
        }

        // Add dimensions if specified
        if let Some(dimensions) = request.dimensions {
            body["dimensions"] = serde_json::json!(dimensions);
        }

        // Build URL and headers
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/embeddings")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_headers(self.base_client.inner().post(&url), headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("jina", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("jina", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("jina", e.to_string()))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/embeddings")
            .build();

        let body = serde_json::json!({
            "model": "jina-embeddings-v2-small-en",
            "input": ["health check"]
        });

        match apply_headers(
            self.base_client.inner().post(&url),
            vec![
                header("Authorization", format!("Bearer {}", self.config.api_key)),
                header_static("Content-Type", "application/json"),
            ],
        )
        .json(&body)
        .send()
        .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(response) if response.status().as_u16() == 401 => {
                debug!("Jina health check: authentication failed");
                HealthStatus::Degraded
            }
            Ok(response) => {
                debug!("Jina health check failed: status={}", response.status());
                HealthStatus::Unhealthy
            }
            Err(e) => {
                debug!("Jina health check error: {}", e);
                HealthStatus::Unhealthy
            }
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        let usage = crate::core::pricing::Usage::new(input_tokens, output_tokens);
        Ok(get_pricing_db().calculate(model, &usage))
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
