//! Embeddings Module for Bedrock
//!
//! Handles text and multimodal embeddings for Titan and Cohere models

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::responses::{EmbeddingData, EmbeddingResponse, Usage};
use serde::{Deserialize, Serialize};

/// Titan embedding request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitanEmbeddingRequest {
    pub input_text: String,
}

/// Titan embedding response
#[derive(Debug, Deserialize)]
pub struct TitanEmbeddingResponse {
    pub embedding: Vec<f32>,
    #[serde(rename = "inputTextTokenCount")]
    pub input_text_token_count: Option<u32>,
}

/// Titan multimodal embedding request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitanMultimodalEmbeddingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_config: Option<EmbeddingConfig>,
}

/// Embedding configuration
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingConfig {
    pub output_embedding_length: u32,
}

/// Cohere embedding request
#[derive(Debug, Serialize)]
pub struct CohereEmbeddingRequest {
    pub texts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate: Option<String>,
}

/// Cohere embedding response
#[derive(Debug, Deserialize)]
pub struct CohereEmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub id: String,
    pub response_type: String,
    pub texts: Vec<String>,
}

/// Execute embedding request
pub async fn execute_embedding(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, ProviderError> {
    let model = &request.model;

    if model.contains("titan-embed") {
        if model.contains("multimodal") {
            execute_titan_multimodal_embedding(client, request).await
        } else {
            execute_titan_embedding(client, request).await
        }
    } else if model.contains("cohere") && model.contains("embed") {
        execute_cohere_embedding(client, request).await
    } else {
        Err(ProviderError::model_not_found(
            "bedrock",
            format!("Embedding model {} not supported", model),
        ))
    }
}

fn execution_model_id(model: &str) -> String {
    crate::core::providers::bedrock::parse_bedrock_model_id(model).execution_model_id
}

/// Execute Titan text embedding
async fn execute_titan_embedding(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, ProviderError> {
    // Titan only supports single text input
    let input_text = match &request.input {
        crate::core::types::embedding::EmbeddingInput::Text(text) => text.clone(),
        crate::core::types::embedding::EmbeddingInput::Array(texts) => {
            if texts.is_empty() {
                return Err(ProviderError::invalid_request(
                    "bedrock",
                    "No input text provided",
                ));
            }
            texts[0].clone()
        }
    };

    let titan_request = TitanEmbeddingRequest { input_text };
    let body = serde_json::to_value(titan_request)?;

    let model_id = execution_model_id(&request.model);
    let response = client.send_request(&model_id, "invoke", &body).await?;
    let titan_response: TitanEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

    // Convert to OpenAI format
    let embedding_data = EmbeddingData {
        index: 0,
        embedding: titan_response.embedding,
        object: "embedding".to_string(),
    };

    let usage = titan_response.input_text_token_count.map(|tokens| Usage {
        prompt_tokens: tokens,
        completion_tokens: 0,
        total_tokens: tokens,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
        cost: None,
    });

    Ok(EmbeddingResponse {
        object: "list".to_string(),
        data: vec![embedding_data.clone()],
        model: request.model.clone(),
        usage,
        embeddings: Some(vec![embedding_data]),
    })
}

/// Execute Titan multimodal embedding
async fn execute_titan_multimodal_embedding(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, ProviderError> {
    // Extract text input
    let input_text = match &request.input {
        crate::core::types::embedding::EmbeddingInput::Text(text) => Some(text.clone()),
        crate::core::types::embedding::EmbeddingInput::Array(texts) => {
            if !texts.is_empty() {
                Some(texts[0].clone())
            } else {
                None
            }
        }
    };

    let titan_request = TitanMultimodalEmbeddingRequest {
        input_text,
        input_image: None, // NOTE: image input not yet supported
        embedding_config: Some(EmbeddingConfig {
            output_embedding_length: request.dimensions.unwrap_or(1024),
        }),
    };

    let body = serde_json::to_value(titan_request)?;
    let model_id = execution_model_id(&request.model);
    let response = client.send_request(&model_id, "invoke", &body).await?;
    let titan_response: TitanEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

    // Convert to OpenAI format
    let embedding_data = EmbeddingData {
        index: 0,
        embedding: titan_response.embedding,
        object: "embedding".to_string(),
    };

    let usage = titan_response.input_text_token_count.map(|tokens| Usage {
        prompt_tokens: tokens,
        completion_tokens: 0,
        total_tokens: tokens,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
        cost: None,
    });

    Ok(EmbeddingResponse {
        object: "list".to_string(),
        data: vec![embedding_data.clone()],
        model: request.model.clone(),
        usage,
        embeddings: Some(vec![embedding_data]),
    })
}

/// Execute Cohere embedding
async fn execute_cohere_embedding(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &EmbeddingRequest,
) -> Result<EmbeddingResponse, ProviderError> {
    // Convert input to text array
    let texts = match &request.input {
        crate::core::types::embedding::EmbeddingInput::Text(text) => vec![text.clone()],
        crate::core::types::embedding::EmbeddingInput::Array(texts) => texts.clone(),
    };

    let cohere_request = CohereEmbeddingRequest {
        texts,
        input_type: Some("search_document".to_string()),
        truncate: Some("END".to_string()),
    };

    let body = serde_json::to_value(cohere_request)?;
    let model_id = execution_model_id(&request.model);
    let response = client.send_request(&model_id, "invoke", &body).await?;
    let cohere_response: CohereEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

    // Convert to OpenAI format
    let data: Vec<EmbeddingData> = cohere_response
        .embeddings
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| EmbeddingData {
            index: index as u32,
            embedding,
            object: "embedding".to_string(),
        })
        .collect();

    Ok(EmbeddingResponse {
        object: "list".to_string(),
        data: data.clone(),
        model: request.model.clone(),
        usage: None, // Cohere doesn't provide usage info
        embeddings: Some(data),
    })
}

#[cfg(test)]
mod tests {
    use super::execution_model_id;

    #[test]
    fn strips_bedrock_selector_for_embedding_execution() {
        assert_eq!(
            execution_model_id("bedrock/amazon.titan-embed-text-v2:0"),
            "amazon.titan-embed-text-v2:0"
        );
    }

    #[test]
    fn preserves_inference_profile_execution_id_for_embeddings() {
        assert_eq!(
            execution_model_id("bedrock/us.amazon.titan-embed-text-v2:0"),
            "us.amazon.titan-embed-text-v2:0"
        );
    }
}
