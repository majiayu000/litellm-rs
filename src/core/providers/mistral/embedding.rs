//! Embedding functionality for Mistral provider

use serde_json::{Value, json};
use tracing::{debug, info};

use crate::core::providers::mistral::{MistralConfig, MistralError};

/// Mistral embedding handler
#[derive(Debug, Clone)]
pub struct MistralEmbeddingHandler {
    config: MistralConfig,
}

impl MistralEmbeddingHandler {
    /// Create a new embedding handler
    pub fn new(config: MistralConfig) -> Result<Self, MistralError> {
        Ok(Self { config })
    }

    /// Get the config
    pub fn config(&self) -> &MistralConfig {
        &self.config
    }

    /// Transform an embedding request to Mistral format
    pub fn transform_request(
        &self,
        request: crate::core::types::embedding::EmbeddingRequest,
    ) -> Result<Value, MistralError> {
        let transformed = json!({
            "model": request.model,
            "input": request.input,
            "encoding_format": request.encoding_format.unwrap_or_else(|| "float".to_string()),
        });

        debug!("Transformed Mistral embedding request");
        Ok(transformed)
    }

    /// Transform a Mistral embedding response to standard format
    pub fn transform_response(
        &self,
        response: Value,
    ) -> Result<crate::core::types::responses::EmbeddingResponse, MistralError> {
        use crate::core::types::responses::{EmbeddingData, Usage};

        let object = response
            .get("object")
            .and_then(|v| v.as_str())
            .unwrap_or("list")
            .to_string();

        let model = response
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("mistral-embed")
            .to_string();

        // Parse embeddings data
        let data: Vec<EmbeddingData> = response
            .get("data")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let index = item.get("index")?.as_i64()? as u32;
                        let embedding = item
                            .get("embedding")?
                            .as_array()?
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect();

                        Some(EmbeddingData {
                            object: "embedding".to_string(),
                            index,
                            embedding,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Parse usage
        let usage = response.get("usage").map(|u| {
            Usage {
                prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: 0, // Not applicable for embeddings
                total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }
        });

        info!("Mistral embedding response transformed successfully");

        Ok(crate::core::types::responses::EmbeddingResponse {
            object,
            data: data.clone(),
            model,
            usage,
            embeddings: Some(data),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::embedding::EmbeddingInput;
    use crate::core::types::embedding::EmbeddingRequest;

    fn create_test_config() -> MistralConfig {
        MistralConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_mistral_embedding_handler_new() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config.clone());
        assert!(handler.is_ok());
        let h = handler.unwrap();
        assert_eq!(h.config().api_key, "test-key");
    }

    #[test]
    fn test_transform_request_single_input() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let request = EmbeddingRequest {
            model: "mistral-embed".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["model"], "mistral-embed");
        assert_eq!(value["encoding_format"], "float");
    }

    #[test]
    fn test_transform_request_respects_user_model() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let request = EmbeddingRequest {
            model: "mistral-embed-v2".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["model"], "mistral-embed-v2");
    }

    #[test]
    fn test_transform_request_with_encoding_format() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let request = EmbeddingRequest {
            model: "mistral-embed".to_string(),
            input: EmbeddingInput::Text("Hello".to_string()),
            encoding_format: Some("base64".to_string()),
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["encoding_format"], "base64");
    }

    #[test]
    fn test_transform_request_array_inputs() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let request = EmbeddingRequest {
            model: "mistral-embed".to_string(),
            input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_response_basic() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let response = json!({
            "object": "list",
            "model": "mistral-embed",
            "data": [{
                "object": "embedding",
                "index": 0,
                "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
            }],
            "usage": {
                "prompt_tokens": 5,
                "total_tokens": 5
            }
        });

        let result = handler.transform_response(response);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.object, "list");
        assert_eq!(embedding_response.model, "mistral-embed");
        assert_eq!(embedding_response.data.len(), 1);
        assert_eq!(embedding_response.data[0].index, 0);
        assert_eq!(embedding_response.data[0].embedding.len(), 5);
        assert!(embedding_response.usage.is_some());
        let usage = embedding_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 5);
    }

    #[test]
    fn test_transform_response_multiple_embeddings() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let response = json!({
            "object": "list",
            "model": "mistral-embed",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                },
                {
                    "object": "embedding",
                    "index": 1,
                    "embedding": [0.4, 0.5, 0.6]
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "total_tokens": 10
            }
        });

        let result = handler.transform_response(response);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 2);
        assert_eq!(embedding_response.data[0].index, 0);
        assert_eq!(embedding_response.data[1].index, 1);
    }

    #[test]
    fn test_transform_response_empty_data() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let response = json!({
            "object": "list",
            "model": "mistral-embed",
            "data": []
        });

        let result = handler.transform_response(response);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert!(embedding_response.data.is_empty());
        assert!(embedding_response.usage.is_none());
    }

    #[test]
    fn test_transform_response_default_values() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let response = json!({
            "data": [{
                "index": 0,
                "embedding": [0.1, 0.2]
            }]
        });

        let result = handler.transform_response(response);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.object, "list");
        assert_eq!(embedding_response.model, "mistral-embed");
    }

    #[test]
    fn test_transform_response_embeddings_field() {
        let config = create_test_config();
        let handler = MistralEmbeddingHandler::new(config).unwrap();

        let response = json!({
            "data": [{
                "index": 0,
                "embedding": [0.1, 0.2, 0.3]
            }]
        });

        let result = handler.transform_response(response);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert!(embedding_response.embeddings.is_some());
        let embeddings = embedding_response.embeddings.unwrap();
        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].embedding, vec![0.1f32, 0.2f32, 0.3f32]);
    }
}
