//! Chat Completion Module for Bedrock
//!
//! Handles both invoke and converse APIs for chat completions

pub mod converse;
pub mod invoke;
pub mod transformations;

use super::get_model_config_for_model_id;
use super::model_config::BedrockApiType;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::Value;

/// Route a chat request to the appropriate API based on model capabilities
pub async fn route_chat_request(
    client: &super::client::BedrockClient,
    request: &ChatRequest,
) -> Result<Value, ProviderError> {
    let model_config = get_model_config_for_model_id(&request.model)?;

    match model_config.api_type {
        BedrockApiType::Converse | BedrockApiType::ConverseStream => {
            converse::execute_converse(client, request).await
        }
        BedrockApiType::Invoke | BedrockApiType::InvokeStream => {
            invoke::execute_invoke(client, request).await
        }
    }
}

/// Check if a model supports the converse API
pub fn supports_converse(model_id: &str) -> bool {
    if let Ok(config) = get_model_config_for_model_id(model_id) {
        matches!(
            config.api_type,
            BedrockApiType::Converse | BedrockApiType::ConverseStream
        )
    } else {
        false
    }
}

/// Check if a model supports streaming
pub fn supports_streaming(model_id: &str) -> bool {
    if let Ok(config) = get_model_config_for_model_id(model_id) {
        config.supports_streaming
    } else {
        false
    }
}
