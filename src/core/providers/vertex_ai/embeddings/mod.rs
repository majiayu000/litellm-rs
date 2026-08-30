//! Vertex AI Embeddings Module

use crate::ProviderError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::types::{
    embedding::EmbeddingInput,
    embedding::EmbeddingRequest,
    responses::{EmbeddingData, EmbeddingResponse},
};

/// Vertex AI embedding models
#[derive(Debug, Clone)]
pub enum VertexEmbeddingModel {
    /// Text Embedding 004 - Latest multilingual model
    TextEmbedding004,
    /// Text Embedding Preview - Latest preview model
    TextEmbeddingPreview0409,
    /// Multilingual Embedding 002 - Multilingual support
    TextMultilingualEmbedding002,
    /// Multimodal Embedding - Text and image embeddings
    MultimodalEmbedding,
    /// Legacy Gecko models
    TextEmbeddingGecko,
    TextEmbeddingGecko003,
    TextEmbeddingGeckoMultilingual,
    /// Custom model
    Custom(String),
}

impl VertexEmbeddingModel {
    /// Get the model ID for API calls
    pub fn model_id(&self) -> String {
        match self {
            Self::TextEmbedding004 => "text-embedding-004".to_string(),
            Self::TextEmbeddingPreview0409 => "text-embedding-preview-0409".to_string(),
            Self::TextMultilingualEmbedding002 => "text-multilingual-embedding-002".to_string(),
            Self::MultimodalEmbedding => "multimodalembedding".to_string(),
            Self::TextEmbeddingGecko => "textembedding-gecko".to_string(),
            Self::TextEmbeddingGecko003 => "textembedding-gecko@003".to_string(),
            Self::TextEmbeddingGeckoMultilingual => "textembedding-gecko-multilingual".to_string(),
            Self::Custom(id) => id.clone(),
        }
    }

    /// Get maximum input length
    pub fn max_input_length(&self) -> usize {
        match self {
            Self::TextEmbedding004 => 3072,
            Self::TextEmbeddingPreview0409 => 3072,
            Self::TextMultilingualEmbedding002 => 2048,
            Self::MultimodalEmbedding => 2048,
            Self::TextEmbeddingGecko
            | Self::TextEmbeddingGecko003
            | Self::TextEmbeddingGeckoMultilingual => 3072,
            Self::Custom(_) => 2048, // Default
        }
    }

    /// Get embedding dimensions
    pub fn dimensions(&self) -> usize {
        match self {
            Self::TextEmbedding004 => 768,
            Self::TextEmbeddingPreview0409 => 768,
            Self::TextMultilingualEmbedding002 => 768,
            Self::MultimodalEmbedding => 1408,
            Self::TextEmbeddingGecko
            | Self::TextEmbeddingGecko003
            | Self::TextEmbeddingGeckoMultilingual => 768,
            Self::Custom(_) => 768, // Default
        }
    }

    /// Check if model supports images
    pub fn supports_images(&self) -> bool {
        matches!(self, Self::MultimodalEmbedding)
    }

    /// Check if model supports batch processing
    pub fn supports_batch(&self) -> bool {
        matches!(
            self,
            Self::TextEmbedding004
                | Self::TextEmbeddingPreview0409
                | Self::TextMultilingualEmbedding002
        )
    }
}

/// Parse embedding model string
pub fn parse_embedding_model(model: &str) -> VertexEmbeddingModel {
    match model {
        "text-embedding-004" => VertexEmbeddingModel::TextEmbedding004,
        "text-embedding-preview-0409" => VertexEmbeddingModel::TextEmbeddingPreview0409,
        "text-multilingual-embedding-002" => VertexEmbeddingModel::TextMultilingualEmbedding002,
        "multimodalembedding" => VertexEmbeddingModel::MultimodalEmbedding,
        "textembedding-gecko" => VertexEmbeddingModel::TextEmbeddingGecko,
        "textembedding-gecko@003" => VertexEmbeddingModel::TextEmbeddingGecko003,
        "textembedding-gecko-multilingual" => VertexEmbeddingModel::TextEmbeddingGeckoMultilingual,
        _ => VertexEmbeddingModel::Custom(model.to_string()),
    }
}

