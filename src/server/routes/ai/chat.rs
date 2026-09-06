//! Chat completions endpoint

use crate::core::models::openai::continuation::{
    ChatCompletionRequestWithExtensions, ChatCompletionResponseWithExtensions,
};
use crate::core::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ContentLogprob, Logprobs, Tool,
    ToolChoice, TopLogprob, Usage,
};
use crate::core::providers::{
    ChatContinuationRequest, ChatContinuationResponse, ChatMessageContinuation, Provider,
    ProviderError,
};
use crate::core::streaming::types::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta,
};
use crate::core::types::{
    self, chat::ChatRequest as CoreChatRequest, context::SharedRequestContext,
    model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};

use super::budgeted::ApiKeyBudgetPolicy;
use super::callbacks::CallbackLifecycle;
use super::execution::execute_with_selected_deployment_matching;
use super::openai_errors;
use super::output_fallback::{self, SelectedAttempt, is_output_guardrail_block};
#[path = "chat_delta.rs"]
mod chat_delta;
use chat_delta::{convert_function_call_delta, convert_tool_call_delta};
#[path = "chat_guardrails.rs"]
mod chat_guardrails;
use chat_guardrails::{
    continuation_after_input_projection, continuation_after_output_projection,
    guardrail_request_with_continuation, guardrail_response_with_continuation,
};
#[path = "chat_sse.rs"]
pub(super) mod chat_sse;
use chat_sse::format_sse_error;
pub(super) use chat_sse::sse_error_classification;
#[path = "chat_streaming.rs"]
mod chat_streaming;

enum ChatAttemptResponse {
    Cached(ChatCompletionResponse),
    Provider(ChatContinuationResponse),
}

/// Chat completions endpoint
///
/// OpenAI-compatible chat completions API that supports streaming and non-streaming responses.
pub async fn chat_completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ChatCompletionRequestWithExtensions>,
) -> ActixResult<HttpResponse> {
    info!(
        "Chat completion request for model: {}",
        request.legacy().model
    );

    let context = match super::token_policy::shared_request_context_with_api_key_token_limit(&req) {
        Ok(context) => context,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    if let Err(e) = RequestValidator::validate_chat_completion_request_with_extensions(
        &request.legacy().model,
        &request.legacy().messages,
        request.message_extensions(),
        request.legacy().max_tokens,
        request.legacy().temperature,
    ) {
        warn!("Invalid chat completion request: {}", e);
        return Ok(openai_errors::validation_error(e.to_string()));
    }
    if let Err(error) = super::context::enforce_api_key_model_and_token_limits(
        &req,
        &request.legacy().model,
        super::token_policy::requested_chat_output_token_limit(request.legacy()),
    ) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let typed = request.into_inner();
    let opt_in = match continuation_opt_in(&req, typed.has_continuation()) {
        Ok(opt_in) => opt_in,
        Err(error) => return Ok(openai_errors::validation_error(error)),
    };
    let (legacy, extensions) = typed.into_parts();
    let request = Arc::new(legacy);

    if request.stream.unwrap_or(false) {
        if opt_in {
            return Ok(openai_errors::validation_error(
                "Anthropic continuation streaming is tracked by #1237 and is not yet supported",
            ));
        }
        chat_streaming::handle_streaming_chat_completion(state.get_ref(), request, context).await
    } else {
        match handle_chat_completion_with_extensions(
            state.get_ref(),
            request,
            context,
            extensions,
            opt_in,
        )
        .await
        {
            Ok(response) => Ok(HttpResponse::Ok().json(response)),
            Err(e) => {
                error!("Chat completion error: {}", e);
                Ok(openai_errors::gateway_error_response(&e))
            }
        }
    }
}

pub(super) fn continuation_opt_in(
    req: &HttpRequest,
    has_carrier: bool,
) -> Result<bool, &'static str> {
    match req.headers().get("x-litellm-anthropic-continuation") {
        Some(value) if value != "v1" => Err("x-litellm-anthropic-continuation must equal v1"),
        Some(_) => Ok(true),
        None => Ok(has_carrier),
    }
}

