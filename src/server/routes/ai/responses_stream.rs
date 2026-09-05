//! Streaming handler for POST /v1/responses
//!
//! Translates internal `ChatChunk` SSE events into Responses API streaming
//! events as defined in the OpenAI Responses API specification.

use crate::core::models::openai::requests::{ChatCompletionRequest, StreamOptions};
use crate::core::models::openai::responses_api::{
    ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage, ResponseStreamEvent,
    ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::responses::Usage as ChatUsage;
use crate::core::types::{context::SharedRequestContext, model::ProviderCapability};
use crate::server::routes::ai::chat::build_core_chat_request;
use crate::server::routes::ai::responses::{
    ResponseOwner, current_unix_ts, custom_tool_input, finish_reason_enum_to_status,
    is_custom_tool, store_response_if_requested, uuid_v4_hex,
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
#[path = "responses_stream_state.rs"]
mod responses_stream_state;
#[cfg(test)]
use responses_stream_state::response_stream_total_tokens;
use responses_stream_state::{ToolCallAccum, response_stream_budget_usage};
#[path = "responses_stream_support.rs"]
mod responses_stream_support;
use responses_stream_support::{
    ResponseStreamEmitError, classify, completed_reasoning_item, emit, encode,
    flush_output_guardrail, in_progress_reasoning_item, make_shell, output_items_in_stream_order,
    response_usage_from_chat_usage, send_encoded, send_guardrail_error, sse_error,
};
/// Streaming path for POST /v1/responses.
pub(crate) async fn handle_streaming_response(
    state: &AppState,
    mut chat_request: ChatCompletionRequest,
    guarded_request: ResponsesApiRequest,
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
        state.unified_router().clone(),
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
                let request_pricing = spend::request_pricing_for_provider(
                    &pricing_service,
                    &provider,
                    &selected_model,
                    ProviderCapability::ChatCompletionStream,
                )?;
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
                let reserve_pricing_config = pricing_config.clone();
                let reserve_request_pricing = request_pricing.clone();
                let callback_provider = provider_name.clone();
                let callback_model = selected_model.clone();
                let callback_request_pricing = request_pricing.clone();
                let (stream, reservations) = budgeted
                    .for_selected_with_api_key_budget(
                        provider_name.clone(),
                        selected_model.clone(),
                        api_key_budget_id,
                        ApiKeyBudgetPolicy::FromProviderReservation,
                    )
                    .reserve_call(
                        |budget| {
                            spend::reserve_chat_completion_budget_with_request_pricing(
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
                            provider.chat_completion_stream(req, provider_context)
                        },
                    )
                    .await?;
                let (budget_reservation, key_budget_reservation) = reservations.into_parts();
                Ok((
                    stream,
                    provider_name,
                    selected_model,
                    request_pricing,
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
                request_pricing,
                budget_reservation,
                key_budget_reservation,
            ),
            lease,
        )) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout = state.config.load().gateway.server.stream_idle_timeout;
            let guardrails = Arc::clone(&state.guardrails());
            let settlement = StreamBudgetSettlement {
                pricing_service: settlement_budgeted.pricing(),
                pricing_config: state.config().gateway.pricing.clone(),
                budget_limits: settlement_budgeted.budget_limits(),
                key_manager: settlement_budgeted.key_manager(),
                api_key_id,
                provider: served_provider,
                model: served_model,
                request_pricing,
                budget_reservation,
                key_budget_reservation,
                ledger_facts: crate::core::request_ledger::current_facts(),
            };

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut settlement = settlement;
                let mut output_guardrail =
                    super::stream_output_guardrail::StreamOutputGuardrail::new(guardrails);

                let shell = make_shell(
                    &resp_id,
                    created_at,
                    &model_name,
                    "in_progress",
                    &guarded_request,
                );
                if let Err(error) = emit(
                    &tx,
                    &ResponseStreamEvent::ResponseCreated {
                        response: Box::new(shell),
                    },
                )
                .await
                {
                    match error {
                        ResponseStreamEmitError::ClientDisconnected => {
                            callback.fail("client disconnected", "client_disconnect");
                        }
                        ResponseStreamEmitError::Serialization(error) => {
                            let message = format!("stream serialization failed: {error}");
                            if let Some(lease) = lease.take() {
                                let provider_error =
                                    ProviderError::serialization("router", message.clone());
                                lease.finish_failure(&provider_error);
                            }
                            callback.fail(message, "serialization_error");
                        }
                    }
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
                macro_rules! settle_if_chargeable {
                    () => {{
                        if final_usage.is_some() || saw_upstream_output {
                            settlement.record_disconnect(final_usage.as_ref()).await;
                        }
                    }};
                }
                macro_rules! return_after_disconnect {
                    () => {{
                        callback.fail("client disconnected", "client_disconnect");
                        settle_if_chargeable!();
                        return;
                    }};
                }
                macro_rules! return_after_emit_error {
                    ($error:expr) => {{
                        match $error {
                            ResponseStreamEmitError::ClientDisconnected => {
                                callback.fail("client disconnected", "client_disconnect");
                            }
                            ResponseStreamEmitError::Serialization(error) => {
                                flush_guardrail!();
                                let message = format!("stream serialization failed: {error}");
                                if let Some(lease) = lease.take() {
                                    let provider_error =
                                        ProviderError::serialization("router", message.clone());
                                    lease.finish_failure(&provider_error);
                                }
                                callback.fail(message, "serialization_error");
                            }
                        }
                        settle_if_chargeable!();
                        return;
                    }};
                }
                macro_rules! return_after_guardrail_error {
                    ($error:expr) => {{
                        let error = $error;
                        if send_guardrail_error(&tx, error).await {
                            info!("Client disconnected before guardrail error could be sent");
                        }
                        drop(lease.take());
                        callback.fail(error.message(), "guardrail_output");
                        settle_if_chargeable!();
                        return;
                    }};
                }
                macro_rules! flush_guardrail {
                    () => {
                        match flush_output_guardrail(&tx, &mut output_guardrail).await {
                            Ok(true) => {}
                            Ok(false) => return_after_disconnect!(),
                            Err(error) => return_after_guardrail_error!(error),
                        }
                    };
                }

                loop {
                    let next = if idle_timeout == 0 {
                        tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                return_after_disconnect!();
                            }
                            result = stream.next() => result,
                        }
                    } else {
                        let timed_result = tokio::select! {
                            biased;
                            _ = tx.closed() => {
                                return_after_disconnect!();
                            }
                            result = tokio::time::timeout(
                                Duration::from_secs(idle_timeout),
                                stream.next(),
                            ) => result,
                        };
                        match timed_result {
                            Ok(r) => r,
                            Err(_) => {
                                flush_guardrail!();
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
                                settle_if_chargeable!();
                                return;
                            }
                        }
                    };

                    let Some(result) = next else { break };

                    match result {
                        Ok(chunk) => {
                            let has_candidate_output =
                                spend::stream_chunk_has_candidate_output(&chunk);
                            if let Some(usage) = &chunk.usage {
                                final_usage = Some(usage.clone());
                            }
                            if let Some(u) = &final_usage {
                                in_tokens = u.prompt_tokens;
                                out_tokens = u.completion_tokens;
                                saw_upstream_output = true;
                            } else {
                                in_tokens = 0;
                                out_tokens = 0;
                            }
                            if chunk.choices.is_empty() && chunk.usage.is_none() {
                                continue;
                            }
                            saw_upstream_output |= has_candidate_output;
                            for choice in &chunk.choices {
                                if let Some(r) = &choice.finish_reason {
                                    final_status = finish_reason_enum_to_status(Some(r));
                                }

                                if let Some(thinking) = &choice.delta.thinking
                                    && let Some(reasoning_text) = thinking.content.as_deref()
                                    && !reasoning_text.is_empty()
                                {
                                    flush_guardrail!();
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

                                        if let Err(error) = emit(
                                            &tx,
                                            &ResponseStreamEvent::ResponseOutputItemAdded {
                                                output_index: reasoning_output_index,
                                                item: in_progress_reasoning_item(
                                                    reasoning_item_id.clone(),
                                                ),
                                            },
                                        )
                                        .await
                                        {
                                            return_after_emit_error!(error);
                                        }
                                    }
                                    full_reasoning.push_str(reasoning_text);
                                    if let Err(error) = emit(
                                        &tx,
                                        &ResponseStreamEvent::ResponseReasoningSummaryTextDelta {
                                            output_index: reasoning_output_index,
                                            summary_index: 0,
                                            delta: reasoning_text.to_string(),
                                        },
                                    )
                                    .await
                                    {
                                        return_after_emit_error!(error);
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
                                        if let Err(error) = emit(
                                            &tx,
                                            &ResponseStreamEvent::ResponseOutputItemAdded {
                                                output_index: text_output_index,
                                                item: placeholder,
                                            },
                                        )
                                        .await
                                        {
                                            return_after_emit_error!(error);
                                        }

                                        if let Err(error) = emit(
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
                                        {
                                            return_after_emit_error!(error);
                                        }
                                    }

                                    full_text.push_str(text);
                                    let event = ResponseStreamEvent::ResponseOutputTextDelta {
                                        output_index: text_output_index,
                                        content_index: 0,
                                        delta: text.to_string(),
                                    };
                                    let encoded = match encode(&event) {
                                        Ok(encoded) => encoded,
                                        Err(error) => return_after_emit_error!(error),
                                    };
                                    let pending = match output_guardrail
                                        .push_until_closed(&tx, text, encoded)
                                        .await
                                    {
                                        Ok(Some(pending)) => pending,
                                        Ok(None) => return_after_disconnect!(),
                                        Err(error) => return_after_guardrail_error!(error),
                                    };
                                    for encoded in pending {
                                        if let Err(error) = send_encoded(&tx, encoded).await {
                                            return_after_emit_error!(error);
                                        }
                                    }
                                }

                                if let Some(tc_deltas) = &choice.delta.tool_calls {
                                    if !tc_deltas.is_empty() {
                                        flush_guardrail!();
                                        saw_upstream_output = true;
                                    }
                                    for tc in tc_deltas {
                                        let idx = tc.index;

                                        if let (
                                            Some(call_id),
                                            std::collections::hash_map::Entry::Vacant(entry),
                                        ) = (&tc.id, tool_states.entry(idx))
                                        {
                                            let out_idx = next_output_index;
                                            next_output_index += 1;
                                            let name = tc
                                                .function
                                                .as_ref()
                                                .and_then(|f| f.name.as_deref())
                                                .unwrap_or("")
                                                .to_string();
                                            let custom = is_custom_tool(&guarded_request, &name);
                                            let item_id = format!(
                                                "{}_{}",
                                                if custom { "ct" } else { "fc" },
                                                uuid_v4_hex()
                                            );

                                            let state = ToolCallAccum::new(
                                                item_id,
                                                call_id.clone(),
                                                name,
                                                out_idx,
                                                custom,
                                            );
                                            if let Err(error) = emit(
                                                &tx,
                                                &ResponseStreamEvent::ResponseOutputItemAdded {
                                                    output_index: out_idx,
                                                    item: state.output_item("in_progress"),
                                                },
                                            )
                                            .await
                                            {
                                                return_after_emit_error!(error);
                                            }

                                            entry.insert(state);
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
                                                state.custom = is_custom_tool(&guarded_request, n);
                                            }
                                            if let Some(args) = &fn_delta.arguments
                                                && !args.is_empty()
                                            {
                                                state.arguments.push_str(args);
                                                if let Some(event) = state.delta_event(args.clone())
                                                    && let Err(error) = emit(&tx, &event).await
                                                {
                                                    return_after_emit_error!(error);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            flush_guardrail!();
                            error!("Responses API stream error: {e}");
                            let (et, ec) = classify(&e);
                            let _ = tx.send(sse_error(&e.to_string(), et, ec)).await;
                            if let Some(lease) = lease.take() {
                                lease.finish_failure(&e);
                            }
                            callback.fail(e.to_string(), "provider_error");
                            settle_if_chargeable!();
                            return;
                        }
                    }
                }
                flush_guardrail!();
                let item_status = final_status;
                let mut all_output: Vec<(u32, ResponseOutputItem)> = Vec::new();

                if reasoning_started {
                    if let Err(error) = emit(
                        &tx,
                        &ResponseStreamEvent::ResponseReasoningSummaryTextDone {
                            output_index: reasoning_output_index,
                            summary_index: 0,
                            text: full_reasoning.clone(),
                        },
                    )
                    .await
                    {
                        return_after_emit_error!(error);
                    }

                    let reasoning_done =
                        completed_reasoning_item(reasoning_item_id, item_status, full_reasoning);
                    if let Err(error) = emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputItemDone {
                            output_index: reasoning_output_index,
                            item: reasoning_done.clone(),
                        },
                    )
                    .await
                    {
                        return_after_emit_error!(error);
                    }
                    all_output.push((reasoning_output_index, reasoning_done));
                }

                if text_started {
                    if let Err(error) = emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputTextDone {
                            output_index: text_output_index,
                            content_index: 0,
                            text: full_text.clone(),
                        },
                    )
                    .await
                    {
                        return_after_emit_error!(error);
                    }

                    if let Err(error) = emit(
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
                    {
                        return_after_emit_error!(error);
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
                    if let Err(error) = emit(
                        &tx,
                        &ResponseStreamEvent::ResponseOutputItemDone {
                            output_index: text_output_index,
                            item: text_done.clone(),
                        },
                    )
                    .await
                    {
                        return_after_emit_error!(error);
                    }
                    all_output.push((text_output_index, text_done));
                }

                for idx in &tool_order {
                    if let Some(state) = tool_states.get(idx) {
                        for event in state.done_events() {
                            if let Err(error) = emit(&tx, &event).await {
                                return_after_emit_error!(error);
                            }
                        }

                        let fc_done = state.output_item("completed");
                        if let Err(error) = emit(
                            &tx,
                            &ResponseStreamEvent::ResponseOutputItemDone {
                                output_index: state.output_index,
                                item: fc_done.clone(),
                            },
                        )
                        .await
                        {
                            return_after_emit_error!(error);
                        }
                        all_output.push((state.output_index, fc_done));
                    }
                }

                let output_items = output_items_in_stream_order(all_output);

                let (total, budget_usage) =
                    response_stream_budget_usage(final_usage.clone(), in_tokens, out_tokens);
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
                    previous_response_id: guarded_request.previous_response_id.clone(),
                    metadata: guarded_request.metadata.clone(),
                };
                if let Err(error) = emit(
                    &tx,
                    &ResponseStreamEvent::ResponseCompleted {
                        response: Box::new(completed.clone()),
                    },
                )
                .await
                {
                    return_after_emit_error!(error);
                }

                if tx
                    .send(Event::default().data("[DONE]").to_bytes())
                    .await
                    .is_err()
                {
                    return_after_disconnect!();
                }

                store_response_if_requested(&guarded_request, &completed, owner);
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