/// Task types for embedding generation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum TaskType {
    /// For retrieval queries
    #[serde(rename = "RETRIEVAL_QUERY")]
    RetrievalQuery,
    /// For retrieval documents
    #[serde(rename = "RETRIEVAL_DOCUMENT")]
    #[default]
    RetrievalDocument,
    /// For semantic similarity
    #[serde(rename = "SEMANTIC_SIMILARITY")]
    SemanticSimilarity,
    /// For classification tasks
    #[serde(rename = "CLASSIFICATION")]
    Classification,
    /// For clustering tasks
    #[serde(rename = "CLUSTERING")]
    Clustering,
    /// For question answering
    #[serde(rename = "QUESTION_ANSWERING")]
    QuestionAnswering,
    /// For fact verification
    #[serde(rename = "FACT_VERIFICATION")]
    FactVerification,
}

/// Embedding instance for Vertex AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingInstance {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<TaskType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Multimodal embedding instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalEmbeddingInstance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoData>,
}

/// Image data for multimodal embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes_base64_encoded: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcs_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Video data for multimodal embeddings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcs_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_offset_sec: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_offset_sec: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_sec: Option<f32>,
}

/// Embedding parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_truncate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimensionality: Option<i32>,
}

/// Embedding handler
pub struct EmbeddingHandler {
    model: VertexEmbeddingModel,
}

impl EmbeddingHandler {
    /// Create new embedding handler
    pub fn new(model: VertexEmbeddingModel) -> Self {
        Self { model }
    }

    /// Transform embedding request to Vertex AI format
    pub fn transform_request(&self, request: &EmbeddingRequest) -> Result<Value, ProviderError> {
        // Convert EmbeddingInput to Vec<String>
        let input_strings = match &request.input {
            EmbeddingInput::Text(text) => vec![text.clone()],
            EmbeddingInput::Array(texts) => texts.clone(),
        };

        let instances = if self.model.supports_images() {
            // For multimodal embeddings
            self.create_multimodal_instances(&input_strings)?
        } else {
            // For text embeddings
            self.create_text_instances(&input_strings, request.task_type.as_deref())?
        };

        let mut body = json!({
            "instances": instances
        });

        // Add parameters if specified
        let parameters = EmbeddingParameters {
            auto_truncate: Some(true),
            output_dimensionality: request.dimensions.map(|d| d as i32),
        };

        body["parameters"] = serde_json::to_value(parameters)?;

        Ok(body)
    }

    /// Create text embedding instances
    fn create_text_instances(
        &self,
        inputs: &[String],
        task_type: Option<&str>,
    ) -> Result<Vec<Value>, ProviderError> {
        let task = task_type
            .and_then(|t| self.parse_task_type(t))
            .unwrap_or_default();

        let instances = inputs
            .iter()
            .map(|content| {
                let instance = EmbeddingInstance {
                    content: content.clone(),
                    task_type: Some(task.clone()),
                    title: None,
                };
                serde_json::to_value(instance).unwrap_or_default()
            })
            .collect();

        Ok(instances)
    }

    /// Create multimodal embedding instances
    fn create_multimodal_instances(&self, inputs: &[String]) -> Result<Vec<Value>, ProviderError> {
        let instances = inputs
            .iter()
            .map(|content| {
                // Check if input is a data URL or GCS URI
                if content.starts_with("data:image/") {
                    // Base64 image
                    let parts: Vec<&str> = content.splitn(2, ',').collect();
                    if parts.len() == 2 {
                        let mime_type = parts[0]
                            .strip_prefix("data:")
                            .and_then(|s| s.strip_suffix(";base64"))
                            .unwrap_or("image/jpeg")
                            .to_string();

                        MultimodalEmbeddingInstance {
                            text: None,
                            image: Some(ImageData {
                                bytes_base64_encoded: Some(parts[1].to_string()),
                                gcs_uri: None,
                                mime_type: Some(mime_type),
                            }),
                            video: None,
                        }
                    } else {
                        MultimodalEmbeddingInstance {
                            text: Some(content.clone()),
                            image: None,
                            video: None,
                        }
                    }
                } else if content.starts_with("gs://") {
                    // GCS URI - could be image or video
                    if content.contains(".mp4")
                        || content.contains(".avi")
                        || content.contains(".mov")
                    {
                        MultimodalEmbeddingInstance {
                            text: None,
                            image: None,
                            video: Some(VideoData {
                                gcs_uri: Some(content.clone()),
                                start_offset_sec: None,
                                end_offset_sec: None,
                                interval_sec: None,
                            }),
                        }
                    } else {
                        MultimodalEmbeddingInstance {
                            text: None,
                            image: Some(ImageData {
                                bytes_base64_encoded: None,
                                gcs_uri: Some(content.clone()),
                                mime_type: None,
                            }),
                            video: None,
                        }
                    }
                } else {
                    // Regular text
                    MultimodalEmbeddingInstance {
                        text: Some(content.clone()),
                        image: None,
                        video: None,
                    }
                }
            })
            .map(|instance| serde_json::to_value(instance).unwrap_or_default())
            .collect();

        Ok(instances)
    }