fn continuation_budget_enabled(
    limits: &crate::core::budget::UnifiedBudgetLimits,
    provider: &str,
    model: &str,
    has_api_key_budget: bool,
) -> bool {
    has_api_key_budget
        || (limits.providers.is_enabled()
            && limits
                .providers
                .list_provider_budgets()
                .iter()
                .any(|budget| budget.enabled && budget.provider_name == provider))
        || (limits.models.is_enabled()
            && limits
                .models
                .list_model_budgets()
                .iter()
                .any(|budget| budget.enabled && budget.model_name == model))
}
pub(super) async fn handle_chat_completion_after_input_guardrail(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    let (response, callback) =
        handle_chat_completion_after_input_guardrail_deferred(state, request, context).await?;
    callback.complete_usage(response.usage.as_ref(), "success");
    Ok(response)
}
pub(super) async fn handle_chat_completion_after_input_guardrail_deferred(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
) -> Result<(ChatCompletionResponse, CallbackLifecycle), GatewayError> {
    let extensions = vec![ChatMessageContinuation::new(); request.messages.len()];
    let (response, callback) =
        handle_chat_completion_internal(state, request, context, extensions, false).await?;
    Ok((response.into_parts().0, callback))
}
pub(super) async fn handle_chat_completion_with_extensions(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
    extensions: Vec<ChatMessageContinuation>,
    opt_in: bool,
) -> Result<ChatCompletionResponseWithExtensions, GatewayError> {
    let projected = crate::server::guardrails::apply_chat_input(state, request.as_ref()).await?;
    let extensions = continuation_after_input_projection(request.as_ref(), &projected, extensions)?;
    let request = Arc::new(projected);
    let guardrail_request = guardrail_request_with_continuation(request.as_ref(), &extensions)?;
    if extensions
        .iter()
        .any(ChatMessageContinuation::has_visible_thinking)
    {
        crate::server::guardrails::ensure_chat_input_unmodified(state, &guardrail_request).await?;
    }
    handle_chat_completion_with_extensions_after_input_guardrail(
        state, request, context, extensions, opt_in,
    )
    .await
}
pub(super) async fn handle_chat_completion_with_extensions_after_input_guardrail(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
    extensions: Vec<ChatMessageContinuation>,
    opt_in: bool,
) -> Result<ChatCompletionResponseWithExtensions, GatewayError> {
    let (response, callback) =
        handle_chat_completion_internal(state, request, context, extensions, opt_in).await?;
    callback.complete_usage(response.usage(), "success");
    Ok(response)
}

pub(super) fn input_guardrail_request_with_extensions(
    request: &ChatCompletionRequest,
    extensions: &[ChatMessageContinuation],
) -> Result<ChatCompletionRequest, GatewayError> {
    guardrail_request_with_continuation(request, extensions)
}

