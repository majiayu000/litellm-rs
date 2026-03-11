//! Cohere Embeddings Handler
//!
//! Handles embedding requests for Cohere embed models.
//! Supports text and image embeddings with various input types.

#[cfg(test)]
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::config::CohereConfig;
use super::error::CohereError;
use crate::core::types::responses::{EmbeddingData, EmbeddingResponse, Usage};
use crate::core::types::{embedding::EmbeddingInput, embedding::EmbeddingRequest};

/// Text and image inputs for Cohere embeddings
type ExtractedInputs = (Option<Vec<String>>, Option<Vec<String>>);

/// Cohere embedding input types
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CohereEmbeddingInputType {
    /// For embedding documents to be stored in a vector database
    #[default]
    SearchDocument,
    /// For embedding search queries
    SearchQuery,
    /// For classification tasks
    Classification,
    /// For clustering tasks
    Clustering,
    /// For image embeddings
    Image,
}

#[cfg(test)]
impl std::fmt::Display for CohereEmbeddingInputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SearchDocument => write!(f, "search_document"),
            Self::SearchQuery => write!(f, "search_query"),
            Self::Classification => write!(f, "classification"),
            Self::Clustering => write!(f, "clustering"),
            Self::Image => write!(f, "image"),
        }
    }
}

/// Embedding handler utilities
pub struct CohereEmbeddingHandler;

impl CohereEmbeddingHandler {
    /// Transform EmbeddingRequest to Cohere format
    pub fn transform_request(
        request: &EmbeddingRequest,
        config: &CohereConfig,
    ) -> Result<Value, CohereError> {
        let (texts, images) = Self::extract_inputs(&request.input)?;
        let is_image_request = images.is_some();

        let input_type = if is_image_request {
            "image".to_string()
        } else {
            request
                .task_type
                .clone()
                .unwrap_or_else(|| config.default_embedding_input_type.clone())
        };

        let mut body = json!({
            "model": request.model,
            "input_type": input_type,
        });

        if let Some(texts) = texts {
            body["texts"] = json!(texts);
        }

        if let Some(images) = images {
            body["images"] = json!(images);
        }

        // Map encoding_format to embedding_types
        if let Some(encoding_format) = &request.encoding_format {
            body["embedding_types"] = json!([encoding_format]);
        }

        // Map dimensions to output_dimension
        if let Some(dimensions) = request.dimensions {
            body["output_dimension"] = json!(dimensions);
        }

        Ok(body)
    }

    /// Extract inputs from EmbeddingInput
    fn extract_inputs(input: &EmbeddingInput) -> Result<ExtractedInputs, CohereError> {
        match input {
            EmbeddingInput::Text(text) => Ok((Some(vec![text.clone()]), None)),
            EmbeddingInput::Array(arr) => {
                // Check if inputs are base64 encoded images
                let is_image = arr
                    .first()
                    .map(|s| Self::is_base64_image(s))
                    .unwrap_or(false);

                if is_image {
                    Ok((None, Some(arr.clone())))
                } else {
                    Ok((Some(arr.clone()), None))
                }
            }
        }
    }

