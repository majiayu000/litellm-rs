//! Embeddings endpoint

use crate::core::models::openai::{EmbeddingRequest, EmbeddingResponse};
use crate::core::providers::ProviderRegistry;
use crate::core::types::{
    context::RequestContext, embedding::EmbeddingInput,
    embedding::EmbeddingRequest as CoreEmbeddingRequest, model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use tracing::info;

use super::context::handle_ai_request;
use super::provider_selection::select_provider_for_model;

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
        |request, context| handle_embedding_via_pool(&state.router, request, context),
    )
    .await
}

/// Handle embedding via provider pool
pub async fn handle_embedding_via_pool(
    pool: &ProviderRegistry,
    request: EmbeddingRequest,
    context: RequestContext,
) -> Result<EmbeddingResponse, GatewayError> {
    // Convert OpenAI format request to core format
    let input = match &request.input {
        serde_json::Value::String(s) => EmbeddingInput::Text(s.clone()),
        serde_json::Value::Array(arr) => {
            let texts: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            EmbeddingInput::Array(texts)
        }
        _ => {
            return Err(GatewayError::validation(
                "Invalid input: expected string or array of strings",
            ));
        }
    };

    let selection =
        select_provider_for_model(pool, &request.model, ProviderCapability::Embeddings)?;

    let core_request = CoreEmbeddingRequest {
        model: selection.model,
        input,
        user: request.user,
        encoding_format: None,
        dimensions: None,
        task_type: None,
    };

    let core_response = selection
        .provider
        .create_embeddings(core_request, context)
        .await
        .map_err(|e| GatewayError::internal(format!("Embedding error: {}", e)))?;

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