pub(super) fn extensions_after_input_projection(
    original: &ChatCompletionRequest,
    projected: &ChatCompletionRequest,
    extensions: Vec<ChatMessageContinuation>,
) -> Result<Vec<ChatMessageContinuation>, GatewayError> {
    continuation_after_input_projection(original, projected, extensions)
}
async fn handle_chat_completion_internal(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
    extensions: Vec<ChatMessageContinuation>,
    opt_in: bool,
) -> Result<(ChatCompletionResponseWithExtensions, CallbackLifecycle), GatewayError> {
    let runtime = state.pin_runtime();
    let unified_router = runtime.unified_router.as_ref();
    let requested_model = request.model.clone();
    let core_request = ChatContinuationRequest::new(
        build_core_chat_request(request.as_ref(), requested_model, false)?,
        extensions,
    )?;
    let cached_response = if opt_in {
        None
    } else {
        super::response_cache::lookup_chat(state, request.as_ref(), context.as_ref()).await?
    };
    let requested_model = core_request.request().model.clone();
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        context.as_ref(),
    );
    let context_for_execution = Arc::clone(&context);
    let request_for_execution = Arc::clone(&request);
    let pricing_service = state.budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let key_manager = state.budgeted.key_manager();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let budget_limits = state.budgeted.budget_limits();
    let has_continuation = core_request.has_continuation();
    let callback_for_execution = callback.clone();
    let skip_cached_replay = Arc::new(AtomicBool::new(false));
    let skip_cached_replay_for_execution = Arc::clone(&skip_cached_replay);
    let operation = move |provider: Provider, selected_model: String, deployment_id: String| {
        let core_request = core_request.clone();
        let context = Arc::clone(&context_for_execution);
        let original_request = Arc::clone(&request_for_execution);
        let pricing_service = pricing_service.clone();
        let key_manager = key_manager.clone();
        let budgeted = budgeted.clone();
        let budget_limits = budget_limits.clone();
        let pricing_config = pricing_config.clone();
        let callback = callback_for_execution.clone();
        let cached_response = cached_response.clone();
        let skip_cached_replay = Arc::clone(&skip_cached_replay_for_execution);
        async move {
            let provider_name = provider.name().to_string();
            let wrap = |value: ChatAttemptResponse| SelectedAttempt {
                value,
                provider: provider_name.clone(),
                deployment_id: deployment_id.clone(),
            };
            let request_pricing = super::spend::request_pricing_for_provider(
                &pricing_service,
                &provider,
                &selected_model,
                ProviderCapability::ChatCompletion,
            )?;
            if let Some(cached) = cached_response
                && !skip_cached_replay.load(Ordering::Relaxed)
            {
                super::response_cache::ensure_chat_cache_pricing_for_attempt(
                    &request_pricing,
                    original_request.as_ref(),
                    &provider_name,
                    &selected_model,
                )?;
                return Ok((wrap(ChatAttemptResponse::Cached(cached)), 0));
            }
            if has_continuation
                && continuation_budget_enabled(
                    budget_limits.as_ref(),
                    &provider_name,
                    &selected_model,
                    api_key_budget_id.is_some(),
                )
            {
                return Err(ProviderError::not_supported(
                    "budget",
                    "Anthropic continuation cannot be metered safely while an API-key, provider, or model budget is enabled",
                ));
            }
            let (legacy_request, extensions) = core_request.into_parts();
            let request_for_provider = super::token_policy::prepare_chat_request_for_provider(
                context.api_key_max_tokens_per_request(),
                &provider_name,
                &selected_model,
                legacy_request,
            )?;
            let request_for_provider =
                ChatContinuationRequest::new(request_for_provider, extensions)?;
            let request_for_budget =
                super::spend::ChatCompletionBudgetRequest::from(original_request.as_ref())
                    .with_output_limits(
                        request_for_provider.request().max_tokens,
                        request_for_provider.request().max_completion_tokens,
                    );
            let provider_context = context.as_ref().clone();
            let settle_pricing_service = pricing_service.clone();
            let reserve_pricing_config = pricing_config.clone();
            let settle_pricing_config = pricing_config;
            let reserve_request_pricing = request_pricing.clone();
            let settle_request_pricing = request_pricing.clone();
            let settle_key_manager = key_manager.clone();
            let callback_provider = provider_name.clone();
            let callback_model = selected_model.clone();
            let callback_request_pricing = request_pricing;
            budgeted
                .for_selected_with_api_key_budget(
                    provider_name.clone(),
                    selected_model.clone(),
                    api_key_budget_id,
                    ApiKeyBudgetPolicy::FromProviderReservation,
                )
                .reserve_call_settle(
                    |budget| {
                        super::spend::reserve_chat_completion_budget_with_request_pricing(
                            &reserve_request_pricing,
                            &reserve_pricing_config,
                            budget.budget_limits(),
                            budget.provider(),
                            budget.model(),
                            request_for_budget,
                        )
                    },
                    || {
                        callback.begin_provider_execution_with_pricing(
                            callback_provider,
                            callback_model,
                            callback_request_pricing,
                        );
                        provider.chat_completion_with_continuation(
                            request_for_provider,
                            provider_context,
                            opt_in,
                        )
                    },
                    |response, reservations, budget| {
                        let (budget_reservation, key_budget_reservation) =
                            reservations.into_parts();
                        async move {
                            let tokens = response
                                .response()
                                .usage
                                .as_ref()
                                .map(|usage| u64::from(usage.total_tokens))
                                .unwrap_or_default();
                            super::spend::record_completion_spend_with_reservation_with_policy(
                                settle_pricing_service.as_ref(),
                                &settle_pricing_config,
                                super::spend::usage_spend_settlement_with_request_pricing(
                                    (budget.budget_limits(), &settle_key_manager, api_key_id),
                                    (
                                        budget.provider(),
                                        budget.model(),
                                        response.response().usage.as_ref(),
                                    ),
                                    settle_request_pricing,
                                    budget_reservation,
                                    key_budget_reservation,
                                ),
                            )
                            .await;
                            (response, tokens)
                        }
                    },
                )
                .await
                .map(|(response, tokens)| (wrap(ChatAttemptResponse::Provider(response)), tokens))
        }
    };
    let mut excluded_deployments = HashSet::new();
    let mut original_deployment = None;
    let mut last_output_block = None;
    for (attempt_idx, model) in output_fallback::models_to_try(unified_router, &requested_model)
        .into_iter()
        .enumerate()
    {
        let excluded = excluded_deployments.clone();
        let routed = match execute_with_selected_deployment_matching(
            unified_router,
            &model,
            ProviderCapability::ChatCompletion,
            move |deployment| !excluded.contains(deployment.id.as_str()),
            operation.clone(),
        )
        .await
        {
            Ok(routed) => routed,
            Err(error) => {
                if attempt_idx == 0 {
                    callback.fail(error.to_string(), "provider_error");
                    return Err(error);
                }
                continue;
            }
        };
        let binding = output_fallback::output_binding(
            original_deployment.as_deref(),
            &routed.provider,
            &routed.deployment_id,
        );
        let blocked_deployment = routed.deployment_id.clone();
        match routed.value {
            ChatAttemptResponse::Cached(cached) => {
                match crate::server::guardrails::apply_chat_output_bound(state, &cached, binding)
                    .await
                {
                    Ok(cached) => {
                        let extensions = vec![ChatMessageContinuation::new(); cached.choices.len()];
                        let response =
                            ChatCompletionResponseWithExtensions::from_parts(cached, extensions)
                                .map_err(GatewayError::internal)?;
                        return Ok((response, callback));
                    }
                    Err(error) if is_output_guardrail_block(&error) => {
                        skip_cached_replay.store(true, Ordering::Relaxed);
                        excluded_deployments.insert(blocked_deployment.clone());
                        original_deployment.get_or_insert(blocked_deployment);
                        last_output_block = Some(error);
                        continue;
                    }
                    Err(error) => {
                        callback.fail(error.to_string(), "guardrail_output");
                        return Err(error);
                    }
                }
            }
            ChatAttemptResponse::Provider(core_response) => {
                let (core_response, choice_extensions) = core_response.into_parts();
                let original_response = convert_core_chat_response(core_response);
                let response = match crate::server::guardrails::apply_chat_output_bound(
                    state,
                    &original_response,
                    binding,
                )
                .await
                {
                    Ok(response) => response,
                    Err(error) if is_output_guardrail_block(&error) => {
                        skip_cached_replay.store(true, Ordering::Relaxed);
                        excluded_deployments.insert(blocked_deployment.clone());
                        original_deployment.get_or_insert(blocked_deployment);
                        last_output_block = Some(error);
                        continue;
                    }
                    Err(error) => {
                        callback.fail(error.to_string(), "guardrail_output");
                        return Err(error);
                    }
                };
                let choice_extensions = continuation_after_output_projection(
                    &original_response,
                    &response,
                    choice_extensions,
                )?;
                let guardrail_response =
                    match guardrail_response_with_continuation(&response, &choice_extensions) {
                        Ok(projected) => projected,
                        Err(error) => {
                            callback.fail(error.to_string(), "guardrail_output_projection");
                            return Err(error);
                        }
                    };
                if choice_extensions
                    .iter()
                    .any(ChatMessageContinuation::has_visible_thinking)
                    && let Err(error) = crate::server::guardrails::ensure_chat_output_unmodified(
                        state,
                        &guardrail_response,
                    )
                    .await
                {
                    callback.fail(error.to_string(), "guardrail_output");
                    return Err(error);
                }
                if !opt_in
                    && let Err(error) = super::response_cache::store_chat(
                        state,
                        request.as_ref(),
                        &response,
                        context.as_ref(),
                    )
                    .await
                {
                    callback.fail(error.to_string(), "cache_error");
                    return Err(error);
                }
                let response =
                    ChatCompletionResponseWithExtensions::from_parts(response, choice_extensions)
                        .map_err(GatewayError::internal)
                        .inspect_err(|error| {
                            callback.fail(error.to_string(), "response_projection")
                        })?;
                return Ok((response, callback));
            }
        }
    }
    let error = last_output_block.unwrap_or_else(|| {
        GatewayError::Forbidden(crate::server::guardrails::OUTPUT_BLOCK_MESSAGE.to_string())
    });
    callback.fail(error.to_string(), "guardrail_output");
    Err(error)
}

