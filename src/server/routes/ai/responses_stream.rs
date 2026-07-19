//! Streaming handler for POST /v1/responses
//!
//! Translates internal `ChatChunk` SSE events into Responses API streaming
//! events as defined in the OpenAI Responses API specification.

use crate::core::models::openai::requests::{ChatCompletionRequest, StreamOptions};
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage,
    ResponseStreamEvent, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::responses::Usage as ChatUsage;
use crate::core::types::{context::SharedRequestContext, model::ProviderCapability};
use crate::server::routes::ai::chat::build_core_chat_request;
use crate::server::routes::ai::responses::{
    ResponseOwner, current_unix_ts, finish_reason_enum_to_status, store_response_if_requested,
    uuid_v4_hex,
};
use crate::server::state::AppState;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::budgeted::{ApiKeyBudgetPolicy, run_stream};
use super::callbacks::CallbackLifecycle;
use super::{openai_errors, spend};
#[path = "responses_stream_budget.rs"]
mod responses_stream_budget;
use responses_stream_budget::StreamBudgetSettlement;
#[path = "responses_stream_support.rs"]
mod responses_stream_support;
use responses_stream_support::{
    classify, completed_reasoning_item, emit, in_progress_reasoning_item, make_shell,
    output_items_in_stream_order, response_usage_from_chat_usage, sse_error,
};

/// Accumulated state for one in-progress tool call during streaming.
struct ToolCallAccum {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    output_index: u32,
}

