//! Chat completions endpoint

use crate::core::models::openai::{
    ChatChoice, ChatCompletionRequest, ChatCompletionResponse, ContentLogprob, Logprobs,
    StreamOptions, Tool, ToolChoice, TopLogprob, Usage,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta, Event,
};
use crate::core::types::{
    self, chat::ChatRequest as CoreChatRequest, context::RequestContext, model::ProviderCapability,
};
use crate::server::state::AppState;
use crate::utils::data::validation::RequestValidator;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::context::get_request_context;
use super::execution::{execute_stream_with_selected_deployment, execute_with_selected_deployment};
use super::openai_errors;
#[path = "chat_delta.rs"]
mod chat_delta;
use chat_delta::{convert_function_call_delta, convert_tool_call_delta};
#[path = "chat_sse.rs"]
pub(super) mod chat_sse;
use chat_sse::format_sse_error;
pub(super) use chat_sse::sse_error_classification;

/// Chat completions endpoint
///
/// OpenAI-compatible chat completions API that supports streaming and non-streaming responses.
pub async fn chat_completions(
    state: web::Data<AppState>,
    req: HttpRequest,
    request: web::Json<ChatCompletionRequest>,
) -> ActixResult<HttpResponse> {
    info!("Chat completion request for model: {}", request.model);

    let mut context = get_request_context(&req)?;
    super::token_policy::attach_api_key_token_limit(&req, &mut context)?;

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

    if request.stream.unwrap_or(false) {
        handle_streaming_chat_completion(state.get_ref(), request.into_inner(), context).await
    } else {
        match handle_chat_completion_with_state(state.get_ref(), request.into_inner(), context)
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

/// Handle streaming chat completion
async fn handle_streaming_chat_completion(
    state: &AppState,
    mut request: ChatCompletionRequest,
    context: RequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Handling streaming chat completion for model: {}",
        request.model
    );

    let requested_model = request.model.clone();
    let request_for_budget = request.clone();
    let client_requested_usage = request
        .stream_options
        .as_ref()
        .and_then(|options| options.include_usage)
        .unwrap_or(false);
    request
        .stream_options
        .get_or_insert(StreamOptions {
            include_usage: None,
        })
        .include_usage = Some(true);
    let core_request = match build_core_chat_request(request, requested_model, true) {
        Ok(req) => req,
        Err(e) => return Ok(openai_errors::gateway_error_response(&e)),
    };

    let requested_model = core_request.model.clone();
    let context_for_execution = context.clone();
    let (budget_limits, pricing_service) = (state.budget_limits.clone(), state.pricing.clone());
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budget_manager = state.budget_manager.clone();
    match execute_stream_with_selected_deployment(
        state.unified_router.clone(),
        &requested_model,
        ProviderCapability::ChatCompletionStream,
        move |provider, selected_model, _selected_deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let (budget_limits, pricing_service) = (budget_limits.clone(), pricing_service.clone());
            let budget_manager = budget_manager.clone();
            let request_for_budget = request_for_budget.clone();
            async move {
                super::spend::ensure_budget_available(
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                )?;
                let provider_name = provider.name().to_string();
                let (request_for_provider, request_for_budget) =
                    super::token_policy::prepare_chat_request_for_provider(
                        context.api_key_max_tokens_per_request(),
                        provider.name(),
                        &selected_model,
                        core_request.clone(),
                        request_for_budget,
                    )?;
                let budget_reservation = super::spend::reserve_chat_completion_budget_with_pricing(
                    pricing_service.as_ref(),
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                    &request_for_budget,
                )?;
                let key_budget_reservation = super::spend::reserve_api_key_budget_for_reservation(
                    &budget_manager,
                    api_key_budget_id,
                    budget_reservation.as_ref(),
                )?;
                let stream = provider
                    .chat_completion_stream(request_for_provider, context)
                    .await?;
                Ok((
                    stream,
                    provider_name,
                    selected_model,
                    budget_reservation,
                    key_budget_reservation,
                ))
            }
        },
    )
    .await
    {
        Ok((
            (
                mut stream,
                served_provider,
                served_model,
                mut budget_reservation,
                mut key_budget_reservation,
            ),
            lease,
        )) => {
            // Use a channel so that when the client disconnects and actix-web
            // drops the response stream, the receiver is dropped, which causes
            // the sender to fail. This breaks the upstream read loop and drops
            // the provider stream, cancelling any in-flight HTTP request.
            let (tx, rx) = mpsc::channel::<Bytes>(8);

            let idle_timeout_secs = state.config.load().gateway.server.stream_idle_timeout;
            let budget_limits = state.budget_limits.clone();
            let (key_manager, pricing_service) = (state.key_manager.clone(), state.pricing.clone());

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut tokens_used = 0_u64;
                let mut final_usage = None;
                let mut saw_upstream_output = false;
                macro_rules! settle_after_upstream_output {
                    () => {
                        if final_usage.is_some() || saw_upstream_output {
                            super::spend::record_stream_disconnect_spend_with_reservation_with_pricing(
                                pricing_service.as_ref(),
                                super::spend::usage_spend_settlement(
                                    (&budget_limits, &key_manager, api_key_id),
                                    (&served_provider, &served_model, final_usage.as_ref()),
                                    budget_reservation.take(),
                                    key_budget_reservation.take(),
                                ),
                            )
                            .await;
                        }
                    };
                }

                loop {
                    let chunk_result = if idle_timeout_secs == 0 {
                        stream.next().await
                    } else {
                        let timeout_dur = Duration::from_secs(idle_timeout_secs);
                        match tokio::time::timeout(timeout_dur, stream.next()).await {
                            Ok(result) => result,
                            Err(_) => {
                                warn!(
                                    "SSE stream idle timeout after {}s, closing connection",
                                    idle_timeout_secs
                                );
                                let error_bytes = format_sse_error(
                                    &format!(
                                        "Stream idle timeout: no data received for {}s",
                                        idle_timeout_secs
                                    ),
                                    "server_error",
                                    "timeout",
                                );
                                if tx.send(error_bytes).await.is_err() {
                                    info!("Client disconnected before timeout error could be sent");
                                }
                                if let Some(lease) = lease.take() {
                                    let error = ProviderError::timeout(
                                        "router",
                                        format!("stream idle timeout after {}s", idle_timeout_secs),
                                    );
                                    lease.finish_failure(&error);
                                }
                                settle_after_upstream_output!();
                                return;
                            }
                        }
                    };

                    let Some(chunk_result) = chunk_result else {
                        break;
                    };

                    let bytes = match chunk_result {
                        Ok(chunk) => {
                            saw_upstream_output = true;
                            if let Some(usage) = &chunk.usage {
                                tokens_used = u64::from(usage.total_tokens);
                                final_usage = Some(usage.clone());
                            }
                            let mut chat_chunk = match convert_core_chunk_to_streaming(chunk) {
                                Ok(chat_chunk) => chat_chunk,
                                Err(e) => {
                                    error!("Stream chunk conversion error: {}", e);
                                    let (error_type, error_code) = sse_error_classification(&e);
                                    let error_bytes =
                                        format_sse_error(&e.to_string(), error_type, error_code);
                                    if tx.send(error_bytes).await.is_err() {
                                        info!(
                                            "Client disconnected before conversion error could be sent"
                                        );
                                    }
                                    if let Some(lease) = lease.take() {
                                        lease.finish_failure(&e);
                                    }
                                    settle_after_upstream_output!();
                                    return;
                                }
                            };
                            if !client_requested_usage {
                                chat_chunk.usage = None;
                                if chat_chunk.choices.is_empty() {
                                    continue;
                                }
                            }
                            match serde_json::to_string(&chat_chunk) {
                                Ok(json) => {
                                    let event = Event::default().data(&json);
                                    event.to_bytes()
                                }
                                Err(e) => {
                                    error!("Stream serialization error: {}", e);
                                    let error_bytes = format_sse_error(
                                        &format!("Serialization error: {}", e),
                                        "server_error",
                                        "internal_error",
                                    );
                                    // Send error then stop reading upstream.
                                    if tx.send(error_bytes).await.is_err() {
                                        info!(
                                            "Client disconnected before error event could be sent"
                                        );
                                    }
                                    if let Some(lease) = lease.take() {
                                        let error = ProviderError::serialization(
                                            "router",
                                            format!("Serialization error: {}", e),
                                        );
                                        lease.finish_failure(&error);
                                    }
                                    settle_after_upstream_output!();
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Stream chunk error: {}", e);
                            let (error_type, error_code) = sse_error_classification(&e);
                            let error_bytes =
                                format_sse_error(&e.to_string(), error_type, error_code);
                            if tx.send(error_bytes).await.is_err() {
                                info!("Client disconnected before error event could be sent");
                            }
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&e);
                            }
                            settle_after_upstream_output!();
                            return;
                        }
                    };

                    if tx.send(bytes).await.is_err() {
                        info!("Client disconnected during streaming, cancelling upstream");
                        super::spend::record_stream_disconnect_spend_with_reservation_with_pricing(
                            pricing_service.as_ref(),
                            super::spend::usage_spend_settlement(
                                (&budget_limits, &key_manager, api_key_id),
                                (&served_provider, &served_model, final_usage.as_ref()),
                                budget_reservation.take(),
                                key_budget_reservation.take(),
                            ),
                        )
                        .await;
                        return;
                    }
                }

                // Send the final [DONE] event; ignore error if client is gone.
                let done_event = Event::default().data("[DONE]");
                if tx.send(done_event.to_bytes()).await.is_err() {
                    info!("Client disconnected before [DONE] event could be sent");
                }
                super::spend::record_finished_stream_spend_with_reservation_with_pricing(
                    pricing_service.as_ref(),
                    super::spend::StreamSpendSettlement {
                        budget_limits: &budget_limits,
                        key_manager: &key_manager,
                        api_key_id,
                        provider: &served_provider,
                        model: &served_model,
                        usage: final_usage.as_ref(),
                        saw_upstream_output,
                        budget_reservation: budget_reservation.take(),
                        key_budget_reservation: key_budget_reservation.take(),
                    },
                )
                .await;
                if let Some(lease) = lease.take() {
                    lease.finish_success(tokens_used);
                }
            });

            let sse_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                .map(Ok::<_, actix_web::error::Error>);

            Ok(HttpResponse::Ok()
                .insert_header((CONTENT_TYPE, "text/event-stream"))
                .insert_header((CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .insert_header(("X-Request-ID", context.request_id.as_str()))
                .streaming(sse_stream))
        }
        Err(e) => {
            error!("Failed to create streaming response: {}", e);
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

/// Handle chat completion with app state (UnifiedRouter only)
pub async fn handle_chat_completion_with_state(
    state: &AppState,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    handle_chat_completion_internal(state, request, context).await
}

async fn handle_chat_completion_internal(
    state: &AppState,
    request: ChatCompletionRequest,
    context: RequestContext,
) -> Result<ChatCompletionResponse, GatewayError> {
    let unified_router = &state.unified_router;
    let requested_model = request.model.clone();
    let request_for_budget = request.clone();
    let request_for_cache = request.clone();
    let core_request = build_core_chat_request(request, requested_model, false)?;
    if let Some(cached) =
        super::response_cache::lookup_chat(state, &request_for_cache, &context).await?
    {
        return Ok(cached);
    }
    let requested_model = core_request.model.clone();
    let context_for_execution = context.clone();

    // Owned handles captured into the (retryable) execution closure so that the
    // successful attempt records budget spend and per-key usage.
    let (budget_limits, pricing_service) = (state.budget_limits.clone(), state.pricing.clone());
    let key_manager = state.key_manager.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let budget_manager = state.budget_manager.clone();

    let core_response = execute_with_selected_deployment(
        unified_router,
        &requested_model,
        ProviderCapability::ChatCompletion,
        move |provider, selected_model, _deployment_id| {
            let core_request = core_request.clone();
            let context = context_for_execution.clone();
            let (budget_limits, pricing_service) = (budget_limits.clone(), pricing_service.clone());
            let key_manager = key_manager.clone();
            let budget_manager = budget_manager.clone();
            let request_for_budget = request_for_budget.clone();
            async move {
                // Reject before spending upstream tokens when the selected
                // provider/model budget is already exhausted.
                super::spend::ensure_budget_available(
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                )?;
                let (request_for_provider, request_for_budget) =
                    super::token_policy::prepare_chat_request_for_provider(
                        context.api_key_max_tokens_per_request(),
                        provider.name(),
                        &selected_model,
                        core_request.clone(),
                        request_for_budget,
                    )?;
                let budget_reservation = super::spend::reserve_chat_completion_budget_with_pricing(
                    pricing_service.as_ref(),
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                    &request_for_budget,
                )?;
                let key_budget_reservation = super::spend::reserve_api_key_budget_for_reservation(
                    &budget_manager,
                    api_key_budget_id,
                    budget_reservation.as_ref(),
                )?;
                let response = provider
                    .chat_completion(request_for_provider, context)
                    .await?;
                let tokens = response
                    .usage
                    .as_ref()
                    .map(|usage| u64::from(usage.total_tokens))
                    .unwrap_or_default();
                super::spend::record_completion_spend_with_reservation_with_pricing(
                    pricing_service.as_ref(),
                    super::spend::usage_spend_settlement(
                        (&budget_limits, &key_manager, api_key_id),
                        (provider.name(), &selected_model, response.usage.as_ref()),
                        budget_reservation,
                        key_budget_reservation,
                    ),
                )
                .await;
                Ok((response, tokens))
            }
        },
    )
    .await?;

    let response = convert_core_chat_response(core_response);
    super::response_cache::store_chat(state, &request_for_cache, &response, &context).await?;
    Ok(response)
}

pub(crate) fn build_core_chat_request(
    request: ChatCompletionRequest,
    model: String,
    stream: bool,
) -> Result<CoreChatRequest, GatewayError> {
    // This is the OpenAI transport DTO -> internal provider request boundary.
    // Keep field forwarding explicit so new OpenAI-compatible parameters cannot
    // disappear silently while routing through the canonical ChatRequest tree.
    let tools = match request.tools {
        Some(tools) => {
            let mut converted = Vec::with_capacity(tools.len());
            for tool in tools {
                converted.push(convert_tool(tool)?);
            }
            Some(converted)
        }
        None => None,
    };

    let tool_choice = request.tool_choice.map(convert_tool_choice);

    let functions = match request.functions {
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

    let function_call = match request.function_call {
        Some(call) => Some(serde_json::to_value(call).map_err(|e| {
            GatewayError::internal(format!("Failed to serialize function call: {}", e))
        })?),
        None => None,
    };

    let response_format = request
        .response_format
        .map(|format| types::tools::ResponseFormat {
            format_type: format.format_type,
            json_schema: format.json_schema,
            response_type: format.response_type,
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

    let mut extra_params = request.extra_body;
    if let Some(modalities) = request.modalities {
        extra_params.insert("modalities".to_string(), json!(modalities));
    }
    if let Some(audio) = request.audio {
        extra_params.insert("audio".to_string(), json!(audio));
    }
    if let Some(prediction) = request.prediction {
        extra_params.insert("prediction".to_string(), prediction);
    }
    if let Some(safety_settings) = request.safety_settings {
        extra_params.insert("safety_settings".to_string(), safety_settings);
    }
    if let Some(cache_control) = request.cache_control {
        extra_params.insert("cache_control".to_string(), cache_control);
    }

    let stream_options = request
        .stream_options
        .map(|so| crate::core::types::chat::StreamOptions {
            include_usage: so.include_usage,
        });

    Ok(CoreChatRequest {
        model,
        messages: request.messages.into_iter().map(Into::into).collect(),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        max_completion_tokens: request.max_completion_tokens,
        top_p: request.top_p,
        frequency_penalty: request.frequency_penalty,
        presence_penalty: request.presence_penalty,
        stop: request.stop,
        stream,
        stream_options,
        tools,
        tool_choice,
        parallel_tool_calls: request.parallel_tool_calls,
        response_format,
        user: request.user,
        seed,
        n: request.n,
        logit_bias: request.logit_bias,
        functions,
        function_call,
        logprobs: request.logprobs,
        top_logprobs: request.top_logprobs,
        thinking: None,
        reasoning_effort: request.reasoning_effort,
        store: request.store,
        metadata: request.metadata,
        service_tier: request.service_tier,
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
