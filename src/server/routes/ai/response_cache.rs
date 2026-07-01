//! Shared response-cache helpers for non-streaming AI routes.

use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::core::types::context::RequestContext;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;

const BYPASS_CHAT_RESPONSE_CACHE_KEY: &str = "bypass_chat_response_cache";

pub(super) fn bypass_chat_response_cache(context: &mut RequestContext) {
    context.metadata.insert(
        BYPASS_CHAT_RESPONSE_CACHE_KEY.to_string(),
        serde_json::json!(true),
    );
}

fn should_bypass_chat_cache(context: &RequestContext) -> bool {
    context
        .metadata
        .get(BYPASS_CHAT_RESPONSE_CACHE_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn cache_identity(context: &RequestContext) -> Option<String> {
    let identity = context
        .api_key_id()
        .map(|id| format!("api_key:{id}"))
        .or_else(|| context.user_id.as_ref().map(|id| format!("user:{id}")))?;

    match context.api_key_max_tokens_per_request() {
        Some(limit) => Some(format!("{identity}:max_tokens_per_request:{limit}")),
        None => Some(identity),
    }
}

pub(super) async fn lookup_chat(
    state: &AppState,
    request: &ChatCompletionRequest,
    context: &RequestContext,
) -> Result<Option<ChatCompletionResponse>, GatewayError> {
    if should_bypass_chat_cache(context) {
        return Ok(None);
    }
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
    if should_bypass_chat_cache(context) {
        return Ok(());
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn chat_cache_identity_includes_api_key_token_cap() {
        let api_key_id = Uuid::from_u128(42);
        let mut uncapped = RequestContext::default().with_api_key(api_key_id);
        let mut capped = uncapped.clone();
        capped.set_api_key_max_tokens_per_request(128);

        assert_eq!(
            cache_identity(&uncapped).as_deref(),
            Some("api_key:00000000-0000-0000-0000-00000000002a")
        );
        assert_eq!(
            cache_identity(&capped).as_deref(),
            Some("api_key:00000000-0000-0000-0000-00000000002a:max_tokens_per_request:128")
        );

        uncapped.set_api_key_max_tokens_per_request(64);
        assert_ne!(cache_identity(&uncapped), cache_identity(&capped));
    }

    #[test]
    fn chat_cache_bypass_flag_is_context_scoped() {
        let mut context = RequestContext::default();
        assert!(!should_bypass_chat_cache(&context));

        bypass_chat_response_cache(&mut context);
        assert!(should_bypass_chat_cache(&context));
    }
}
