//! Chat completions endpoint

use crate::core::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ContentLogprob, Logprobs, Tool,
    ToolChoice, TopLogprob, Usage,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta,
};
use crate::core::types::{
    self,
    chat::ChatRequest as CoreChatRequest,
    context::{RequestContext, SharedRequestContext},
    model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info, warn};

use super::budgeted::{ApiKeyBudgetPolicy, run_unary};
use super::callbacks::CallbackLifecycle;
use super::openai_errors;
#[path = "chat_delta.rs"]
mod chat_delta;
use chat_delta::{convert_function_call_delta, convert_tool_call_delta};
#[path = "chat_sse.rs"]
pub(super) mod chat_sse;
use chat_sse::format_sse_error;
pub(super) use chat_sse::sse_error_classification;
#[path = "chat_streaming.rs"]
mod chat_streaming;

/// Chat completions endpoint
///
/// OpenAI-compatible chat completions API that supports streaming and non-streaming responses.
pub async fn chat_completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ChatCompletionRequest>,
) -> ActixResult<HttpResponse> {
    info!("Chat completion request for model: {}", request.model);

    let context = match super::token_policy::shared_request_context_with_api_key_token_limit(&req) {
        Ok(context) => context,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    if let Err(e) = RequestValidator::validate_chat_completion_request(
        &request.model,
        &request.messages,
        request.max_tokens,
        request.temperature,
    ) {
        warn!("Invalid chat completion request: {}", e);
        return Ok(openai_errors::validation_error(e.to_string()));
    }
    if let Err(error) = super::context::enforce_api_key_model_and_token_limits(
        &req,
        &request.model,
        super::token_policy::requested_chat_output_token_limit(&request),
    ) {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    let request = Arc::new(request.into_inner());

    if request.stream.unwrap_or(false) {
        chat_streaming::handle_streaming_chat_completion(state.get_ref(), request, context).await
    } else {
        match handle_chat_completion_with_shared_state(state.get_ref(), request, context).await {
            Ok(response) => Ok(HttpResponse::Ok().json(response)),
            Err(e) => {
                error!("Chat completion error: {}", e);
                Ok(openai_errors::gateway_error_response(&e))
            }
        }
    }
}

/// Handle chat completion with app state (UnifiedRouter only)
pub async fn handle_chat_completion_with_state(
    state: &AppState,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    handle_chat_completion_with_shared_state(state, Arc::new(request), Arc::new(context)).await
}

pub async fn handle_chat_completion_with_shared_state(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    crate::server::guardrails::check_chat_input(state, request.as_ref()).await?;
    handle_chat_completion_internal(state, request, context).await
}

async fn handle_chat_completion_internal(
    state: &AppState,
    request: Arc<ChatCompletionRequest>,
    context: SharedRequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    let unified_router = &state.unified_router;
    let requested_model = request.model.clone();
    let core_request = build_core_chat_request(request.as_ref(), requested_model, false)?;
    if let Some(cached) =
        super::response_cache::lookup_chat(state, request.as_ref(), context.as_ref()).await?
    {
        super::response_cache::ensure_chat_cache_pricing_gate(state, request.as_ref())?;
        crate::server::guardrails::check_chat_output(state, &cached).await?;
        return Ok(cached);
    }
    let requested_model = core_request.model.clone();
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        context.as_ref(),
    );
    let context_for_execution = Arc::clone(&context);
    let request_for_execution = Arc::clone(&request);

    // Owned handles captured into the (retryable) execution closure so that the
    // successful attempt records budget spend and per-key usage.
    let pricing_service = state.budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let key_manager = state.budgeted.key_manager();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budgeted = state.budgeted.clone();
    let callback_for_execution = callback.clone();

    let core_response = match run_unary(
        unified_router,
        &requested_model,
        ProviderCapability::ChatCompletion,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = Arc::clone(&context_for_execution);
            let original_request = Arc::clone(&request_for_execution);
            let pricing_service = pricing_service.clone();
            let key_manager = key_manager.clone();
            let budgeted = budgeted.clone();
            let pricing_config = pricing_config.clone();
            let callback = callback_for_execution.clone();
            async move {
                let provider_name = provider.name().to_string();
                let (pricing_provider, pricing_model) =
                    super::spend::pricing_identity_for_provider(
                        pricing_service.as_ref(),
                        &provider,
                        &selected_model,
                    )
                    .into_lookup_parts();
                let request_for_provider = super::token_policy::prepare_chat_request_for_provider(
                    context.api_key_max_tokens_per_request(),
                    &provider_name,
                    &selected_model,
                    core_request.clone(),
                )?;
                let request_for_budget =
                    super::spend::ChatCompletionBudgetRequest::from(original_request.as_ref())
                        .with_output_limits(
                            request_for_provider.max_tokens,
                            request_for_provider.max_completion_tokens,
                        );
                let provider_context = context.as_ref().clone();
                let reserve_pricing_service = pricing_service.clone();
                let settle_pricing_service = pricing_service.clone();
                let reserve_pricing_config = pricing_config.clone();
                let settle_pricing_config = pricing_config;
                let reserve_pricing_provider = pricing_provider.clone();
                let reserve_pricing_model = pricing_model.clone();
                let settle_pricing_provider = pricing_provider;
                let settle_pricing_model = pricing_model;
                let settle_key_manager = key_manager.clone();
                let callback_provider = provider_name.clone();
                let callback_model = selected_model.clone();
                let callback_pricing_provider = reserve_pricing_provider.clone();
                let callback_pricing_model = reserve_pricing_model.clone();
                budgeted
                    .for_selected_with_api_key_budget(
                        provider_name.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call_settle(
                        |budget| {
                            super::spend::reserve_chat_completion_budget_with_split_pricing(
                                reserve_pricing_service.as_ref(),
                                &reserve_pricing_config,
                                budget.budget_limits(),
                                budget.provider(),
                                budget.model(),
                                &reserve_pricing_provider,
                                &reserve_pricing_model,
                                request_for_budget,
                            )
                        },
                        || {
                            callback.begin_provider_execution(
                                callback_provider,
                                callback_model,
                                callback_pricing_provider,
                                callback_pricing_model,
                            );
                            provider.chat_completion(request_for_provider, provider_context)
                        },
                        |response, reservations, budget| {
                            let (budget_reservation, key_budget_reservation) =
                                reservations.into_parts();
                            async move {
                                let tokens = response
                                    .usage
                                    .as_ref()
                                    .map(|usage| u64::from(usage.total_tokens))
                                    .unwrap_or_default();
                                super::spend::record_completion_spend_with_reservation_with_policy(
                                    settle_pricing_service.as_ref(),
                                    &settle_pricing_config,
                                    super::spend::usage_spend_settlement_with_pricing(
                                        (budget.budget_limits(), &settle_key_manager, api_key_id),
                                        (
                                            budget.provider(),
                                            budget.model(),
                                            response.usage.as_ref(),
                                        ),
                                        (&settle_pricing_provider, &settle_pricing_model),
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
            }
        },
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            callback.fail(error.to_string(), "provider_error");
            return Err(error);
        }
    };

    let response = convert_core_chat_response(core_response);
    if let Err(error) = crate::server::guardrails::check_chat_output(state, &response).await {
        callback.fail(error.to_string(), "guardrail_output");
        return Err(error);
    }
    if let Err(error) =
        super::response_cache::store_chat(state, request.as_ref(), &response, context.as_ref())
            .await
    {
        callback.fail(error.to_string(), "cache_error");
        return Err(error);
    }
    callback.complete_usage(response.usage.as_ref(), "success");
    Ok(response)
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
