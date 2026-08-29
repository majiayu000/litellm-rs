//! Shared response-cache helpers for non-streaming AI routes.

use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::core::pricing_service::PricingUsage;
use crate::core::providers::{Provider, ProviderError};
use crate::core::router::execution::router_error_to_provider_error;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingInput;
use crate::core::types::model::ProviderCapability;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use std::collections::HashSet;
use tracing::warn;

const BYPASS_CHAT_RESPONSE_CACHE_KEY: &str = "bypass_chat_response_cache";

pub(super) fn bypass_chat_response_cache(context: &mut RequestContext) {
    context.metadata.insert(
        BYPASS_CHAT_RESPONSE_CACHE_KEY.to_string(),
        serde_json::json!(true),
    );
}

fn should_bypass_chat_cache(request: &ChatCompletionRequest, context: &RequestContext) -> bool {
    context
        .metadata
        .get(BYPASS_CHAT_RESPONSE_CACHE_KEY)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
        || context.api_key_budget_id().is_some()
        || request.store == Some(true)
}

fn embedding_request_for_cache(
    request: &EmbeddingRequest,
    context: &RequestContext,
) -> EmbeddingRequest {
    let mut request = request.clone();
    if let Some(identity) = cache_identity(context) {
        request.user = Some(identity);
    }
    request
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
    if should_bypass_chat_cache(request, context) {
        return Ok(None);
    }
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(None);
    };
    let identity = cache_identity(context);
    match cache
        .get_chat_response_with_user(request, identity.as_deref())
        .await
    {
        Ok(cached) => Ok(cached.map(|response| response.as_ref().clone())),
        Err(error) => {
            warn!(error = %error, "Chat response cache lookup failed; treating as miss");
            Ok(None)
        }
    }
}

pub(super) async fn store_chat(
    state: &AppState,
    request: &ChatCompletionRequest,
    response: &ChatCompletionResponse,
    context: &RequestContext,
) -> Result<(), GatewayError> {
    if should_bypass_chat_cache(request, context) {
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
    context: &RequestContext,
) -> Result<Option<EmbeddingResponse>, GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(None);
    };
    let request = embedding_request_for_cache(request, context);
    match cache.get_embedding_response(&request).await {
        Ok(cached) => Ok(cached.map(|response| response.as_ref().clone())),
        Err(error) => {
            warn!(error = %error, "Embedding response cache lookup failed; treating as miss");
            Ok(None)
        }
    }
}

pub(super) async fn store_embedding(
    state: &AppState,
    request: &EmbeddingRequest,
    response: &EmbeddingResponse,
    context: &RequestContext,
) -> Result<(), GatewayError> {
    let Some(cache) = state.response_cache.as_ref() else {
        return Ok(());
    };
    let request = embedding_request_for_cache(request, context);
    cache
        .cache_embedding_response(&request, response.clone())
        .await
}

pub(super) fn ensure_chat_cache_pricing_gate(
    state: &AppState,
    request: &ChatCompletionRequest,
) -> Result<(), GatewayError> {
    let pricing = state.budgeted.pricing();
    let output_tokens = request
        .max_completion_tokens
        .or(request.max_tokens)
        .or(Some(1));
    ensure_cache_pricing_gate(
        state,
        &request.model,
        ProviderCapability::ChatCompletion,
        |provider, selected_model| {
            let (pricing_provider, pricing_model) = super::spend::pricing_identity_for_provider(
                pricing.as_ref(),
                provider,
                selected_model,
                ProviderCapability::ChatCompletion,
            );
            let token_model = super::spend::token_count_model_id(&pricing_provider, &pricing_model);
            let prompt_tokens = super::spend::try_estimate_chat_prompt_tokens(
                &token_model,
                &request.messages,
                request.tools.as_deref(),
                request.functions.as_deref(),
                request.function_call.as_ref(),
                request.response_format.as_ref(),
            )
            .map_err(|error| {
                super::spend::token_count_error(
                    provider.name(),
                    selected_model,
                    &pricing_provider,
                    &pricing_model,
                    error,
                )
            })?;
            pricing
                .estimate_loaded_completion_cost_for_provider(
                    &pricing_provider,
                    &pricing_model,
                    prompt_tokens,
                    output_tokens,
                )
                .map(|_| ())
                .map_err(|error| {
                    super::spend::model_not_priced_error(provider.name(), selected_model, error)
                })
        },
    )
}

