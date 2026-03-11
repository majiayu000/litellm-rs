//! Azure AI Embedding Handler
//!
//! Complete embedding functionality for Azure AI services following unified architecture

use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use super::config::{AzureAIConfig, AzureAIEndpointType};
use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    context::RequestContext,
    embedding::EmbeddingRequest,
    responses::{EmbeddingData, EmbeddingResponse},
};
use crate::utils::net::http::create_custom_client_with_headers;

/// Azure AI embedding handler following unified architecture
#[derive(Debug, Clone)]
pub struct AzureAIEmbeddingHandler {
    config: AzureAIConfig,
    client: reqwest::Client,
}

impl AzureAIEmbeddingHandler {
    /// Create a new embedding handler
    pub fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        // Create headers for the client
        let mut headers = HeaderMap::new();
        let default_headers = config
            .create_default_headers()
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        for (key, value) in default_headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration("azure_ai", format!("Invalid header name: {}", e))
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                ProviderError::configuration("azure_ai", format!("Invalid header value: {}", e))
            })?;
            headers.insert(header_name, header_value);
        }

        let client = create_custom_client_with_headers(config.timeout(), headers).map_err(|e| {
            ProviderError::configuration("azure_ai", format!("Failed to create HTTP client: {}", e))
        })?;

        Ok(Self { config, client })
    }

    /// Handle embedding request
    pub async fn embedding(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Validate request
        AzureAIEmbeddingUtils::validate_request(&request)?;

        // Transform request to Azure AI format
        let azure_request = AzureAIEmbeddingUtils::transform_request(&request)?;

        // Build URL
        let url = if self.is_multimodal_embedding_model(&request.model) {
            // Use image embeddings endpoint for multimodal models
            self.config
                .build_endpoint_url(AzureAIEndpointType::ImageEmbeddings.as_path())
        } else {
            // Use regular embeddings endpoint
            self.config
                .build_endpoint_url(AzureAIEndpointType::Embeddings.as_path())
        }
        .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        // Execute request
        let response = self
            .client
            .post(&url)
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure_ai", format!("Request failed: {}", e)))?;

        // Handle error responses
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpErrorMapper::map_status_code(
                "azure_ai",
                status,
                &error_body,
            ));
        }

        // Parse response
        let response_json: Value = response.json().await.map_err(|e| {
            ProviderError::response_parsing("azure_ai", format!("Failed to parse response: {}", e))
        })?;

        // Transform to standard format
        AzureAIEmbeddingUtils::transform_response(response_json, &request.model)
    }

    /// Check if model is multimodal embedding model
    fn is_multimodal_embedding_model(&self, model: &str) -> bool {
        model.contains("cohere-embed") || model.contains("multimodal")
    }
}

/// Utility struct for Azure AI embedding operations
pub struct AzureAIEmbeddingUtils;

impl AzureAIEmbeddingUtils {
    /// Validate embedding request
    pub fn validate_request(request: &EmbeddingRequest) -> Result<(), ProviderError> {
        // Check if input is empty based on the enum variant
        let is_empty = match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => text.is_empty(),
            crate::core::types::embedding::EmbeddingInput::Array(array) => array.is_empty(),
        };

        if is_empty {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Input cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Model cannot be empty",
            ));
        }

        // Validate dimensions if specified
        if let Some(dimensions) = request.dimensions
            && (dimensions == 0 || dimensions > 3072)
        {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Dimensions must be between 1 and 3072",
            ));
        }

        Ok(())
    }

    /// Transform EmbeddingRequest to Azure AI format
    pub fn transform_request(request: &EmbeddingRequest) -> Result<Value, ProviderError> {
        let mut azure_request = json!({
            "model": request.model,
            "input": request.input
        });

        // Add optional parameters
        if let Some(encoding_format) = &request.encoding_format {
            azure_request["encoding_format"] = json!(encoding_format);
        }

        if let Some(dimensions) = request.dimensions {
            azure_request["dimensions"] = json!(dimensions);
        }

        if let Some(user) = &request.user {
            azure_request["user"] = json!(user);
        }

        Ok(azure_request)
    }

    /// Transform Azure AI response to EmbeddingResponse
    pub fn transform_response(
        response: Value,
        model: &str,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Parse data array
        let data_array = response["data"].as_array().ok_or_else(|| {
            ProviderError::response_parsing("azure_ai", "Missing or invalid 'data' field")
        })?;

        let mut embedding_data = Vec::new();

        for (index, item) in data_array.iter().enumerate() {
            let embedding_vec = item["embedding"]
                .as_array()
                .ok_or_else(|| {
                    ProviderError::response_parsing("azure_ai", "Missing embedding vector")
                })?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect::<Vec<f32>>();

            embedding_data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: item["index"].as_u64().unwrap_or(index as u64) as u32,
                embedding: embedding_vec,
            });
        }

        // Parse usage information
        let usage = response
            .get("usage")
            .map(|usage_data| crate::core::types::responses::Usage {
                prompt_tokens: usage_data["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: 0, // Embeddings don't have completion tokens
                total_tokens: usage_data["total_tokens"].as_u64().unwrap_or(0) as u32,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            });

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: embedding_data,
            model: model.to_string(),
            usage,
            embeddings: None, // Backward compatibility field
        })
    }

    /// Get supported encoding formats for model
    pub fn get_supported_encoding_formats(model: &str) -> Vec<&'static str> {
        match model {
            m if m.contains("text-embedding-3") => vec!["float", "base64"],
            m if m.contains("cohere") => vec!["float"],
            _ => vec!["float"],
        }
    }

    /// Get default dimensions for model
    pub fn get_default_dimensions(model: &str) -> Option<u32> {
        match model {
            m if m.contains("text-embedding-3-large") => Some(3072),
            m if m.contains("text-embedding-3-small") => Some(1536),
            m if m.contains("cohere-embed") => Some(1024),
            _ => None,
        }
    }

    /// Get maximum input length for model
    pub fn get_max_input_length(model: &str) -> u32 {
        match model {
            m if m.contains("text-embedding-3") => 8192,
            m if m.contains("cohere-embed") => 512,
            _ => 2048,
        }
    }

    /// Check if model supports batch processing
    pub fn supports_batch_processing(model: &str) -> bool {
        // Most embedding models support batch processing
        !model.contains("legacy")
    }

    /// Calculate approximate token count for input
    pub fn estimate_token_count(input: &[String]) -> u32 {
        // Rough estimation: ~4 characters per token on average
        input
            .iter()
            .map(|s| (s.len() as f32 / 4.0).ceil() as u32)
            .sum()
    }
}

