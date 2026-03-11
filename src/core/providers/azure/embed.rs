//! Azure OpenAI Embedding Handler
//!
//! Complete embedding functionality for Azure OpenAI Service

use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use crate::core::types::{
    context::RequestContext,
    embedding::EmbeddingRequest,
    responses::{EmbeddingData, EmbeddingResponse},
};

use super::config::AzureConfig;
use super::error::{azure_api_error, azure_config_error, azure_header_error};
use super::utils::{AzureEndpointType, AzureUtils};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig;
use crate::utils::net::http::create_custom_client;

/// Azure OpenAI embedding handler
#[derive(Debug, Clone)]
pub struct AzureEmbeddingHandler {
    config: AzureConfig,
    client: reqwest::Client,
}

impl AzureEmbeddingHandler {
    /// Create new embedding handler
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = create_custom_client(ProviderConfig::timeout(&config))
            .map_err(|e| azure_config_error(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Build request headers
    async fn build_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();

        // Add API key
        if let Some(api_key) = self.config.get_effective_api_key().await {
            headers.insert(
                "api-key",
                api_key
                    .parse()
                    .map_err(|e| azure_header_error(format!("Invalid API key: {}", e)))?,
            );
        } else {
            return Err(ProviderError::authentication(
                "azure",
                "No API key available",
            ));
        }

        headers.insert(
            "Content-Type",
            "application/json"
                .parse()
                .map_err(|e| azure_header_error(format!("Invalid content type: {}", e)))?,
        );

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| azure_header_error(format!("Invalid header name: {}", e)))?;
            let header_value = value
                .parse()
                .map_err(|e| azure_header_error(format!("Invalid header value: {}", e)))?;
            headers.insert(header_name, header_value);
        }

        Ok(headers)
    }

    /// Create embeddings
    pub async fn create_embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        // Validate request
        AzureEmbeddingUtils::validate_request(&request)?;

        // Get deployment name (Azure uses deployment names for embeddings too)
        let deployment = self.config.get_effective_deployment_name(&request.model);

        // Get Azure endpoint
        let azure_endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured"))?;

        // Build URL
        let url = AzureUtils::build_azure_url(
            &azure_endpoint,
            &deployment,
            &self.config.api_version,
            AzureEndpointType::Embeddings,
        );

        // Transform request
        let azure_request = AzureEmbeddingUtils::transform_request(&request)?;

        // Build headers
        let headers = self.build_headers().await?;

        // Execute request
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&azure_request)
            .send()
            .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(azure_api_error(status, error_body));
        }

        // Parse response
        let response_json: Value = response.json().await?;

        // Transform response
        AzureEmbeddingUtils::transform_response(response_json, &request.model)
    }
}

/// Azure embedding utilities
pub struct AzureEmbeddingUtils;

impl AzureEmbeddingUtils {
    /// Validate embedding request
    pub fn validate_request(request: &EmbeddingRequest) -> Result<(), ProviderError> {
        // Check if input is empty based on the enum variant
        let is_empty = match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => text.is_empty(),
            crate::core::types::embedding::EmbeddingInput::Array(array) => array.is_empty(),
        };

        if is_empty {
            return Err(azure_config_error("Input cannot be empty"));
        }

        if request.model.is_empty() {
            return Err(azure_config_error("Model cannot be empty"));
        }

        // Validate dimensions if specified (only for certain models)
        if let Some(dimensions) = request.dimensions
            && (dimensions == 0 || dimensions > 3072)
        {
            return Err(azure_config_error("Dimensions must be between 1 and 3072"));
        }

