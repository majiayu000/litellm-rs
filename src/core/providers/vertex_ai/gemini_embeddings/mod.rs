//! Gemini Embeddings Module
//!
//! Specialized embeddings using Gemini models

use crate::ProviderError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::embeddings::{TaskType, VertexEmbeddingModel};

/// Gemini embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiEmbeddingRequest {
    pub model: String,
    pub content: ContentInput,
    pub task_type: Option<TaskType>,
    pub title: Option<String>,
    pub output_dimensionality: Option<i32>,
}

/// Content input for embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentInput {
    /// Single text input
    Text(String),
    /// Structured content with parts
    Structured { parts: Vec<ContentPart> },
}

/// Content part for structured input
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentPart {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

/// Inline data for images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String,
}

/// Batch embedding request for processing multiple contents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEmbedContentRequest {
    pub model: String,
    pub requests: Vec<EmbedContentRequest>,
}

/// Individual embed content request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedContentRequest {
    pub content: ContentInput,
    pub task_type: Option<TaskType>,
    pub title: Option<String>,
    pub output_dimensionality: Option<i32>,
}

/// Batch embedding response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEmbedContentResponse {
    pub embeddings: Vec<ContentEmbedding>,
}

/// Content embedding result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentEmbedding {
    pub values: Vec<f32>,
    pub truncated: Option<bool>,
}

/// Gemini embedding handler
pub struct GeminiEmbeddingHandler {
    model: VertexEmbeddingModel,
}

impl GeminiEmbeddingHandler {
    /// Create new Gemini embedding handler
    pub fn new(model: VertexEmbeddingModel, _project_id: String, _location: String) -> Self {
        Self { model }
    }

    /// Generate embedding for single content
    pub async fn embed_content(
        &self,
        request: GeminiEmbeddingRequest,
    ) -> Result<ContentEmbedding, ProviderError> {
        // Transform to Vertex AI format
        let instances =
            vec![self.transform_content_to_instance(&request.content, &request.task_type)?];

        let _body = serde_json::json!({
            "instances": instances,
            "parameters": {
                "outputDimensionality": request.output_dimensionality
            }
        });

        // TODO: Make actual API call
        // For now, return dummy embedding
        Ok(ContentEmbedding {
            values: vec![0.0; self.model.dimensions()],
            truncated: Some(false),
        })
    }

    /// Generate embeddings for multiple contents (batch)
    pub async fn batch_embed_content(
        &self,
        request: BatchEmbedContentRequest,
    ) -> Result<BatchEmbedContentResponse, ProviderError> {
        if request.requests.len() > 100 {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Batch size cannot exceed 100 requests",
            ));
        }

        let instances: Result<Vec<_>, _> = request
            .requests
            .iter()
            .map(|req| self.transform_content_to_instance(&req.content, &req.task_type))
            .collect();

        let instances = instances?;

        let _body = serde_json::json!({
            "instances": instances
        });

        // TODO: Make actual API call
        // For now, return dummy embeddings
        let embeddings = request
            .requests
            .iter()
            .map(|_| ContentEmbedding {
                values: vec![0.0; self.model.dimensions()],
                truncated: Some(false),
            })
            .collect();

        Ok(BatchEmbedContentResponse { embeddings })
    }

    /// Transform content to Vertex AI instance format
    fn transform_content_to_instance(
        &self,
        content: &ContentInput,
        task_type: &Option<TaskType>,
    ) -> Result<Value, ProviderError> {
        let instance = match content {
            ContentInput::Text(text) => {
                serde_json::json!({
                    "content": text,
                    "task_type": task_type.as_ref().unwrap_or(&TaskType::RetrievalDocument)
                })
            }
            ContentInput::Structured { parts } => {
                let content_parts: Result<Vec<_>, _> = parts
                    .iter()
                    .map(|part| self.transform_content_part(part))
                    .collect();

                serde_json::json!({
                    "content": {
                        "parts": content_parts?
                    },
                    "task_type": task_type.as_ref().unwrap_or(&TaskType::RetrievalDocument)
                })
            }
        };

        Ok(instance)
    }

    /// Transform content part to Vertex AI format
    fn transform_content_part(&self, part: &ContentPart) -> Result<Value, ProviderError> {
        match part {
            ContentPart::Text { text } => Ok(serde_json::json!({
                "text": text
            })),
            ContentPart::InlineData { inline_data } => Ok(serde_json::json!({
                "inlineData": {
                    "mimeType": inline_data.mime_type,
                    "data": inline_data.data
                }
            })),
        }
    }

    /// Get supported task types for this model
    pub fn get_supported_task_types(&self) -> Vec<TaskType> {
        vec![
            TaskType::RetrievalQuery,
            TaskType::RetrievalDocument,
            TaskType::SemanticSimilarity,
            TaskType::Classification,
            TaskType::Clustering,
            TaskType::QuestionAnswering,
            TaskType::FactVerification,
        ]
    }

    /// Calculate similarity between two embeddings
    pub fn calculate_cosine_similarity(&self, embedding1: &[f32], embedding2: &[f32]) -> f32 {
        crate::core::providers::shared::cosine_similarity(embedding1, embedding2)
    }

    /// Validate embedding request
    #[cfg(test)]
    fn validate_request(&self, request: &GeminiEmbeddingRequest) -> Result<(), ProviderError> {
        // Check content length
        match &request.content {
            ContentInput::Text(text) => {
                if text.len() > self.model.max_input_length() * 4 {
                    return Err(ProviderError::invalid_request(
                        "vertex_ai",
                        format!("Text too long for model {}", self.model.model_id()),
                    ));
                }
            }
            ContentInput::Structured { parts } => {
                let total_text_length: usize = parts
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text.len()),
                        _ => None,
                    })
                    .sum();

                if total_text_length > self.model.max_input_length() * 4 {
                    return Err(ProviderError::invalid_request(
                        "vertex_ai",
                        "Combined text in parts too long",
                    ));
                }
            }
        }

        // Check output dimensionality
        if let Some(dims) = request.output_dimensionality
            && (dims <= 0 || dims > self.model.dimensions() as i32)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                format!("Invalid output dimensionality: {}", dims),
            ));
        }

        Ok(())
    }
}

