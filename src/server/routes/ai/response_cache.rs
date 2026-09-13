//! Shared response-cache helpers for non-streaming AI routes.

use crate::core::cache::LLMCache;
use crate::core::models::openai::{
    ChatCompletionRequest, ChatCompletionResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::core::providers::ProviderError;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingInput;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::Arc;
use tracing::{error, warn};

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

/// Look up a cached chat response, returning the cache instance from the same
/// runtime generation that performed the lookup.
///
/// Callers that later invalidate after an output-guardrail block must use the
/// returned `Arc<LLMCache>` rather than re-reading `AppState::response_cache()`,
/// which can point at a newer revision if caching was disabled mid-request.
pub(super) async fn lookup_chat(
    state: &AppState,
    request: &ChatCompletionRequest,
    context: &RequestContext,
) -> Result<(Option<Arc<LLMCache>>, Option<ChatCompletionResponse>), GatewayError> {
    if should_bypass_chat_cache(request, context) {
        return Ok((None, None));
    }
    let Some(cache) = state.response_cache() else {
        return Ok((None, None));
    };
    let identity = cache_identity(context);
    match cache
        .get_chat_response_with_user(request, identity.as_deref())
        .await
    {
        Ok(cached) => Ok((
            Some(cache),
            cached.map(|response| response.as_ref().clone()),
        )),
        Err(error) => {
            warn!(error = %error, "Chat response cache lookup failed; treating as miss");
            Ok((Some(cache), None))
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
    let Some(cache) = state.response_cache() else {
        return Ok(());
    };
    let identity = cache_identity(context);
    cache
        .cache_chat_response_with_user(request, response.clone(), identity.as_deref())
        .await
}

/// Drop a chat response-cache entry so a later identical request re-fetches.
///
/// Used when an output guardrail blocks a cached replay: the entry may have
/// been lawful when stored but is poisoned after a guardrail config change.
///
/// `cache` must be the same instance returned by [`lookup_chat`] for this
/// request so invalidation still hits Redis/L1 after a mid-request reload that
/// disables the live `AppState::response_cache()`.
///
/// Returns `Err` when Redis/L2 deletion fails so callers can surface the
/// failure (and avoid treating a still-poisoned entry as cleared). Transient
/// Dual-mode L2 errors are retried briefly before propagating.
pub(super) async fn invalidate_chat(
    cache: &LLMCache,
    request: &ChatCompletionRequest,
    context: &RequestContext,
) -> Result<(), GatewayError> {
    if should_bypass_chat_cache(request, context) {
        return Ok(());
    }
    let identity = cache_identity(context);
    let mut last_error = None;
    for attempt in 1..=3u8 {
        match cache
            .invalidate_chat_with_user(request, identity.as_deref())
            .await
        {
            Ok(_) => return Ok(()),
            Err(error) => {
                warn!(
                    attempt,
                    error = %error,
                    "Chat response cache invalidate failed"
                );
                last_error = Some(error);
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(25 * u64::from(attempt)))
                        .await;
                }
            }
        }
    }
    let error = last_error.expect("invalidate retries always record an error");
    error!(error = %error, "Chat response cache invalidate exhausted retries");
    Err(GatewayError::Internal(format!(
        "Chat response cache invalidate failed: {error}"
    )))
}

pub(super) async fn lookup_embedding(
    state: &AppState,
    request: &EmbeddingRequest,
    context: &RequestContext,
) -> Result<Option<EmbeddingResponse>, GatewayError> {
    let Some(cache) = state.response_cache() else {
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
    let Some(cache) = state.response_cache() else {
        return Ok(());
    };
    let request = embedding_request_for_cache(request, context);
    cache
        .cache_embedding_response(&request, response.clone())
        .await
}

pub(super) fn ensure_chat_cache_pricing_for_attempt(
    request_pricing: &super::spend::RequestPricing,
    request: &ChatCompletionRequest,
    provider: &str,
    model: &str,
) -> Result<(), ProviderError> {
    let prompt_tokens = super::spend::try_estimate_chat_prompt_tokens(
        request_pricing.token_identity(),
        &request.messages,
        request.tools.as_deref(),
        request.functions.as_deref(),
        request.function_call.as_ref(),
        request.response_format.as_ref(),
    )
    .map_err(|error| {
        super::spend::token_count_error(provider, model, request_pricing.token_identity(), error)
    })?;
    let output_tokens = request
        .max_completion_tokens
        .or(request.max_tokens)
        .or(Some(1));
    request_pricing
        .estimate_completion(prompt_tokens, output_tokens)
        .map(|_| ())
        .map_err(|error| super::spend::model_not_priced_error(provider, model, error))
}

pub(super) fn ensure_embedding_cache_pricing_for_attempt(
    request_pricing: &super::spend::RequestPricing,
    input: &EmbeddingInput,
    provider: &str,
    model: &str,
) -> Result<(), ProviderError> {
    let prompt_tokens =
        super::spend::estimate_embedding_input_tokens(request_pricing.token_identity(), input)
            .map_err(|error| {
                super::spend::token_count_error(
                    provider,
                    model,
                    request_pricing.token_identity(),
                    error,
                )
            })?;
    request_pricing
        .estimate_completion(prompt_tokens, Some(0))
        .map(|_| ())
        .map_err(|error| super::spend::model_not_priced_error(provider, model, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::pricing_service::{LiteLLMModelInfo, PricingService};
    use crate::server::routes::ai::spend::RequestPricing;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn priced_model(provider: &str) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(1024),
            input_cost_per_token: Some(0.01),
            output_cost_per_token: Some(0.02),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        }
    }

    #[test]
    fn cache_gate_cannot_substitute_a_priced_sibling_for_the_selected_attempt() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model(
            "priced-sibling".to_string(),
            priced_model("review-provider"),
        );
        let selected =
            RequestPricing::from_exact(&pricing, "review-provider", "selected-explicitly-unpriced");
        let request = ChatCompletionRequest {
            model: "public-route".to_string(),
            ..Default::default()
        };

        let error = ensure_chat_cache_pricing_for_attempt(
            &selected,
            &request,
            "selected-provider",
            "selected-wire-model",
        )
        .expect_err("the selected attempt must not borrow a priced sibling identity");

        assert!(super::super::spend::is_model_not_priced_error(&error));
    }

    #[test]
    fn embedding_cache_gate_fails_for_selected_exact_identity_without_tokenizer() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model("gpt-audio-1.5".to_string(), priced_model("openai"));
        let selected = RequestPricing::from_exact(&pricing, "openai", "gpt-audio-1.5");

        let error = ensure_embedding_cache_pricing_for_attempt(
            &selected,
            &EmbeddingInput::Text("hello".to_string()),
            "selected-openai",
            "wire-audio-deployment",
        )
        .expect_err("cache hits must enforce the selected attempt's token identity");

        assert!(matches!(
            error,
            ProviderError::InvalidRequest {
                provider: "token_count",
                ..
            }
        ));
    }

    #[test]
    fn chat_cache_gate_fails_for_selected_exact_identity_without_tokenizer() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model("gpt-audio-1.5".to_string(), priced_model("openai"));
        let selected = RequestPricing::from_exact(&pricing, "openai", "gpt-audio-1.5");
        let request = ChatCompletionRequest {
            model: "public-alias".to_string(),
            messages: vec![crate::core::models::openai::ChatMessage {
                role: crate::core::models::openai::MessageRole::User,
                content: Some(crate::core::models::openai::MessageContent::Text(
                    "hello".to_string(),
                )),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            ..Default::default()
        };

        let error = ensure_chat_cache_pricing_for_attempt(
            &selected,
            &request,
            "selected-openai",
            "wire-audio-deployment",
        )
        .expect_err("cache hit must use the selected attempt's exact token identity");

        assert!(matches!(
            error,
            ProviderError::InvalidRequest {
                provider: "token_count",
                ..
            }
        ));
    }

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
            encoding_format: None,
            dimensions: None,
            input_type: None,
            truncation: None,
        };
        let context = RequestContext::default().with_api_key(api_key_id);

        let cache_request = embedding_request_for_cache(&request, &context);

        assert_eq!(
            cache_request.user.as_deref(),
            Some("api_key:00000000-0000-0000-0000-00000000002a")
        );
    }

    #[tokio::test]
    async fn invalidate_chat_removes_entry_for_next_lookup() {
        use crate::core::models::openai::{ChatChoice, ChatMessage, MessageContent, MessageRole};

        let mut config = crate::server::valid_test_config();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.cache.enabled = true;

        let state = crate::server::HttpServer::new(&config)
            .await
            .expect("gateway should initialize with response cache")
            .state()
            .clone();
        assert!(
            state.response_cache().is_some(),
            "response cache must be enabled for this regression"
        );

        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            }],
            ..Default::default()
        };
        let response = ChatCompletionResponse {
            id: "chatcmpl-poisoned".to_string(),
            object: "chat.completion".to_string(),
            created: 1,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("blocked later".to_string())),
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                    audio: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };
        let context = RequestContext::default().with_api_key(Uuid::from_u128(99));

        store_chat(&state, &request, &response, &context)
            .await
            .expect("store should succeed");
        let (lookup_cache, before) = lookup_chat(&state, &request, &context)
            .await
            .expect("lookup should succeed");
        assert!(
            before.is_some(),
            "cached entry must be present before invalidate"
        );
        let cache = lookup_cache.expect("lookup must pin the cache generation used for the hit");

        invalidate_chat(&cache, &request, &context)
            .await
            .expect("invalidate should succeed against memory-only cache");

        let (_, after) = lookup_chat(&state, &request, &context)
            .await
            .expect("lookup after invalidate should succeed");
        assert!(
            after.is_none(),
            "output-blocked cache entry must be gone for the next lookup"
        );
    }
}
