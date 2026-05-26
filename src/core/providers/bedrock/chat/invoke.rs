//! Invoke API Implementation
//!
//! Legacy API for model-specific chat completions in Bedrock

use super::transformations;
use crate::core::providers::bedrock::parse_bedrock_model_id;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::Value;

/// Execute an invoke API request
pub async fn execute_invoke(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &ChatRequest,
) -> Result<Value, ProviderError> {
    // Get model configuration
    let model_config =
        crate::core::providers::bedrock::get_model_config_for_model_id(&request.model)?;

    // Transform request based on model family
    let body = transformations::transform_for_model(request, model_config)?;

    let execution_model_id = parse_bedrock_model_id(&request.model).execution_model_id;
    let response = client
        .send_request(&execution_model_id, "invoke", &body)
        .await?;

    // Parse response and return as Value
    response
        .json::<Value>()
        .await
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))
}