pub(super) fn ensure_embedding_cache_pricing_gate(
    state: &AppState,
    request: &EmbeddingRequest,
    input: &EmbeddingInput,
) -> Result<(), GatewayError> {
    let pricing = state.budgeted.pricing();
    ensure_cache_pricing_gate(
        state,
        &request.model,
        ProviderCapability::Embeddings,
        |provider, selected_model| {
            let (pricing_provider, pricing_model) = super::spend::pricing_identity_for_provider(
                pricing.as_ref(),
                provider,
                selected_model,
                ProviderCapability::Embeddings,
            );
            let prompt_tokens = super::spend::estimate_embedding_input_tokens(
                &pricing_provider,
                &pricing_model,
                input,
            )
            .map_err(|error| {
                super::spend::token_count_error(
                    provider.name(),
                    selected_model,
                    &pricing_provider,
                    &pricing_model,
                    error,
                )
            })?;
            let usage = PricingUsage::new(prompt_tokens, 0);
            pricing
                .calculate_loaded_usage_cost_for_provider(&pricing_provider, &pricing_model, &usage)
                .map(|_| ())
                .map_err(|error| {
                    super::spend::model_not_priced_error(provider.name(), selected_model, error)
                })
        },
    )
}

fn ensure_cache_pricing_gate<F>(
    state: &AppState,
    requested_model: &str,
    capability: ProviderCapability,
    mut check: F,
) -> Result<(), GatewayError>
where
    F: FnMut(&Provider, &str) -> Result<(), ProviderError>,
{
    let mut excluded_deployments = HashSet::new();
    let mut last_error = None;

    loop {
        let lease = match state
            .unified_router
            .select_deployment_lease_for_capability_matching(
                requested_model,
                &capability,
                |deployment| !excluded_deployments.contains(deployment.id.as_str()),
            ) {
            Ok(lease) => lease,
            Err(router_error) => {
                if let Some(error) = last_error {
                    return Err(GatewayError::Provider(error));
                }
                return Err(GatewayError::Provider(router_error_to_provider_error(
                    router_error,
                )));
            }
        };

        let deployment = lease.deployment();
        match check(&deployment.provider, &deployment.model) {
            Ok(()) => return Ok(()),
            Err(error) if super::spend::is_model_not_priced_error(&error) => {
                super::execution::observability::record_candidate_exclusion(
                    deployment, &error, false,
                );
                excluded_deployments.insert(lease.clone_deployment_id());
                last_error = Some(error);
            }
            Err(error) => return Err(GatewayError::Provider(error)),
        }
    }
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
        let request = ChatCompletionRequest::default();
        assert!(!should_bypass_chat_cache(&request, &context));

        bypass_chat_response_cache(&mut context);
        assert!(should_bypass_chat_cache(&request, &context));
    }

    #[test]
    fn chat_cache_bypasses_api_key_budget_and_store_side_effects() {
        let mut context = RequestContext::default();
        context.set_api_key_budget_id(Uuid::from_u128(7));
        assert!(should_bypass_chat_cache(
            &ChatCompletionRequest::default(),
            &context
        ));

        let request = ChatCompletionRequest {
            store: Some(true),
            ..Default::default()
        };
        assert!(should_bypass_chat_cache(
            &request,
            &RequestContext::default()
        ));
    }

    #[test]
    fn embedding_cache_request_uses_authenticated_identity() {
        let api_key_id = Uuid::from_u128(42);
        let request = EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: serde_json::json!("hello"),
            user: Some("caller-supplied".to_string()),
        };
        let context = RequestContext::default().with_api_key(api_key_id);

        let cache_request = embedding_request_for_cache(&request, &context);

        assert_eq!(
            cache_request.user.as_deref(),
            Some("api_key:00000000-0000-0000-0000-00000000002a")
        );
    }
}
