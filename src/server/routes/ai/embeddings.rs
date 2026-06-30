//! Embeddings endpoint

use crate::core::models::openai::{EmbeddingRequest, EmbeddingResponse};
use crate::core::types::{
    context::RequestContext, embedding::EmbeddingInput,
    embedding::EmbeddingRequest as CoreEmbeddingRequest, model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::info;

use super::context::handle_ai_request;
use super::execution::execute_with_selected_deployment;

fn parse_embedding_input(input: &serde_json::Value) -> Result<EmbeddingInput, GatewayError> {
    match input {
        serde_json::Value::String(s) => Ok(EmbeddingInput::Text(s.clone())),
        serde_json::Value::Array(arr) => {
            let mut texts = Vec::with_capacity(arr.len());
            for (index, value) in arr.iter().enumerate() {
                let Some(text) = value.as_str() else {
                    return Err(GatewayError::validation(format!(
                        "Invalid input: array element at index {} must be a string, got {}",
                        index,
                        json_value_type(value)
                    )));
                };
                texts.push(text.to_string());
            }
            Ok(EmbeddingInput::Array(texts))
        }
        _ => Err(GatewayError::validation(
            "Invalid input: expected string or array of strings",
        )),
    }
}

fn json_value_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Embeddings endpoint
///
/// OpenAI-compatible embeddings API for generating text embeddings.
pub async fn embeddings(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<EmbeddingRequest>,
) -> ActixResult<HttpResponse> {
    info!("Embedding request for model: {}", request.model);

    handle_ai_request(
        &req,
        request.into_inner(),
        "Embedding",
        |request, context| handle_embedding_with_state(state.get_ref(), request, context),
    )
    .await
}

/// Handle embedding with app state (UnifiedRouter only)
pub async fn handle_embedding_with_state(
    state: &AppState,
    request: EmbeddingRequest,
    context: RequestContext,
) -> Result<EmbeddingResponse, GatewayError> {
    let unified_router = &state.unified_router;
    handle_embedding_internal(unified_router, request, context).await
}

async fn handle_embedding_internal(
    unified_router: &crate::core::router::UnifiedRouter,
    request: EmbeddingRequest,
    context: RequestContext,
) -> Result<EmbeddingResponse, GatewayError> {
    // Convert OpenAI format request to core format.
    let input = parse_embedding_input(&request.input)?;

    if request.model.trim().is_empty() {
        return Err(GatewayError::validation("Model is required"));
    }

    let requested_model = request.model.clone();
    let core_request = CoreEmbeddingRequest {
        model: requested_model,
        input,
        user: request.user,
        encoding_format: None,
        dimensions: None,
        task_type: None,
    };

    let requested_model = core_request.model.clone();
    let context_for_execution = context.clone();
    let core_response = execute_with_selected_deployment(
        unified_router,
        &requested_model,
        ProviderCapability::Embeddings,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            async move {
                let mut request_for_provider = core_request.clone();
                request_for_provider.model = selected_model;
                let response = provider
                    .create_embeddings(request_for_provider, context)
                    .await?;
                let tokens = response
                    .usage
                    .as_ref()
                    .map(|usage| u64::from(usage.total_tokens))
                    .unwrap_or_default();
                Ok((response, tokens))
            }
        },
    )
    .await?;

    // Convert core response to OpenAI format
    let response = EmbeddingResponse {
        object: core_response.object,
        data: core_response
            .data
            .into_iter()
            .map(|d| crate::core::models::openai::EmbeddingObject {
                object: d.object,
                embedding: d.embedding.into_iter().map(|f| f as f64).collect(),
                index: d.index,
            })
            .collect(),
        model: core_response.model,
        usage: crate::core::models::openai::EmbeddingUsage {
            prompt_tokens: core_response
                .usage
                .as_ref()
                .map(|u| u.prompt_tokens)
                .unwrap_or(0),
            total_tokens: core_response
                .usage
                .as_ref()
                .map(|u| u.total_tokens)
                .unwrap_or(0),
        },
    };

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_embedding_input_accepts_string() {
        let input = parse_embedding_input(&serde_json::json!("hello")).unwrap();

        match input {
            EmbeddingInput::Text(text) => assert_eq!(text, "hello"),
            EmbeddingInput::Array(_) => panic!("expected text embedding input"),
        }
    }

    #[test]
    fn parse_embedding_input_preserves_string_array() {
        let input = parse_embedding_input(&serde_json::json!(["a", "b"])).unwrap();

        match input {
            EmbeddingInput::Array(texts) => assert_eq!(texts, vec!["a", "b"]),
            EmbeddingInput::Text(_) => panic!("expected array embedding input"),
        }
    }

    #[test]
    fn parse_embedding_input_rejects_non_string_array_item() {
        let error = parse_embedding_input(&serde_json::json!(["a", 123])).unwrap_err();

        match error {
            GatewayError::Validation(message) => {
                assert!(message.contains("index 1"));
                assert!(message.contains("number"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn parse_embedding_input_rejects_object() {
        let error = parse_embedding_input(&serde_json::json!({ "text": "hello" })).unwrap_err();

        match error {
            GatewayError::Validation(message) => {
                assert_eq!(
                    message,
                    "Invalid input: expected string or array of strings"
                );
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
}