    /// Parse task type from string
    fn parse_task_type(&self, task_type: &str) -> Option<TaskType> {
        match task_type.to_uppercase().as_str() {
            "RETRIEVAL_QUERY" => Some(TaskType::RetrievalQuery),
            "RETRIEVAL_DOCUMENT" => Some(TaskType::RetrievalDocument),
            "SEMANTIC_SIMILARITY" => Some(TaskType::SemanticSimilarity),
            "CLASSIFICATION" => Some(TaskType::Classification),
            "CLUSTERING" => Some(TaskType::Clustering),
            "QUESTION_ANSWERING" => Some(TaskType::QuestionAnswering),
            "FACT_VERIFICATION" => Some(TaskType::FactVerification),
            _ => None,
        }
    }

    /// Transform Vertex AI response to standard format
    pub fn transform_response(&self, response: Value) -> Result<EmbeddingResponse, ProviderError> {
        let predictions = response["predictions"].as_array().ok_or_else(|| {
            ProviderError::response_parsing(
                "vertex_ai",
                "Missing predictions in embedding response",
            )
        })?;

        let mut embeddings = Vec::new();

        for prediction in predictions {
            let values =
                if let Some(embedding_values) = prediction["embeddings"]["values"].as_array() {
                    // Standard embedding format
                    embedding_values
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                } else if let Some(values) = prediction["values"].as_array() {
                    // Alternative format
                    values
                        .iter()
                        .filter_map(|v| v.as_f64().map(|f| f as f32))
                        .collect()
                } else {
                    return Err(ProviderError::response_parsing(
                        "vertex_ai",
                        "Missing embedding values",
                    ));
                };

            embeddings.push(values);
        }

        let embedding_data: Vec<EmbeddingData> = embeddings
            .into_iter()
            .enumerate()
            .map(|(index, embedding)| EmbeddingData {
                object: "embedding".to_string(),
                embedding,
                index: index as u32,
            })
            .collect();

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: embedding_data.clone(),
            embeddings: Some(embedding_data),
            model: self.model.model_id(),
            usage: None, // Vertex AI doesn't return token usage for embeddings
        })
    }
}

/// Batch embedding handler for processing large numbers of texts
pub struct BatchEmbeddingHandler {
    model: VertexEmbeddingModel,
    batch_size: usize,
}

impl BatchEmbeddingHandler {
    /// Create new batch embedding handler
    pub fn new(model: VertexEmbeddingModel, batch_size: usize) -> Self {
        Self { model, batch_size }
    }

    /// Process embeddings in batches
    pub async fn process_batch(
        &self,
        inputs: Vec<String>,
        _task_type: Option<String>,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        if !self.model.supports_batch() {
            return Err(ProviderError::not_supported(
                "vertex_ai",
                format!(
                    "Model {} does not support batch processing",
                    self.model.model_id()
                ),
            ));
        }

        let mut all_embeddings = Vec::new();

        // Process in batches
        for chunk in inputs.chunks(self.batch_size) {
            let request = EmbeddingRequest {
                model: self.model.model_id(),
                input: crate::core::types::embedding::EmbeddingInput::Array(chunk.to_vec()),
                encoding_format: None,
                dimensions: None,
                user: None,
                task_type: Some("RETRIEVAL_DOCUMENT".to_string()), // Default
                truncation: None,
            };

            let handler = EmbeddingHandler::new(self.model.clone());
            let _vertex_request = handler.transform_request(&request)?;

            // NOTE: actual API call not yet implemented
            // For now, return dummy embeddings
            for _ in chunk {
                all_embeddings.push(vec![0.0; self.model.dimensions()]);
            }
        }

        Ok(all_embeddings)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