    /// Check if a string is a base64 encoded image
    fn is_base64_image(s: &str) -> bool {
        // Simple heuristic: check for data URI or long base64 string
        s.starts_with("data:image")
            || (s.len() > 1000
                && s.chars()
                    .all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '='))
    }

    /// Transform Cohere response to standard EmbeddingResponse
    pub fn transform_response(
        response_json: Value,
        model: &str,
        input_count: usize,
    ) -> Result<EmbeddingResponse, CohereError> {
        let embeddings = response_json.get("embeddings").ok_or_else(|| {
            super::error::cohere_response_parsing("Missing embeddings in response")
        })?;

        // Get the first available embedding type
        let embedding_vectors = Self::extract_embeddings(embeddings)?;

        let mut data = Vec::new();
        for (index, embedding) in embedding_vectors.into_iter().enumerate() {
            data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: index as u32,
                embedding,
            });
        }

        // Calculate usage
        let usage = Self::extract_usage(&response_json, input_count);

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model: model.to_string(),
            usage: Some(usage),
            embeddings: None,
        })
    }

    /// Extract embeddings from the response
    fn extract_embeddings(embeddings: &Value) -> Result<Vec<Vec<f32>>, CohereError> {
        // Try float first (most common)
        if let Some(float_embeddings) = embeddings.get("float")
            && let Some(arr) = float_embeddings.as_array()
        {
            return arr
                .iter()
                .map(|emb| {
                    emb.as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|n| n.as_f64().map(|f| f as f32))
                                .collect()
                        })
                        .ok_or_else(|| {
                            super::error::cohere_response_parsing("Invalid embedding format")
                        })
                })
                .collect();
        }

        // Fallback: try to parse embeddings directly as a nested array
        if let Some(arr) = embeddings.as_array()
            && let Some(first) = arr.first()
            && first.is_array()
        {
            return arr
                .iter()
                .map(|emb| {
                    emb.as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|n| n.as_f64().map(|f| f as f32))
                                .collect()
                        })
                        .ok_or_else(|| {
                            super::error::cohere_response_parsing("Invalid embedding format")
                        })
                })
                .collect();
        }

        Err(super::error::cohere_response_parsing(
            "No valid embeddings found in response",
        ))
    }

    /// Extract usage from response
    fn extract_usage(response_json: &Value, input_count: usize) -> Usage {
        let mut prompt_tokens = 0u32;

        if let Some(meta) = response_json.get("meta")
            && let Some(billed_units) = meta.get("billed_units")
        {
            if let Some(input_tokens) = billed_units.get("input_tokens").and_then(|v| v.as_u64()) {
                prompt_tokens = input_tokens as u32;
            }
            if let Some(images) = billed_units.get("images").and_then(|v| v.as_u64()) {
                prompt_tokens += images as u32;
            }
        }

        // If no usage info, estimate based on input count
        if prompt_tokens == 0 {
            prompt_tokens = (input_count * 100) as u32; // Rough estimate
        }

        Usage {
            prompt_tokens,
            completion_tokens: 0,
            total_tokens: prompt_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }
    }

    /// Get supported OpenAI parameters for embeddings
    pub fn get_supported_params() -> &'static [&'static str] {
        &["encoding_format", "dimensions"]
    }

    /// Get default dimensions for a model
    #[cfg(test)]
    pub fn get_default_dimensions(model: &str) -> Option<u32> {
        match model {
            m if m.contains("embed-english-v3") => Some(1024),
            m if m.contains("embed-multilingual-v3") => Some(1024),
            m if m.contains("embed-english-v2") => Some(4096),
            m if m.contains("embed-multilingual-v2") => Some(768),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_type_display() {
        assert_eq!(
            CohereEmbeddingInputType::SearchDocument.to_string(),
            "search_document"
        );
        assert_eq!(
            CohereEmbeddingInputType::SearchQuery.to_string(),
            "search_query"
        );
        assert_eq!(
            CohereEmbeddingInputType::Classification.to_string(),
            "classification"
        );
    }

    #[test]
    fn test_extract_inputs_text() {
        let input = EmbeddingInput::Text("Hello world".to_string());
        let (texts, images) = CohereEmbeddingHandler::extract_inputs(&input).unwrap();

        assert!(texts.is_some());
        assert!(images.is_none());
        assert_eq!(texts.unwrap()[0], "Hello world");
    }

    #[test]
    fn test_extract_inputs_array() {
        let input = EmbeddingInput::Array(vec!["Hello".to_string(), "World".to_string()]);
        let (texts, images) = CohereEmbeddingHandler::extract_inputs(&input).unwrap();

        assert!(texts.is_some());
        assert!(images.is_none());
        assert_eq!(texts.unwrap().len(), 2);
    }

    #[test]
    fn test_extract_embeddings() {
        let embeddings = json!({
            "float": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]
        });

        let result = CohereEmbeddingHandler::extract_embeddings(&embeddings).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 3);
    }

    #[test]
    fn test_get_default_dimensions() {
        assert_eq!(
            CohereEmbeddingHandler::get_default_dimensions("embed-english-v3.0"),
            Some(1024)
        );
        assert_eq!(
            CohereEmbeddingHandler::get_default_dimensions("embed-multilingual-v3.0"),
            Some(1024)
        );
        assert_eq!(
            CohereEmbeddingHandler::get_default_dimensions("embed-english-v2.0"),
            Some(4096)
        );
    }

    #[test]
    fn test_supported_params() {
        let params = CohereEmbeddingHandler::get_supported_params();
        assert!(params.contains(&"encoding_format"));
        assert!(params.contains(&"dimensions"));
    }
}