pub(crate) fn build_core_chat_request(
    request: &ChatCompletionRequest,
    model: String,
    stream: bool,
) -> Result<CoreChatRequest, GatewayError> {
    build_core_chat_request_with_stream_usage(request, model, stream, None)
}
pub(crate) fn build_core_chat_request_with_stream_usage(
    request: &ChatCompletionRequest,
    model: String,
    stream: bool,
    include_usage_override: Option<bool>,
) -> Result<CoreChatRequest, GatewayError> {
    // This is the OpenAI transport DTO -> internal provider request boundary.
    // Keep field forwarding explicit so new OpenAI-compatible parameters cannot
    // disappear silently while routing through the canonical ChatRequest tree.
    let tools = match request.tools.as_ref() {
        Some(tools) => {
            let mut converted = Vec::with_capacity(tools.len());
            for tool in tools.iter().cloned() {
                converted.push(convert_tool(tool)?);
            }
            Some(converted)
        }
        None => None,
    };

    let tool_choice = request.tool_choice.clone().map(convert_tool_choice);

    let functions = match request.functions.as_ref() {
        Some(funcs) => {
            let mut values = Vec::with_capacity(funcs.len());
            for function in funcs {
                values.push(serde_json::to_value(function).map_err(|e| {
                    GatewayError::internal(format!("Failed to serialize function: {}", e))
                })?);
            }
            Some(values)
        }
        None => None,
    };

    let function_call = match request.function_call.as_ref() {
        Some(call) => Some(serde_json::to_value(call).map_err(|e| {
            GatewayError::internal(format!("Failed to serialize function call: {}", e))
        })?),
        None => None,
    };

    let response_format =
        request
            .response_format
            .as_ref()
            .map(|format| types::tools::ResponseFormat {
                format_type: format.format_type.clone(),
                json_schema: format.json_schema.clone(),
                response_type: format.response_type.clone(),
            });

    let seed = request
        .seed
        .map(|seed| {
            i32::try_from(seed).map_err(|_| {
                GatewayError::validation(format!(
                    "seed must be between {} and {}",
                    i32::MIN,
                    i32::MAX
                ))
            })
        })
        .transpose()?;

    let mut extra_params = request.extra_body.clone();
    if let Some(modalities) = request.modalities.as_ref() {
        extra_params.insert("modalities".to_string(), json!(modalities));
    }
    if let Some(audio) = request.audio.as_ref() {
        extra_params.insert("audio".to_string(), json!(audio));
    }
    if let Some(prediction) = request.prediction.as_ref() {
        extra_params.insert("prediction".to_string(), prediction.clone());
    }
    if let Some(safety_settings) = request.safety_settings.as_ref() {
        extra_params.insert("safety_settings".to_string(), safety_settings.clone());
    }
    if let Some(cache_control) = request.cache_control.as_ref() {
        extra_params.insert("cache_control".to_string(), cache_control.clone());
    }

    let stream_options = match (request.stream_options.as_ref(), include_usage_override) {
        (Some(_), Some(include_usage)) => Some(crate::core::types::chat::StreamOptions {
            include_usage: Some(include_usage),
        }),
        (None, Some(include_usage)) => Some(crate::core::types::chat::StreamOptions {
            include_usage: Some(include_usage),
        }),
        (Some(options), None) => Some(crate::core::types::chat::StreamOptions {
            include_usage: options.include_usage,
        }),
        (None, None) => None,
    };

    Ok(CoreChatRequest {
        model,
        messages: request.messages.iter().cloned().map(Into::into).collect(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        max_completion_tokens: request.max_completion_tokens,
        top_p: request.top_p,
        frequency_penalty: request.frequency_penalty,
        presence_penalty: request.presence_penalty,
        stop: request.stop.clone(),
        stream,
        stream_options,
        tools,
        tool_choice,
        parallel_tool_calls: request.parallel_tool_calls,
        response_format,
        user: request.user.clone(),
        seed,
        n: request.n,
        logit_bias: request.logit_bias.clone(),
        functions,
        function_call,
        logprobs: request.logprobs,
        top_logprobs: request.top_logprobs,
        thinking: None,
        reasoning_effort: request.reasoning_effort.clone(),
        store: request.store,
        metadata: request.metadata.clone(),
        service_tier: request.service_tier.clone(),
        extra_params,
    })
}

fn convert_core_chat_response(response: types::responses::ChatResponse) -> ChatCompletionResponse {
    ChatCompletionResponse {
        id: response.id,
        object: response.object,
        created: response.created as u64,
        model: response.model,
        system_fingerprint: response.system_fingerprint,
        choices: response
            .choices
            .into_iter()
            .map(|choice| ChatChoice {
                index: choice.index,
                message: choice.message.into(),
                logprobs: choice.logprobs.map(convert_logprobs),
                finish_reason: choice.finish_reason.map(convert_finish_reason),
            })
            .collect(),
        usage: response.usage.map(convert_usage),
    }
}

fn convert_tool(tool: Tool) -> Result<types::tools::Tool, GatewayError> {
    if tool.tool_type.to_lowercase() != "function" {
        return Err(GatewayError::validation("Unsupported tool type"));
    }

    Ok(types::tools::Tool {
        tool_type: types::tools::ToolType::Function,
        function: types::tools::FunctionDefinition {
            name: tool.function.name,
            description: tool.function.description,
            parameters: tool.function.parameters,
        },
    })
}

fn convert_tool_choice(choice: ToolChoice) -> types::tools::ToolChoice {
    match choice {
        ToolChoice::None(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Auto(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Required(value) => types::tools::ToolChoice::String(value),
        ToolChoice::Specific(spec) => types::tools::ToolChoice::Specific {
            choice_type: spec.tool_type,
            function: Some(types::tools::FunctionChoice {
                name: spec.function.name,
            }),
        },
    }
}

fn convert_logprobs(logprobs: types::responses::LogProbs) -> Logprobs {
    let content = if logprobs.content.is_empty() {
        None
    } else {
        Some(
            logprobs
                .content
                .into_iter()
                .map(|token| ContentLogprob {
                    token: token.token,
                    logprob: token.logprob,
                    bytes: token.bytes,
                    top_logprobs: token.top_logprobs.map(|tops| {
                        tops.into_iter()
                            .map(|top| TopLogprob {
                                token: top.token,
                                logprob: top.logprob,
                                bytes: top.bytes,
                            })
                            .collect()
                    }),
                })
                .collect(),
        )
    };

    Logprobs { content }
}

fn convert_finish_reason(reason: types::responses::FinishReason) -> String {
    match reason {
        types::responses::FinishReason::Stop => "stop",
        types::responses::FinishReason::Length => "length",
        types::responses::FinishReason::ToolCalls => "tool_calls",
        types::responses::FinishReason::ContentFilter => "content_filter",
        types::responses::FinishReason::FunctionCall => "function_call",
        types::responses::FinishReason::StopSequence => "stop_sequence",
        types::responses::FinishReason::Refusal => "refusal",
        types::responses::FinishReason::PauseTurn => "pause_turn",
    }
    .to_string()
}

fn convert_usage(usage: types::responses::Usage) -> Usage {
    let thinking_tokens = usage.thinking_tokens();
    let completion_tokens_details = usage
        .completion_tokens_details
        .map(
            |details| crate::core::models::openai::CompletionTokensDetails {
                reasoning_tokens: details.reasoning_tokens.or(thinking_tokens),
                audio_tokens: details.audio_tokens,
            },
        )
        .or_else(|| {
            thinking_tokens.map(
                |tokens| crate::core::models::openai::CompletionTokensDetails {
                    reasoning_tokens: Some(tokens),
                    audio_tokens: None,
                },
            )
        });

    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: usage.prompt_tokens_details.map(|details| {
            crate::core::models::openai::PromptTokensDetails {
                cached_tokens: details.cached_tokens,
                cache_creation_tokens: details.cache_creation_tokens,
                cache_read_tokens: details.cache_read_tokens,
                audio_tokens: details.audio_tokens,
            }
        }),
        completion_tokens_details,
        thinking_usage: usage.thinking_usage,
    }
}

fn convert_core_chunk_to_streaming(
    chunk: types::responses::ChatChunk,
) -> Result<ChatCompletionChunk, ProviderError> {
    let choices = chunk
        .choices
        .into_iter()
        .map(|choice| {
            let logprobs = choice
                .logprobs
                .map(|lp| {
                    serde_json::to_value(convert_logprobs(lp)).map_err(|e| {
                        ProviderError::serialization(
                            "router",
                            format!("Failed to serialize stream logprobs: {}", e),
                        )
                    })
                })
                .transpose()?;

            Ok(ChatCompletionChunkChoice {
                index: choice.index,
                delta: {
                    let reasoning_content = choice
                        .delta
                        .thinking
                        .as_ref()
                        .and_then(|thinking| thinking.content.clone());
                    ChatCompletionDelta {
                        role: choice.delta.role,
                        content: choice.delta.content,
                        thinking: choice.delta.thinking,
                        reasoning_content,
                        tool_calls: choice
                            .delta
                            .tool_calls
                            .map(|calls| calls.into_iter().map(convert_tool_call_delta).collect()),
                        function_call: choice.delta.function_call.map(convert_function_call_delta),
                        audio: choice.delta.audio,
                        ..Default::default()
                    }
                },
                finish_reason: choice.finish_reason.map(convert_finish_reason),
                logprobs,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;

    Ok(ChatCompletionChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created as u64,
        model: chunk.model,
        system_fingerprint: chunk.system_fingerprint,
        choices,
        usage: chunk.usage.map(convert_usage),
    })
}

#[cfg(test)]
#[path = "chat_tests.rs"]
mod chat_tests;
