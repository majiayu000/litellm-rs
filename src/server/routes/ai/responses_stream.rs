//! Streaming handler for POST /v1/responses
//!
//! Translates internal `ChatChunk` SSE events into Responses API streaming
//! events as defined in the OpenAI Responses API specification.

use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseFunctionCall, ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage,
    ResponseStreamEvent, ResponseUsage, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::{context::RequestContext, model::ProviderCapability};
use crate::server::routes::ai::chat::build_core_chat_request;
use crate::server::routes::ai::execution::execute_with_selected_deployment;
use crate::server::routes::ai::responses::{
    current_unix_ts, finish_reason_enum_to_status, uuid_v4_hex,
};
use crate::server::state::AppState;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, ResponseError, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

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
    context: RequestContext,
) -> ActixResult<HttpResponse> {
    info!(
        "Streaming Responses API request for model: {}",
        chat_request.model
    );

    let unified_router = &state.unified_router;

    if let Err(e) = super::provider_selection::select_provider_for_model(
        unified_router,
        &chat_request.model,
        ProviderCapability::ChatCompletionStream,
    ) {
        return Ok(e.error_response());
    }

    chat_request.stream = Some(true);
    let model_name = chat_request.model.clone();
    let resp_id = format!("resp_{}", uuid_v4_hex());
    let created_at = current_unix_ts();

    let core_request = match build_core_chat_request(chat_request, model_name.clone(), true) {
        Ok(r) => r,
        Err(e) => {
            use actix_web::ResponseError;
            return Ok(e.error_response());
        }
    };

    let requested_model = core_request.model.clone();
    let context_clone = context.clone();

    match execute_with_selected_deployment(
        unified_router,
        &requested_model,
        move |provider, selected_model| {
            let core_request = core_request.clone();
            let ctx = context_clone.clone();
            async move {
                let mut req = core_request.clone();
                req.model = selected_model;
                let stream = provider.chat_completion_stream(req, ctx).await?;
                Ok((stream, 0))
            }
        },
    )
    .await
    {
        Ok(mut stream) => {
            let (tx, rx) = mpsc::channel::<Bytes>(8);
            let idle_timeout = state.config.load().gateway.server.stream_idle_timeout;

            tokio::spawn(async move {
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
                let mut next_output_index: u32 = 0;
                let mut final_status: &'static str = "completed";
                // Tool calls keyed by streaming index
                let mut tool_states: HashMap<u32, ToolCallAccum> = HashMap::new();
                // Preserves insertion order for final iteration
                let mut tool_order: Vec<u32> = Vec::new();

                // ── text and tool-call deltas ─────────────────────────────────
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
                                return;
                            }
                        }
                    };

                    let Some(result) = next else { break };

                    match result {
                        Ok(chunk) => {
                            if let Some(u) = &chunk.usage {
                                in_tokens = u.prompt_tokens;
                                out_tokens = u.completion_tokens;
                            }
                            for choice in &chunk.choices {
                                if let Some(r) = &choice.finish_reason {
                                    final_status = finish_reason_enum_to_status(Some(r));
                                }

                                // ── text content ──────────────────────────────
                                let text = choice.delta.content.as_deref().unwrap_or("");
                                if !text.is_empty() {
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
                                            return;
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
                                            return;
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
                                        return;
                                    }
                                }

                                // ── tool-call deltas ──────────────────────────
                                if let Some(tc_deltas) = &choice.delta.tool_calls {
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
                                                return;
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
                                                    return;
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
                            return;
                        }
                    }
                }

                let item_status = final_status;
                let mut all_output: Vec<(u32, ResponseOutputItem)> = Vec::new();

                // ── text done events ──────────────────────────────────────────
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
                        return;
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
                        return;
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
                        return;
                    }
                    all_output.push((text_output_index, text_done));
                }

                // ── tool-call done events ─────────────────────────────────────
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
                            return;
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
                            return;
                        }
                        all_output.push((state.output_index, fc_done));
                    }
                }

                // Sort by output_index to preserve stream order
                all_output.sort_by_key(|(i, _)| *i);
                let output_items: Vec<ResponseOutputItem> =
                    all_output.into_iter().map(|(_, item)| item).collect();

                // ── response.completed ────────────────────────────────────────
                let total = in_tokens + out_tokens;
                let usage = (total > 0).then_some(ResponseUsage {
                    input_tokens: in_tokens,
                    output_tokens: out_tokens,
                    total_tokens: total,
                    input_tokens_details: None,
                    output_tokens_details: None,
                });
                let completed = ResponsesApiResponse {
                    id: resp_id,
                    object: "response".to_string(),
                    created_at,
                    status: item_status.to_string(),
                    model: model_name,
                    output: output_items,
                    usage,
                    error: None,
                    previous_response_id: original.previous_response_id,
                    metadata: original.metadata,
                };
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
            Ok(e.error_response())
        }
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

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
mod tests {
    use super::*;

    #[test]
    fn test_sse_error_contains_done() {
        let b = sse_error("oops", "server_error", "internal_error");
        let s = String::from_utf8(b.to_vec()).unwrap();
        assert!(s.contains("data: {"));
        assert!(s.contains("[DONE]"));
        assert!(s.contains("oops"));
    }

    #[test]
    fn test_classify_auth_error() {
        let e = ProviderError::Authentication {
            provider: "openai",
            message: "bad key".to_string(),
        };
        let (t, c) = classify(&e);
        assert_eq!(t, "invalid_request_error");
        assert_eq!(c, "authentication_error");
    }

    #[test]
    fn test_classify_timeout() {
        let e = ProviderError::Timeout {
            provider: "openai",
            message: "timed out".to_string(),
        };
        let (t, c) = classify(&e);
        assert_eq!(t, "server_error");
        assert_eq!(c, "timeout");
    }
}