        Ok(())
    }

    /// Transform request to Azure format
    pub fn transform_request(request: &EmbeddingRequest) -> Result<Value, ProviderError> {
        let mut body = json!({
            "model": request.model,
        });

        // Handle input based on enum variant
        match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => {
                body["input"] = json!(text);
            }
            crate::core::types::embedding::EmbeddingInput::Array(array) => {
                body["input"] = json!(array);
            }
        }

        // Add optional parameters
        if let Some(dimensions) = request.dimensions {
            body["dimensions"] = json!(dimensions);
        }

        if let Some(user) = &request.user {
            body["user"] = json!(user);
        }

        if let Some(encoding_format) = &request.encoding_format {
            body["encoding_format"] = json!(encoding_format);
        }

        Ok(body)
    }

    /// Transform Azure response to standard format
    pub fn transform_response(
        response: Value,
        model: &str,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let data = response["data"]
            .as_array()
            .ok_or_else(|| ProviderError::serialization("azure", "Missing data array"))?
            .iter()
            .map(|item| {
                let embedding = item["embedding"]
                    .as_array()
                    .ok_or_else(|| {
                        ProviderError::serialization("azure", "Missing embedding array")
                    })?
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();

                Ok(EmbeddingData {
                    index: item["index"].as_u64().unwrap_or(0) as u32,
                    embedding,
                    object: "embedding".to_string(),
                })
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;

        // Calculate usage
        let usage = response["usage"]
            .as_object()
            .map(|u| crate::core::types::responses::Usage {
                prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens_details: None,
                prompt_tokens_details: None,
                thinking_usage: None,
            });

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: model.to_string(),
            usage,
            // For backward compatibility
            embeddings: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::embedding::EmbeddingInput;

    // ==================== Validation Tests ====================

    #[test]
    fn test_validate_request_success_text() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_request_success_array() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_request_empty_text() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("".to_string()),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_empty_array() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Array(vec![]),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_empty_model() {
        let request = EmbeddingRequest {
            model: "".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_valid_dimensions() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: Some(256),
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_request_dimensions_zero() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: Some(0),
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_dimensions_too_large() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: Some(3073),
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_max_dimensions() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: Some(3072),
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::validate_request(&request);
        assert!(result.is_ok());
    }

    // ==================== Transform Request Tests ====================

    #[test]
    fn test_transform_request_basic_text() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["model"], "text-embedding-ada-002");
        assert_eq!(body["input"], "Hello world");
    }

    #[test]
    fn test_transform_request_array_input() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
            dimensions: None,
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert!(body["input"].is_array());
        assert_eq!(body["input"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_transform_request_with_dimensions() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: Some(256),
            user: None,
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["dimensions"], 256);
    }

    #[test]
    fn test_transform_request_with_user() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: None,
            user: Some("user-123".to_string()),
            encoding_format: None,
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["user"], "user-123");
    }

    #[test]
    fn test_transform_request_with_encoding_format() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            dimensions: None,
            user: None,
            encoding_format: Some("base64".to_string()),
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["encoding_format"], "base64");
    }

    #[test]
    fn test_transform_request_with_all_options() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            dimensions: Some(512),
            user: Some("user-456".to_string()),
            encoding_format: Some("float".to_string()),
            task_type: None,
        };

        let result = AzureEmbeddingUtils::transform_request(&request);
        assert!(result.is_ok());

        let body = result.unwrap();
        assert_eq!(body["model"], "text-embedding-3-large");
        assert_eq!(body["input"], "Hello world");
        assert_eq!(body["dimensions"], 512);
        assert_eq!(body["user"], "user-456");
        assert_eq!(body["encoding_format"], "float");
    }

    // ==================== Transform Response Tests ====================

    #[test]
    fn test_transform_response_basic() {
        let response = json!({
            "object": "list",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
                }
            ],
            "model": "text-embedding-ada-002",
            "usage": {
                "prompt_tokens": 2,
                "completion_tokens": 0,
                "total_tokens": 2
            }
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.object, "list");
        assert_eq!(embedding_response.model, "text-embedding-ada-002");
        assert_eq!(embedding_response.data.len(), 1);
        assert_eq!(embedding_response.data[0].index, 0);
        assert_eq!(embedding_response.data[0].embedding.len(), 5);
    }

    #[test]
    fn test_transform_response_multiple_embeddings() {
        let response = json!({
            "data": [
                {
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                },
                {
                    "index": 1,
                    "embedding": [0.4, 0.5, 0.6]
                }
            ],
            "model": "text-embedding-ada-002"
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 2);
        assert_eq!(embedding_response.data[0].index, 0);
        assert_eq!(embedding_response.data[1].index, 1);
    }

    #[test]
    fn test_transform_response_with_usage() {
        let response = json!({
            "data": [
                {
                    "index": 0,
                    "embedding": [0.1, 0.2]
                }
            ],
            "model": "text-embedding-ada-002",
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 0,
                "total_tokens": 10
            }
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert!(embedding_response.usage.is_some());
        let usage = embedding_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.total_tokens, 10);
    }

    #[test]
    fn test_transform_response_without_usage() {
        let response = json!({
            "data": [
                {
                    "index": 0,
                    "embedding": [0.1, 0.2]
                }
            ],
            "model": "text-embedding-ada-002"
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        assert!(embedding_response.usage.is_none());
    }

    #[test]
    fn test_transform_response_missing_data() {
        let response = json!({
            "model": "text-embedding-ada-002"
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_missing_embedding() {
        let response = json!({
            "data": [
                {
                    "index": 0
                }
            ],
            "model": "text-embedding-ada-002"
        });

        let result = AzureEmbeddingUtils::transform_response(response, "text-embedding-ada-002");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_embedding_values() {
        let response = json!({
            "data": [
                {
                    "index": 0,
                    "embedding": [0.123456789, -0.987654321, 0.0]
                }
            ]
        });

        let result = AzureEmbeddingUtils::transform_response(response, "test-model");
        assert!(result.is_ok());

        let embedding_response = result.unwrap();
        let embedding = &embedding_response.data[0].embedding;
        assert_eq!(embedding.len(), 3);
        // Check values are approximately correct (f64 to f32 conversion)
        assert!((embedding[0] - 0.123_456_79_f32).abs() < 0.0001);
        assert!((embedding[1] - (-0.987_654_3_f32)).abs() < 0.0001);
        assert!((embedding[2] - 0.0_f32).abs() < 0.0001);
    }

    // ==================== Handler Tests ====================

    #[test]
    fn test_embedding_handler_new_success() {
        let config = AzureConfig::new()
            .with_api_key("test-key".to_string())
            .with_azure_endpoint("https://test.openai.azure.com".to_string());

        let handler = AzureEmbeddingHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_embedding_handler_new_basic_config() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());

        let handler = AzureEmbeddingHandler::new(config);
        assert!(handler.is_ok());
    }
}
