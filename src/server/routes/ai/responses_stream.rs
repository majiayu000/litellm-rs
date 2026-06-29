//! Streaming handler for POST /v1/responses
//!
//! Translates internal `ChatChunk` SSE events into Responses API streaming
//! events as defined in the OpenAI Responses API specification.

use crate::core::budget::{UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::models::openai::requests::{ChatCompletionRequest, StreamOptions};
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseInputTokensDetails, ResponseOutputContent, ResponseOutputItem,
    ResponseOutputMessage, ResponseOutputTokensDetails, ResponseReasoningItem, ResponseStreamEvent,
    ResponseUsage, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::pricing_service::PricingService;
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::responses::Usage as ChatUsage;
use crate::core::types::{context::RequestContext, model::ProviderCapability};
use crate::server::routes::ai::chat::build_core_chat_request;
use crate::server::routes::ai::execution::execute_stream_with_selected_deployment;
use crate::server::routes::ai::responses::{
    ResponseOwner, current_unix_ts, finish_reason_enum_to_status, store_response_if_requested,
    uuid_v4_hex,
};
use crate::server::state::AppState;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::{openai_errors, spend};

/// Accumulated state for one in-progress tool call during streaming.
struct ToolCallAccum {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    output_index: u32,
}

struct StreamBudgetSettlement {
    pricing_service: Arc<PricingService>,
    budget_limits: Arc<UnifiedBudgetLimits>,
    key_manager: KeyManager,
    api_key_id: Option<Uuid>,
    provider: String,
    model: String,
    reservation: Option<UnifiedBudgetReservation>,
}

impl StreamBudgetSettlement {
    async fn record_completion(mut self, usage: Option<&ChatUsage>, saw_upstream_output: bool) {
        spend::record_finished_stream_spend_with_reservation_with_pricing(
            self.pricing_service.as_ref(),
            spend::StreamSpendSettlement {
                budget_limits: self.budget_limits.as_ref(),
                key_manager: &self.key_manager,
                api_key_id: self.api_key_id,
                provider: &self.provider,
                model: &self.model,
                usage,
                saw_upstream_output,
                budget_reservation: self.reservation.take(),
            },
        )
        .await;
    }

    async fn record_disconnect(&mut self, usage: Option<&ChatUsage>) {
        spend::record_stream_disconnect_spend_with_reservation_with_pricing(
            self.pricing_service.as_ref(),
            self.budget_limits.as_ref(),
            &self.key_manager,
            self.api_key_id,
            &self.provider,
            &self.model,
            usage,
            self.reservation.take(),
        )
        .await;
    }
}

/// Streaming path for POST /v1/responses.
pub(crate) async fn handle_streaming_response(
    state: &AppState,
    mut chat_request: ChatCompletionRequest,
    original: ResponsesApiRequest,
    context: RequestContext,
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
    let request_for_budget = chat_request.clone();
    let model_name = chat_request.model.clone();
    let resp_id = format!("resp_{}", uuid_v4_hex());
    let created_at = current_unix_ts();

    let core_request = match build_core_chat_request(chat_request, model_name.clone(), true) {
        Ok(r) => r,
        Err(e) => {
            return Ok(openai_errors::gateway_error_response(&e));
        }
    };

    let requested_model = core_request.model.clone();
    let context_clone = context.clone();
    let budget_limits = state.budget_limits.clone();
    let pricing_service = state.pricing.clone();
    let api_key_id = context.api_key_id();

    match execute_stream_with_selected_deployment(
        state.unified_router.clone(),
        &requested_model,
        ProviderCapability::ChatCompletionStream,
        move |provider, selected_model| {
            let core_request = core_request.clone();
            let ctx = context_clone.clone();
            let budget_limits = budget_limits.clone();
            let pricing_service = pricing_service.clone();
            let request_for_budget = request_for_budget.clone();
            async move {
                spend::ensure_budget_available(&budget_limits, provider.name(), &selected_model)?;
                let budget_reservation = spend::reserve_chat_completion_budget_with_pricing(
                    pricing_service.as_ref(),
                    &budget_limits,
                    provider.name(),
                    &selected_model,
                    &request_for_budget,
                )?;
                let provider_name = provider.name().to_string();
                let mut req = core_request.clone();
                req.model = selected_model.clone();
                let stream = provider.chat_completion_stream(req, ctx).await?;
                Ok((stream, provider_name, selected_model, budget_reservation))
            }
        },
    )
    .await
    {
        Ok(((mut stream, served_provider, served_model, budget_reservation), lease)) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout = state.config.load().gateway.server.stream_idle_timeout;
            let settlement = StreamBudgetSettlement {
                pricing_service: state.pricing.clone(),
                budget_limits: state.budget_limits.clone(),
                key_manager: state.key_manager.clone(),
                api_key_id,
                provider: served_provider,
                model: served_model,
                reservation: budget_reservation,
            };

            tokio::spawn(async move {
                let mut lease = Some(lease);
                let mut settlement = settlement;

                // ── response.created ──────────────────────────────────────────
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
                    return;
                }

                // ── streaming state ───────────────────────────────────────────
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
                // Tool calls keyed by streaming index
                let mut tool_states: HashMap<u32, ToolCallAccum> = HashMap::new();
                // Preserves insertion order for final iteration
                let mut tool_order: Vec<u32> = Vec::new();
                // Reasoning-summary state (o-series / extended thinking / DeepSeek R1 / Gemini).
                //
                // Output-index ordering invariant: the OpenAI Responses API requires
                // reasoning items to have a lower `output_index` than the text item.
                // We rely on upstream providers to emit reasoning chunks before text
                // chunks (OpenAI o-series, Anthropic extended thinking, DeepSeek R1,
                // and Gemini thinking all conform to this protocol convention). The
                // first-arrival assignment below preserves canonical order under that
                // assumption. If a provider ever violates the convention, a `warn!`
                // is emitted so operators can detect the inversion.
                let mut full_reasoning = String::new();
                let mut reasoning_item_id = String::new();
                let mut reasoning_output_index: u32 = 0;
                let mut reasoning_started = false;
                macro_rules! return_after_disconnect {
                    () => {
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
                                            // Protocol violation: reasoning arrived after
                                            // text claimed an output_index. The Responses API
                                            // contract (reasoning at lower output_index than
                                            // text) is broken for this stream. Continue
                                            // emitting in arrival order; flag for ops.
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

                // Final SSE terminator
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
            Ok(openai_errors::gateway_error_response(&e))
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn completed_reasoning_item(
    item_id: String,
    status: &str,
    summary_text: String,
) -> ResponseOutputItem {
    ResponseOutputItem::Reasoning(ResponseReasoningItem {
        id: item_id,
        status: status.to_string(),
        summary: Some(vec![json!({
            "type": "summary_text",
            "text": summary_text,
        })]),
    })
}

fn in_progress_reasoning_item(item_id: String) -> ResponseOutputItem {
    ResponseOutputItem::Reasoning(ResponseReasoningItem {
        id: item_id,
        status: "in_progress".to_string(),
        summary: Some(vec![]),
    })
}

fn output_items_in_stream_order(
    mut all_output: Vec<(u32, ResponseOutputItem)>,
) -> Vec<ResponseOutputItem> {
    all_output.sort_by_key(|(i, _)| *i);
    all_output.into_iter().map(|(_, item)| item).collect()
}

fn response_usage_from_chat_usage(usage: &ChatUsage) -> ResponseUsage {
    ResponseUsage {
        input_tokens: usage.prompt_tokens,
        output_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        input_tokens_details: usage.prompt_tokens_details.as_ref().map(|d| {
            ResponseInputTokensDetails {
                cached_tokens: d.cached_tokens.unwrap_or(0),
            }
        }),
        output_tokens_details: usage.completion_tokens_details.as_ref().map(|d| {
            ResponseOutputTokensDetails {
                reasoning_tokens: d.reasoning_tokens.unwrap_or(0),
            }
        }),
    }
}

fn make_shell(
    id: &str,
    created_at: i64,
    model: &str,
    status: &str,
    original: &ResponsesApiRequest,
) -> ResponsesApiResponse {
    ResponsesApiResponse {
        id: id.to_string(),
        object: "response".to_string(),
        created_at,
        status: status.to_string(),
        model: model.to_string(),
        output: vec![],
        usage: None,
        error: None,
        previous_response_id: original.previous_response_id.clone(),
        metadata: None,
    }
}

async fn emit(tx: &mpsc::Sender<Bytes>, event: &ResponseStreamEvent) -> Result<(), ()> {
    match serde_json::to_string(event) {
        Ok(json) => tx
            .send(Event::default().data(&json).to_bytes())
            .await
            .map_err(|_| ()),
        Err(e) => {
            error!("Failed to serialise stream event: {e}");
            Err(())
        }
    }
}

fn sse_error(message: &str, error_type: &str, code: &str) -> Bytes {
    let err = json!({"type":"error","error":{"type":error_type,"code":code,"message":message}});
    let err_ev = Event::default().data(&err.to_string());
    let done_ev = Event::default().data("[DONE]");
    let mut v = err_ev.to_bytes().to_vec();
    v.extend_from_slice(&done_ev.to_bytes());
    Bytes::from(v)
}

fn classify(e: &ProviderError) -> (&'static str, &'static str) {
    match e {
        ProviderError::Authentication { .. } => ("invalid_request_error", "authentication_error"),
        ProviderError::RateLimit { .. } => ("rate_limit_error", "rate_limit_exceeded"),
        ProviderError::Timeout { .. } => ("server_error", "timeout"),
        _ => ("server_error", "internal_error"),
    }
}

#[cfg(test)]
#[path = "responses_stream_tests.rs"]
mod tests;
