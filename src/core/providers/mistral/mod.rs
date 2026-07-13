//! Mistral AI Provider (Refactored)
//!
//! Mistral AI model integration using the base infrastructure.
//! This implementation eliminates the need for common_utils.rs.

use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tracing::debug;

// Use base infrastructure instead of common_utils
use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HttpErrorMapper, OpenAIRequestTransformer, UrlBuilder,
    apply_provider_headers, get_pricing_db, header, header_static,
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

// Re-export submodules (these remain as-is for now)
pub mod chat;
pub mod embedding;
mod model_catalog;

// Static capabilities
const MISTRAL_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::Embeddings,
];

/// Mistral provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MistralConfig {
    /// API key for authentication
    pub api_key: String,
    /// API base URL (defaults to <https://api.mistral.ai/v1>)
    pub api_base: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Network scope allowed for the configured endpoint
    #[serde(default)]
    pub endpoint_access: crate::core::net::ProviderEndpointAccess,
}

impl Default for MistralConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.mistral.ai/v1".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
            endpoint_access: Default::default(),
        }
    }
}

impl ProviderConfig for MistralConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("Mistral")
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

    fn endpoint_access(&self) -> crate::core::net::ProviderEndpointAccess {
        self.endpoint_access
    }
}

/// Mistral error type (simplified using ProviderError)
pub type MistralError = ProviderError;

/// Mistral error mapper
pub struct MistralErrorMapper;

impl ErrorMapper<MistralError> for MistralErrorMapper {
    fn map_http_error(&self, status_code: u16, response_body: &str) -> MistralError {
        HttpErrorMapper::map_status_code("mistral", status_code, response_body)
    }

    fn map_json_error(&self, error_response: &Value) -> MistralError {
        HttpErrorMapper::parse_json_error("mistral", error_response)
    }

    fn map_network_error(&self, error: &dyn std::error::Error) -> MistralError {
        ProviderError::network("mistral", error.to_string())
    }

    fn map_parsing_error(&self, error: &dyn std::error::Error) -> MistralError {
        ProviderError::response_parsing("mistral", error.to_string())
    }

    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> MistralError {
        ProviderError::timeout(
            "mistral",
            format!("Request timed out after {:?}", timeout_duration),
        )
    }
}

/// Mistral provider implementation (refactored)
#[derive(Debug, Clone)]
pub struct MistralProvider {
    config: MistralConfig,
    base_client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl MistralProvider {
    /// Create a new Mistral provider instance
    pub async fn new(config: MistralConfig) -> Result<Self, MistralError> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| ProviderError::configuration("mistral", e))?;

        // Create base HTTP client using our infrastructure
        let base_config = BaseConfig {
            api_key: Some(config.api_key.clone()),
            api_base: Some(config.api_base.clone()),
            endpoint_access: config.endpoint_access,
            timeout: config.timeout_seconds,
            max_retries: config.max_retries,
            ..Default::default()
        };

        let base_client = BaseHttpClient::new_for_provider("mistral", base_config)?;

        // Define supported models with pricing
        let models = model_catalog::mistral_model_catalog();

        Ok(Self {
            config,
            base_client,
            models,
        })
    }

    /// Check if model is an embedding model
    fn is_embedding_model(&self, model: &str) -> bool {
        model.contains("embed")
    }

    fn canonical_model_id(&self, model: &str) -> String {
        let normalized = model
            .strip_prefix("mistral/")
            .or_else(|| model.strip_prefix("mistralai/"))
            .unwrap_or(model);

        self.models
            .iter()
            .find(|model_info| model_info.id == normalized)
            .and_then(|model_info| model_info.metadata.get("alias_for"))
            .and_then(|alias| alias.as_str())
            .unwrap_or(normalized)
            .to_string()
    }
}

impl LLMProvider for MistralProvider {
    fn name(&self) -> &'static str {
        "mistral"
    }

    fn error_provider_name(&self) -> &'static str {
        "mistral"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        MISTRAL_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "temperature",
            "top_p",
            "max_tokens",
            "stream",
            "stop",
            "random_seed",
            "tools",
            "tool_choice",
            "response_format",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        // Mistral uses OpenAI-compatible parameters, so mostly pass-through
        let mut mapped = HashMap::new();

        for (key, value) in params {
            match key.as_str() {
                // Mistral uses 'random_seed' instead of 'seed'
                "seed" => mapped.insert("random_seed".to_string(), value),
                // Direct pass-through for standard parameters
                "temperature" | "top_p" | "max_tokens" | "stream" | "stop" | "tools"
                | "tool_choice" | "response_format" => mapped.insert(key, value),
                // Skip unsupported parameters
                _ => None,
            };
        }

        Ok(mapped)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        let canonical_model = self.canonical_model_id(&request.model);

        // Use the OpenAI transformer from base_provider
        let mut body = OpenAIRequestTransformer::transform_chat_request(&request);

        // Mistral-specific adjustments
        if let Some(obj) = body.as_object_mut() {
            obj.insert("model".to_string(), Value::String(canonical_model));
            if let Some(seed) = obj.remove("seed") {
                obj.insert("random_seed".to_string(), seed);
            }
        }

        Ok(body)
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Parse Mistral response (OpenAI-compatible format)
        serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(MistralErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        debug!("Mistral chat request: model={}", request.model);

        if self.is_embedding_model(&request.model) {
            return Err(ProviderError::invalid_request(
                "mistral",
                "Use embeddings endpoint for embedding models".to_string(),
            ));
        }

        let body = self.transform_request(request, context).await?;

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_provider_headers(self.base_client.post(&url)?, headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        debug!("Mistral streaming chat request: model={}", request.model);

        let mut body = self.transform_request(request, context).await?;
        body["stream"] = serde_json::json!(true);

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/chat/completions")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_provider_headers(self.base_client.post(&url)?, headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        Ok(crate::core::providers::base::create_provider_sse_stream(
            response, "mistral",
        ))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        debug!("Mistral embedding request: model={}", request.model);

        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            "encoding_format": request.encoding_format,
        });

        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/embeddings")
            .build();

        let headers = vec![
            header("Authorization", format!("Bearer {}", self.config.api_key)),
            header_static("Content-Type", "application/json"),
        ];

        let response = apply_provider_headers(self.base_client.post(&url)?, headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::network("mistral", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(HttpErrorMapper::map_status_code("mistral", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("mistral", e.to_string()))
    }

    async fn health_check(&self) -> HealthStatus {
        let url = UrlBuilder::new(&self.config.api_base)
            .with_path("/models")
            .build();

        let request = match self.base_client.get(&url) {
            Ok(request) => request,
            Err(error) => {
                debug!("Mistral health check policy error: {}", error);
                return HealthStatus::Unhealthy;
            }
        };

        match apply_provider_headers(
            request,
            vec![header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )],
        )
        .send()
        .await
        {
            Ok(response) if response.status().is_success() => HealthStatus::Healthy,
            Ok(response) => {
                debug!("Mistral health check failed: status={}", response.status());
                HealthStatus::Unhealthy
            }
            Err(e) => {
                debug!("Mistral health check error: {}", e);
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
        let canonical_model = self.canonical_model_id(model);
        let usage = crate::core::pricing::Usage::new(input_tokens, output_tokens);
        Ok(get_pricing_db().calculate_for_provider("mistral", &canonical_model, &usage))
    }
}

#[cfg(test)]
mod tests;
