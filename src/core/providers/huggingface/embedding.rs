//! HuggingFace Embedding Handler
//!
//! Handles embedding requests using HuggingFace's text-embeddings-inference.

use serde_json::{Value, json};
use tracing::debug;

use crate::core::providers::huggingface::{HuggingFaceConfig, HuggingFaceError};
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::responses::{EmbeddingData, EmbeddingResponse, Usage};

/// HuggingFace embedding handler
#[derive(Debug, Clone)]
pub struct HuggingFaceEmbeddingHandler;

impl HuggingFaceEmbeddingHandler {
    /// Create a new embedding handler
    pub fn new(_config: HuggingFaceConfig) -> Self {
        Self
    }

    /// Transform an embedding request to HuggingFace format
    pub fn transform_request(&self, request: &EmbeddingRequest) -> Value {
        let input = match &request.input {
            crate::core::types::embedding::EmbeddingInput::Text(text) => {
                json!([text])
            }
            crate::core::types::embedding::EmbeddingInput::Array(texts) => {
                json!(texts)
            }
        };

        let mut body = json!({
            "inputs": input,
        });

        // Add optional parameters
        if let Some(task_type) = &request.task_type {
            body["options"] = json!({
                "wait_for_model": true,
            });
            // Task type could affect how embeddings are computed
            debug!("Embedding task type: {}", task_type);
        }

        body
    }

    /// Transform a HuggingFace embedding response to standard format
    pub fn transform_response(
        &self,
        response: Value,
        model: &str,
        input_count: usize,
    ) -> Result<EmbeddingResponse, HuggingFaceError> {
        // HuggingFace can return different formats based on the task
        let data = self.parse_embeddings(&response)?;

        // Estimate token count (rough approximation)
        let prompt_tokens = (input_count * 10) as u32; // Very rough estimate

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: data.clone(),
            model: model.to_string(),
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens: 0,
                total_tokens: prompt_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            embeddings: Some(data),
        })
    }

    /// Parse embeddings from various HuggingFace response formats
    fn parse_embeddings(&self, response: &Value) -> Result<Vec<EmbeddingData>, HuggingFaceError> {
        // Check for error in response
        if let Some(error) = response.get("error") {
            return Err(HuggingFaceError::huggingface_api_error(
                500,
                error.as_str().unwrap_or("Unknown error"),
            ));
        }

        // Handle similarities response (sentence-similarity task)
        if let Some(similarities) = response.get("similarities") {
            return self.parse_similarities(similarities);
        }

        // Handle array of embeddings (feature-extraction task)
        if let Some(arr) = response.as_array() {
            return self.parse_embedding_array(arr);
        }

        // Handle OpenAI-compatible format
        if let Some(data) = response.get("data")
            && let Some(arr) = data.as_array()
        {
            return arr
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let embedding = item
                        .get("embedding")
                        .and_then(|e| e.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_f64().map(|f| f as f32))
                                .collect()
                        })
                        .unwrap_or_default();

                    Ok(EmbeddingData {
                        object: "embedding".to_string(),
                        index: idx as u32,
                        embedding,
                    })
                })
                .collect();
        }

        // Single embedding response
        if let Some(arr) = response.as_array()
            && !arr.is_empty()
            && arr[0].is_number()
        {
            // Direct embedding vector
            let embedding: Vec<f32> = arr
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();
            return Ok(vec![EmbeddingData {
                object: "embedding".to_string(),
                index: 0,
                embedding,
            }]);
        }

        Err(HuggingFaceError::huggingface_response_parsing(
            "Unable to parse embedding response format",
        ))
    }

    /// Parse similarities response
    fn parse_similarities(
        &self,
        similarities: &Value,
    ) -> Result<Vec<EmbeddingData>, HuggingFaceError> {
        let arr = similarities.as_array().ok_or_else(|| {
            HuggingFaceError::huggingface_response_parsing("Invalid similarities format")
        })?;

        arr.iter()
            .enumerate()
            .map(|(idx, val)| {
                let similarity = val.as_f64().unwrap_or(0.0) as f32;
                Ok(EmbeddingData {
                    object: "embedding".to_string(),
                    index: idx as u32,
                    embedding: vec![similarity], // Similarity score as single value
                })
            })
            .collect()
    }

    /// Parse array of embeddings
    fn parse_embedding_array(&self, arr: &[Value]) -> Result<Vec<EmbeddingData>, HuggingFaceError> {
        arr.iter()
            .enumerate()
            .map(|(idx, item)| {
                let embedding = if let Some(nested) = item.as_array() {
                    // Nested array (embedding vector)
                    if nested.is_empty() {
                        vec![]
                    } else if let Some(first_nested) = nested[0].as_array() {
                        // Double nested (batch of embeddings)
                        first_nested
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    } else {
                        // Single embedding
                        nested
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    }
                } else if item.is_number() {
                    vec![item.as_f64().unwrap_or(0.0) as f32]
                } else {
                    vec![]
                };

                Ok(EmbeddingData {
                    object: "embedding".to_string(),
                    index: idx as u32,
                    embedding,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::embedding::EmbeddingInput;

    fn create_test_handler() -> HuggingFaceEmbeddingHandler {
        HuggingFaceEmbeddingHandler::new(HuggingFaceConfig::new("test_token"))
    }

    #[test]
    fn test_transform_request_single_text() {
        let handler = create_test_handler();
        let request = EmbeddingRequest {
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            input: EmbeddingInput::Text("Hello world".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(&request);
        assert!(result.get("inputs").is_some());
        assert!(result["inputs"].is_array());
    }

    #[test]
    fn test_transform_request_multiple_texts() {
        let handler = create_test_handler();
        let request = EmbeddingRequest {
            model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            input: EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]),
            encoding_format: None,
            dimensions: None,
            user: None,
            task_type: None,
        };

        let result = handler.transform_request(&request);
        let inputs = result["inputs"].as_array().unwrap();
        assert_eq!(inputs.len(), 2);
    }

    #[test]
    fn test_transform_response_openai_format() {
        let handler = create_test_handler();
        let response = json!({
            "data": [
                {
                    "embedding": [0.1, 0.2, 0.3],
                    "index": 0,
                    "object": "embedding"
                }
            ],
            "model": "test-model",
            "object": "list"
        });

        let result = handler.transform_response(response, "test-model", 1);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 1);
        assert_eq!(embedding_response.data[0].embedding.len(), 3);
    }

    #[test]
    fn test_transform_response_array_format() {
        let handler = create_test_handler();
        let response = json!([[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]);

        let result = handler.transform_response(response, "test-model", 2);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 2);
    }

    #[test]
    fn test_transform_response_similarities_format() {
        let handler = create_test_handler();
        let response = json!({
            "similarities": [0.9, 0.7, 0.5]
        });

        let result = handler.transform_response(response, "test-model", 3);
        assert!(result.is_ok());
        let embedding_response = result.unwrap();
        assert_eq!(embedding_response.data.len(), 3);
        assert_eq!(embedding_response.data[0].embedding.len(), 1);
    }

    #[test]
    fn test_transform_response_error() {
        let handler = create_test_handler();
        let response = json!({
            "error": "Model not found"
        });

        let result = handler.transform_response(response, "test-model", 1);
        assert!(result.is_err());
    }
}
