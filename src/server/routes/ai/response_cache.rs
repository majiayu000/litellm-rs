//! Shared response-cache helpers for non-streaming AI routes.

use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::core::types::context::RequestContext;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

fn cache_identity(context: &RequestContext) -> Option<String> {
    context
        .api_key_id()
        .map(|id| format!("api_key:{id}"))
        .or_else(|| context.user_id.as_ref().map(|id| format!("user:{id}")))
}

pub(super) async fn lookup_chat(
    state: &AppState,
    request: &ChatCompletionRequest,
    context: &RequestContext,
) -> Result<Option<ChatCompletionResponse>, GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(None);
    };
    let identity = cache_identity(context);
    Ok(cache
        .get_chat_response_with_user(request, identity.as_deref())
        .await?
        .map(|response| response.as_ref().clone()))
}

pub(super) async fn store_chat(
    state: &AppState,
    request: &ChatCompletionRequest,
    response: &ChatCompletionResponse,
    context: &RequestContext,
) -> Result<(), GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(());
    };
    let identity = cache_identity(context);
    cache
        .cache_chat_response_with_user(request, response.clone(), identity.as_deref())
        .await
}

pub(super) async fn lookup_embedding(
    state: &AppState,
    request: &EmbeddingRequest,
) -> Result<Option<EmbeddingResponse>, GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(None);
    };
    Ok(cache
        .get_embedding_response(request)
        .await?
        .map(|response| response.as_ref().clone()))
}

pub(super) async fn store_embedding(
    state: &AppState,
    request: &EmbeddingRequest,
    response: &EmbeddingResponse,
) -> Result<(), GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(());
    };
    cache
        .cache_embedding_response(request, response.clone())
        .await
}