/// Streaming path for POST /v1/responses.
pub(crate) async fn handle_streaming_response(
    state: &AppState,
    mut chat_request: ChatCompletionRequest,
    original: ResponsesApiRequest,
    context: SharedRequestContext,
    owner: Option<ResponseOwner>,
) -> ActixResult<HttpResponse> {
    info!(
        "Streaming Responses API request for model: {}",
        chat_request.model
    );

    chat_request.stream = Some(true);
    chat_request.stream_options = Some(StreamOptions {
        include_usage: Some(true),
    });
    let chat_request = Arc::new(chat_request);
    let model_name = chat_request.model.clone();
    let resp_id = format!("resp_{}", uuid_v4_hex());
    let created_at = current_unix_ts();

    let core_request =
        match build_core_chat_request(chat_request.as_ref(), model_name.clone(), true) {
            Ok(r) => r,
            Err(e) => {
                return Ok(openai_errors::gateway_error_response(&e));
            }
        };

    let requested_model = core_request.model.clone();
    let callback = CallbackLifecycle::new(
        &state.callbacks,
        state.budgeted.pricing(),
        &requested_model,
        context.as_ref(),
    );
    let context_clone = Arc::clone(&context);
    let request_for_execution = Arc::clone(&chat_request);
    let budgeted = state.budgeted.clone();
    let pricing_service = budgeted.pricing();
    let pricing_config = state.config().gateway.pricing.clone();
    let api_key_id = context.api_key_id();
    let api_key_budget_id = context.api_key_budget_id();
    let settlement_budgeted = state.budgeted.clone();
    let callback_for_execution = callback.clone();

    match run_stream(
        state.unified_router.clone(),
        &requested_model,
        ProviderCapability::ChatCompletionStream,
        move |provider, selected_model, _selected_deployment_id| {
            let core_request = core_request.clone();
            let ctx = Arc::clone(&context_clone);
            let original_request = Arc::clone(&request_for_execution);
            let budgeted = budgeted.clone();
            let pricing_service = pricing_service.clone();
            let pricing_config = pricing_config.clone();
            let callback = callback_for_execution.clone();
            async move {
                let provider_name = provider.name().to_string();
                let (pricing_provider, pricing_model) = spend::pricing_identity_for_provider(
                    pricing_service.as_ref(),
                    &provider,
                    &selected_model,
                );
                let req = super::token_policy::prepare_chat_request_for_provider(
                    ctx.api_key_max_tokens_per_request(),
                    &provider_name,
                    &selected_model,
                    core_request.clone(),
                )?;
                let request_for_budget =
                    spend::ChatCompletionBudgetRequest::from(original_request.as_ref())
                        .with_output_limits(req.max_tokens, req.max_completion_tokens);
                let provider_context = ctx.as_ref().clone();
                let reserve_pricing_service = pricing_service.clone();
                let reserve_pricing_config = pricing_config.clone();
                let reserve_pricing_provider = pricing_provider.clone();
                let reserve_pricing_model = pricing_model.clone();
                let callback_provider = provider_name.clone();
                let callback_model = selected_model.clone();
                let callback_pricing_provider = reserve_pricing_provider.clone();
                let callback_pricing_model = reserve_pricing_model.clone();
                let (stream, reservations) = budgeted
                    .for_selected_with_api_key_budget(
                        provider_name.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call(
                        |budget| {
                            spend::reserve_chat_completion_budget_with_split_pricing(
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
                            provider.chat_completion_stream(req, provider_context)
                        },
                    )
                    .await?;
                let (budget_reservation, key_budget_reservation) = reservations.into_parts();
                Ok((
                    stream,
                    provider_name,
                    selected_model,
                    pricing_provider,
                    pricing_model,
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
                pricing_provider,
                pricing_model,
                budget_reservation,
                key_budget_reservation,
            ),
            lease,
        )) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout = state.config.load().gateway.server.stream_idle_timeout;
            let settlement = StreamBudgetSettlement {
                pricing_service: settlement_budgeted.pricing(),
                pricing_config: state.config().gateway.pricing.clone(),
                budget_limits: settlement_budgeted.budget_limits(),
                key_manager: settlement_budgeted.key_manager(),
                api_key_id,
                provider: served_provider,
                model: served_model,
                pricing_provider,
                pricing_model,
                budget_reservation,
                key_budget_reservation,
            };

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut settlement = settlement;

                let shell = make_shell(&resp_id, created_at, &model_name, "in_progress", &original);
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseCreated {
                        response: Box::new(shell),
                    },
                )
                .await
                .is_err()
                {
                    callback.fail("client disconnected", "client_disconnect");
                    return;
                }

                let mut full_text = String::new();
                let mut text_started = false;
                let mut text_item_id = String::new();
                let mut text_output_index: u32 = 0;
                let mut in_tokens: u32 = 0;
                let mut out_tokens: u32 = 0;
                let mut final_usage: Option<ChatUsage> = None;
                let mut saw_upstream_output = false;
                let mut next_output_index: u32 = 0;
                let mut final_status: &'static str = "completed";
                let mut tool_states: HashMap<u32, ToolCallAccum> = HashMap::new();
                let mut tool_order: Vec<u32> = Vec::new();
                let mut full_reasoning = String::new();
                let mut reasoning_item_id = String::new();
                let mut reasoning_output_index: u32 = 0;
                let mut reasoning_started = false;
                macro_rules! return_after_disconnect {
                    () => {
                        callback.fail("client disconnected", "client_disconnect");
                        if final_usage.is_some() || saw_upstream_output {
                            settlement.record_disconnect(final_usage.as_ref()).await;
                        }
                        return;
                    };
                }

                loop {
                    let next = if idle_timeout == 0 {
                        stream.next().await
                    } else {
                        match tokio::time::timeout(Duration::from_secs(idle_timeout), stream.next())
                            .await
                        {
                            Ok(r) => r,
                            Err(_) => {
                                warn!("Responses API stream idle timeout after {idle_timeout}s");
                                let _ = tx
                                    .send(sse_error(
                                        "stream idle timeout",
                                        "server_error",
                                        "timeout",
                                    ))
                                    .await;
                                if let Some(lease) = lease.take() {
                                    let error = ProviderError::timeout(
                                        "router",
                                        format!("stream idle timeout after {idle_timeout}s"),
                                    );
                                    lease.finish_failure(&error);
                                }
                                callback.fail(
                                    format!("stream idle timeout after {idle_timeout}s"),
                                    "timeout",
                                );
                                return_after_disconnect!();
                            }
                        }
                    };

                    let Some(result) = next else { break };

                    match result {
                        Ok(chunk) => {
                            if let Some(u) = &chunk.usage {
                                in_tokens = u.prompt_tokens;
                                out_tokens = u.completion_tokens;
                                final_usage = Some(u.clone());
                                saw_upstream_output = true;
                            }
                            for choice in &chunk.choices {
                                if let Some(r) = &choice.finish_reason {
                                    final_status = finish_reason_enum_to_status(Some(r));
                                }

                                if let Some(thinking) = &choice.delta.thinking
                                    && let Some(reasoning_text) = thinking.content.as_deref()
                                    && !reasoning_text.is_empty()
                                {
                                    saw_upstream_output = true;
                                    if !reasoning_started {
                                        if text_started {
                                            warn!(
                                                "responses stream reasoning chunk arrived after text chunk; \
                                                 output_index ordering will be reversed from canonical \
                                                 (reasoning={}, text={})",
                                                next_output_index, text_output_index
                                            );
                                        }
                                        reasoning_started = true;
                                        reasoning_output_index = next_output_index;
                                        next_output_index += 1;
                                        reasoning_item_id = format!("rs_{}", uuid_v4_hex());

                                        if emit(
                                            &tx,
                                            &ResponseStreamEvent::ResponseOutputItemAdded {
                                                output_index: reasoning_output_index,
                                                item: in_progress_reasoning_item(
                                                    reasoning_item_id.clone(),
                                                ),
                                            },
                                        )
                                        .await
                                        .is_err()
                                        {
                                            return_after_disconnect!();
                                        }
                                    }
                                    full_reasoning.push_str(reasoning_text);
                                    if emit(
                                        &tx,
                                        &ResponseStreamEvent::ResponseReasoningSummaryTextDelta {
                                            output_index: reasoning_output_index,
                                            summary_index: 0,
                                            delta: reasoning_text.to_string(),
                                        },
                                    )
                                    .await
                                    .is_err()
                                    {
                                        return_after_disconnect!();
                                    }
                                }

                                let text = choice.delta.content.as_deref().unwrap_or("");
                                if !text.is_empty() {
                                    saw_upstream_output = true;
                                    if !text_started {
                                        text_started = true;
                                        text_output_index = next_output_index;
                                        next_output_index += 1;
                                        text_item_id = format!("msg_{}", uuid_v4_hex());

                                        let placeholder =
                                            ResponseOutputItem::Message(ResponseOutputMessage {
                                                id: text_item_id.clone(),
                                                role: "assistant".to_string(),
                                                status: "in_progress".to_string(),
                                                content: vec![],
                                            });
                                        if emit(
                                            &tx,
                                            &ResponseStreamEvent::ResponseOutputItemAdded {
                                                output_index: text_output_index,
                                                item: placeholder,
                                            },
                                        )
                                        .await
                                        .is_err()
                                        {
                                            return_after_disconnect!();
                                        }

                                        if emit(
                                            &tx,
                                            &ResponseStreamEvent::ResponseContentPartAdded {
                                                output_index: text_output_index,
                                                content_index: 0,
                                                part: ResponseOutputContent::OutputText {
                                                    text: String::new(),
                                                    annotations: None,
                                                    logprobs: None,
                                                },
                                            },
                                        )
                                        .await
                                        .is_err()
                                        {
                                            return_after_disconnect!();
                                        }
                                    }

                                    full_text.push_str(text);
                                    if emit(
                                        &tx,
                                        &ResponseStreamEvent::ResponseOutputTextDelta {
                                            output_index: text_output_index,
                                            content_index: 0,
                                            delta: text.to_string(),
                                        },
                                    )
                                    .await
                                    .is_err()
                                    {
                                        return_after_disconnect!();
                                    }
                                }

                                if let Some(tc_deltas) = &choice.delta.tool_calls {
                                    if !tc_deltas.is_empty() {
                                        saw_upstream_output = true;
                                    }
                                    for tc in tc_deltas {
                                        let idx = tc.index;

                                        // First chunk for this call (has an id): emit placeholder
                                        if let (
                                            Some(call_id),
                                            std::collections::hash_map::Entry::Vacant(entry),
                                        ) = (&tc.id, tool_states.entry(idx))
                                        {
                                            let item_id = format!("fc_{}", uuid_v4_hex());
                                            let out_idx = next_output_index;
                                            next_output_index += 1;
                                            let name = tc
                                                .function
                                                .as_ref()
                                                .and_then(|f| f.name.as_deref())
                                                .unwrap_or("")
                                                .to_string();

                                            let fc_item = ResponseOutputItem::FunctionCall(
                                                ResponseFunctionCall {
                                                    id: item_id.clone(),
                                                    name: name.clone(),
                                                    arguments: String::new(),
                                                    status: "in_progress".to_string(),
                                                    call_id: Some(call_id.clone()),
                                                },
                                            );
                                            if emit(
                                                &tx,
                                                &ResponseStreamEvent::ResponseOutputItemAdded {
                                                    output_index: out_idx,
                                                    item: fc_item,
                                                },
                                            )
                                            .await
                                            .is_err()
                                            {
                                                return_after_disconnect!();
                                            }

                                            entry.insert(ToolCallAccum {
                                                item_id,
                                                call_id: call_id.clone(),
                                                name,
                                                arguments: String::new(),
                                                output_index: out_idx,
                                            });
                                            tool_order.push(idx);
                                        }

                                        if let Some(fn_delta) = &tc.function
                                            && let Some(state) = tool_states.get_mut(&idx)
                                        {
                                            // Late-arriving name (rare)
                                            if let Some(n) = &fn_delta.name
                                                && state.name.is_empty()
                                            {
                                                state.name.clone_from(n);
                                            }
                                            // Emit argument deltas
                                            if let Some(args) = &fn_delta.arguments
                                                && !args.is_empty()
                                            {
                                                state.arguments.push_str(args);
                                                let (cid, oi) =
                                                    (state.call_id.clone(), state.output_index);
                                                if emit(
                                                    &tx,
                                                    &ResponseStreamEvent::ResponseFunctionCallArgumentsDelta {
                                                        output_index: oi,
                                                        call_id: cid,
                                                        delta: args.clone(),
                                                    },
                                                )
                                                .await
                                                .is_err()
                                                {
                                                    return_after_disconnect!();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Responses API stream error: {e}");
                            let (et, ec) = classify(&e);
                            let _ = tx.send(sse_error(&e.to_string(), et, ec)).await;
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&e);
                            }
                            callback.fail(e.to_string(), "provider_error");
                            return_after_disconnect!();
                        }
                    }
                }

                let item_status = final_status;
                let mut all_output: Vec<(u32, ResponseOutputItem)> = Vec::new();

                if reasoning_started {
                    if emit(
                        &tx,
                        &ResponseStreamEvent::ResponseReasoningSummaryTextDone {
                            output_index: reasoning_output_index,
                            summary_index: 0,
                            text: full_reasoning.clone(),
                        },
                    )
                    .await
                    .is_err()
                    {
                        return_after_disconnect!();
                    }

                    let reasoning_done =
                        completed_reasoning_item(reasoning_item_id, item_status, full_reasoning);
                    if emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputItemDone {
                            output_index: reasoning_output_index,
                            item: reasoning_done.clone(),
                        },
                    )
                    .await
                    .is_err()
                    {
                        return_after_disconnect!();
                    }
                    all_output.push((reasoning_output_index, reasoning_done));
                }

                if text_started {
                    if emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputTextDone {
                            output_index: text_output_index,
                            content_index: 0,
                            text: full_text.clone(),
                        },
                    )
                    .await
                    .is_err()
                    {
                        return_after_disconnect!();
                    }

                    if emit(
                        &tx,
                        &ResponseStreamEvent::ResponseContentPartDone {
                            output_index: text_output_index,
                            content_index: 0,
                            part: ResponseOutputContent::OutputText {
                                text: full_text.clone(),
                                annotations: None,
                                logprobs: None,
                            },
                        },
                    )
                    .await
                    .is_err()
                    {
                        return_after_disconnect!();
                    }

                    let text_done = ResponseOutputItem::Message(ResponseOutputMessage {
                        id: text_item_id,
                        role: "assistant".to_string(),
                        status: item_status.to_string(),
                        content: vec![ResponseOutputContent::OutputText {
                            text: full_text.clone(),
                            annotations: None,
                            logprobs: None,
                        }],
                    });
                    if emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputItemDone {
                            output_index: text_output_index,
                            item: text_done.clone(),
                        },
                    )
                    .await
                    .is_err()
                    {
                        return_after_disconnect!();
                    }
                    all_output.push((text_output_index, text_done));
                }

                for idx in &tool_order {
                    if let Some(state) = tool_states.get(idx) {
                        if emit(
                            &tx,
                            &ResponseStreamEvent::ResponseFunctionCallArgumentsDone {
                                output_index: state.output_index,
                                call_id: state.call_id.clone(),
                                arguments: state.arguments.clone(),
                            },
                        )
                        .await
                        .is_err()
                        {
                            return_after_disconnect!();
                        }

                        let fc_done = ResponseOutputItem::FunctionCall(ResponseFunctionCall {
                            id: state.item_id.clone(),
                            name: state.name.clone(),
                            arguments: state.arguments.clone(),
                            status: "completed".to_string(),
                            call_id: Some(state.call_id.clone()),
                        });
                        if emit(
                            &tx,
                            &ResponseStreamEvent::ResponseOutputItemDone {
                                output_index: state.output_index,
                                item: fc_done.clone(),
                            },
                        )
                        .await
                        .is_err()
                        {
                            return_after_disconnect!();
                        }
                        all_output.push((state.output_index, fc_done));
                    }
                }

                let output_items = output_items_in_stream_order(all_output);

                let total = in_tokens + out_tokens;
                let budget_usage = final_usage.or_else(|| {
                    (total > 0).then_some(ChatUsage {
                        prompt_tokens: in_tokens,
                        completion_tokens: out_tokens,
                        total_tokens: total,
                        prompt_tokens_details: None,
                        completion_tokens_details: None,
                        thinking_usage: None,
                    })
                });
                settlement
                    .record_completion(budget_usage.as_ref(), saw_upstream_output)
                    .await;
                callback.complete_usage(budget_usage.as_ref(), "success");
                if let Some(lease) = lease.take() {
                    let tokens_used = budget_usage
                        .as_ref()
                        .map(|u| u.total_tokens)
                        .unwrap_or(total);
                    lease.finish_success(u64::from(tokens_used));
                }
                let usage = budget_usage.as_ref().map(response_usage_from_chat_usage);
                let completed = ResponsesApiResponse {
                    id: resp_id,
                    object: "response".to_string(),
                    created_at,
                    status: item_status.to_string(),
                    model: model_name,
                    output: output_items,
                    usage,
                    error: None,
                    previous_response_id: original.previous_response_id.clone(),
                    metadata: original.metadata.clone(),
                };
                store_response_if_requested(&original, &completed, owner);
                let _ = emit(
                    &tx,
                    &ResponseStreamEvent::ResponseCompleted {
                        response: Box::new(completed),
                    },
                )
                .await;

                let _ = tx.send(Event::default().data("[DONE]").to_bytes()).await;
            });

            let body = tokio_stream::wrappers::ReceiverStream::new(rx)
                .map(Ok::<_, actix_web::error::Error>);

            Ok(HttpResponse::Ok()
                .insert_header((CONTENT_TYPE, "text/event-stream"))
                .insert_header((CACHE_CONTROL, "no-cache"))
                .insert_header(("Connection", "keep-alive"))
                .insert_header(("X-Request-ID", context.request_id.as_str()))
                .streaming(body))
        }
        Err(e) => {
            error!("Failed to start Responses API stream: {e}");
            callback.fail(e.to_string(), "provider_error");
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

#[cfg(test)]
#[path = "responses_stream_tests.rs"]
mod tests;
