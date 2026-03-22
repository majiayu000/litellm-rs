//! Streaming handler for POST /v1/responses
//!
//! Translates internal `ChatChunk` SSE events into Responses API streaming
//! events as defined in the OpenAI Responses API specification.

use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseOutputContent, ResponseOutputItem, ResponseOutputMessage, ResponseStreamEvent,
    ResponseUsage, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::{context::RequestContext, model::ProviderCapability};
use crate::server::routes::ai::chat::build_core_chat_request;
use crate::server::routes::ai::responses::{current_unix_ts, uuid_v4_hex};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use actix_web::{HttpResponse, ResponseError, Result as ActixResult};
use bytes::Bytes;
use futures::StreamExt;
use serde_json::json;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

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

    match unified_router
        .execute_with_retry(&requested_model, move |deployment_id| {
            let core_request = core_request.clone();
            let ctx = context_clone.clone();
            async move {
                let deployment =
                    unified_router
                        .get_deployment(&deployment_id)
                        .ok_or_else(|| {
                            ProviderError::other("router", "Selected deployment not found")
                        })?;
                let provider = deployment.provider.clone();
                let selected_model = deployment.model.clone();
                drop(deployment);
                let mut req = core_request.clone();
                req.model = selected_model;
                let stream = provider.chat_completion_stream(req, ctx).await?;
                Ok((stream, 0))
            }
        })
        .await
    {
        Ok((mut stream, _, _, _)) => {
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

                // ── response.output_item.added ────────────────────────────────
                let item_id = format!("msg_{}", uuid_v4_hex());
                let placeholder = ResponseOutputItem::Message(ResponseOutputMessage {
                    id: item_id.clone(),
                    role: "assistant".to_string(),
                    status: "in_progress".to_string(),
                    content: vec![],
                });
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseOutputItemAdded {
                        output_index: 0,
                        item: placeholder,
                    },
                )
                .await
                .is_err()
                {
                    return;
                }

                // ── response.content_part.added ───────────────────────────────
                let empty_part = ResponseOutputContent::OutputText {
                    text: String::new(),
                    annotations: None,
                    logprobs: None,
                };
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseContentPartAdded {
                        output_index: 0,
                        content_index: 0,
                        part: empty_part,
                    },
                )
                .await
                .is_err()
                {
                    return;
                }

                // ── text deltas ───────────────────────────────────────────────
                let mut full_text = String::new();
                let mut in_tokens: u32 = 0;
                let mut out_tokens: u32 = 0;

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
                                let text = choice.delta.content.as_deref().unwrap_or("");
                                if text.is_empty() {
                                    continue;
                                }
                                full_text.push_str(text);
                                if emit(
                                    &tx,
                                    &ResponseStreamEvent::ResponseOutputTextDelta {
                                        output_index: 0,
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
                        }
                        Err(e) => {
                            error!("Responses API stream error: {e}");
                            let (et, ec) = classify(&e);
                            let _ = tx.send(sse_error(&e.to_string(), et, ec)).await;
                            return;
                        }
                    }
                }

                // ── response.output_text.done ─────────────────────────────────
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseOutputTextDone {
                        output_index: 0,
                        content_index: 0,
                        text: full_text.clone(),
                    },
                )
                .await
                .is_err()
                {
                    return;
                }

                // ── response.content_part.done ────────────────────────────────
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseContentPartDone {
                        output_index: 0,
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

                // ── response.output_item.done ─────────────────────────────────
                let done_item = ResponseOutputItem::Message(ResponseOutputMessage {
                    id: item_id,
                    role: "assistant".to_string(),
                    status: "completed".to_string(),
                    content: vec![ResponseOutputContent::OutputText {
                        text: full_text.clone(),
                        annotations: None,
                        logprobs: None,
                    }],
                });
                if emit(
                    &tx,
                    &ResponseStreamEvent::ResponseOutputItemDone {
                        output_index: 0,
                        item: done_item.clone(),
                    },
                )
                .await
                .is_err()
                {
                    return;
                }

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
                    status: "completed".to_string(),
                    model: model_name,
                    output: vec![done_item],
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
        Err((e, _)) => {
            error!("Failed to start Responses API stream: {e}");
            Ok(GatewayError::Provider(e).error_response())
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
