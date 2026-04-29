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

    // Keep provider selection for capability validation before execution.
    select_provider_for_model(
        unified_router,
        &request.model,
        ProviderCapability::Embeddings,
    )?;

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
        move |provider, selected_model| {
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