/// Embedding model capabilities
#[derive(Debug, Clone)]
pub struct EmbeddingModelCapabilities {
    pub max_input_length: u32,
    pub default_dimensions: Option<u32>,
    pub max_dimensions: u32,
    pub supports_batch: bool,
    pub supports_multimodal: bool,
    pub encoding_formats: Vec<String>,
}

impl EmbeddingModelCapabilities {
    /// Get capabilities for a specific model
    pub fn for_model(model: &str) -> Self {
        match model {
            m if m.contains("text-embedding-3-large") => Self {
                max_input_length: 8192,
                default_dimensions: Some(3072),
                max_dimensions: 3072,
                supports_batch: true,
                supports_multimodal: false,
                encoding_formats: vec!["float".to_string(), "base64".to_string()],
            },
            m if m.contains("text-embedding-3-small") => Self {
                max_input_length: 8192,
                default_dimensions: Some(1536),
                max_dimensions: 1536,
                supports_batch: true,
                supports_multimodal: false,
                encoding_formats: vec!["float".to_string(), "base64".to_string()],
            },
            m if m.contains("cohere-embed-v3-multilingual") => Self {
                max_input_length: 512,
                default_dimensions: Some(1024),
                max_dimensions: 1024,
                supports_batch: true,
                supports_multimodal: true,
                encoding_formats: vec!["float".to_string()],
            },
            _ => Self {
                max_input_length: 2048,
                default_dimensions: None,
                max_dimensions: 1536,
                supports_batch: false,
                supports_multimodal: false,
                encoding_formats: vec!["float".to_string()],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::azure_ai::config::AzureAIConfig;

    #[test]
    fn test_embedding_utils_validation() {
        use crate::core::types::embedding::EmbeddingInput;

        let mut request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Array(vec!["test".to_string()]),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        // Valid request should pass
        assert!(AzureAIEmbeddingUtils::validate_request(&request).is_ok());

        // Empty input should fail
        request.input = EmbeddingInput::Array(vec![]);
        assert!(AzureAIEmbeddingUtils::validate_request(&request).is_err());

        // Empty model should fail
        request.input = EmbeddingInput::Array(vec!["test".to_string()]);
        request.model = "".to_string();
        assert!(AzureAIEmbeddingUtils::validate_request(&request).is_err());
    }

    #[test]
    fn test_model_capabilities() {
        let caps = EmbeddingModelCapabilities::for_model("text-embedding-3-large");
        assert_eq!(caps.max_input_length, 8192);
        assert_eq!(caps.default_dimensions, Some(3072));
        assert!(caps.supports_batch);
        assert!(!caps.supports_multimodal);

        let cohere_caps = EmbeddingModelCapabilities::for_model("cohere-embed-v3-multilingual");
        assert_eq!(cohere_caps.max_input_length, 512);
        assert!(cohere_caps.supports_multimodal);
    }

    #[test]
    fn test_token_estimation() {
        let input = vec!["Hello world".to_string(), "This is a test".to_string()];
        let estimated = AzureAIEmbeddingUtils::estimate_token_count(&input);
        assert!(estimated > 0);
        assert!(estimated < 20); // Should be reasonable estimate
    }

    #[test]
    fn test_request_transformation() {
        use crate::core::types::embedding::EmbeddingInput;

        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Array(vec!["test input".to_string()]),
            encoding_format: Some("float".to_string()),
            dimensions: Some(1536),
            user: Some("test-user".to_string()),
            task_type: None,
        };

        let result = AzureAIEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let azure_request = result.unwrap();
        assert_eq!(azure_request["model"], "text-embedding-3-large");
        assert_eq!(azure_request["encoding_format"], "float");
        assert_eq!(azure_request["dimensions"], 1536);
        assert_eq!(azure_request["user"], "test-user");
    }

    #[test]
    fn test_multimodal_detection() {
        let config = AzureAIConfig::new("azure_ai");
        if let Ok(handler) = AzureAIEmbeddingHandler::new(config) {
            assert!(handler.is_multimodal_embedding_model("cohere-embed-v3-multilingual"));
            assert!(!handler.is_multimodal_embedding_model("text-embedding-3-large"));
        }
    }
}