/// Batch embedding handler for optimized processing
pub struct BatchGeminiEmbeddingHandler {
    base_handler: GeminiEmbeddingHandler,
    batch_size: usize,
}

impl BatchGeminiEmbeddingHandler {
    /// Create new batch handler
    pub fn new(
        model: VertexEmbeddingModel,
        project_id: String,
        location: String,
        batch_size: usize,
    ) -> Self {
        Self {
            base_handler: GeminiEmbeddingHandler::new(model, project_id, location),
            batch_size: batch_size.min(100), // Max 100 per batch
        }
    }

    /// Process large number of texts in optimized batches
    pub async fn process_texts_in_batches(
        &self,
        texts: Vec<String>,
        task_type: Option<TaskType>,
    ) -> Result<Vec<ContentEmbedding>, ProviderError> {
        let mut all_embeddings = Vec::new();

        for chunk in texts.chunks(self.batch_size) {
            let requests = chunk
                .iter()
                .map(|text| EmbedContentRequest {
                    content: ContentInput::Text(text.clone()),
                    task_type: task_type.clone(),
                    title: None,
                    output_dimensionality: None,
                })
                .collect();

            let batch_request = BatchEmbedContentRequest {
                model: self.base_handler.model.model_id(),
                requests,
            };

            let batch_response = self.base_handler.batch_embed_content(batch_request).await?;
            all_embeddings.extend(batch_response.embeddings);
        }

        Ok(all_embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cosine_similarity() {
        let handler = GeminiEmbeddingHandler::new(
            VertexEmbeddingModel::TextEmbedding004,
            "test".to_string(),
            "us-central1".to_string(),
        );

        let vec1 = vec![1.0, 0.0, 0.0];
        let vec2 = vec![0.0, 1.0, 0.0];
        let vec3 = vec![1.0, 0.0, 0.0];

        // Orthogonal vectors should have similarity 0
        assert!((handler.calculate_cosine_similarity(&vec1, &vec2) - 0.0).abs() < 1e-6);

        // Identical vectors should have similarity 1
        assert!((handler.calculate_cosine_similarity(&vec1, &vec3) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_validate_request() {
        let handler = GeminiEmbeddingHandler::new(
            VertexEmbeddingModel::TextEmbedding004,
            "test".to_string(),
            "us-central1".to_string(),
        );

        let valid_request = GeminiEmbeddingRequest {
            model: "text-embedding-004".to_string(),
            content: ContentInput::Text("Hello world".to_string()),
            task_type: Some(TaskType::RetrievalDocument),
            title: None,
            output_dimensionality: Some(512),
        };

        assert!(handler.validate_request(&valid_request).is_ok());

        let invalid_request = GeminiEmbeddingRequest {
            output_dimensionality: Some(2000), // Too large
            ..valid_request
        };

        assert!(handler.validate_request(&invalid_request).is_err());
    }
}
