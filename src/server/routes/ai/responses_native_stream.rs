use actix_web::HttpResponse;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::{
    AppState, MAX_RESPONSE_BYTES, NativeCall, ProviderError, RequestContext, response_usage, settle,
};
use crate::server::guardrails::{GuardrailDecisionSink, native_responses_projection};
use crate::server::routes::ai::execution::StreamingDeploymentLease;
use crate::server::routes::ai::stream_output_guardrail::StreamOutputGuardrail;

pub(super) fn response(
    state: AppState,
    context: RequestContext,
    call: NativeCall,
    mut lease: StreamingDeploymentLease,
) -> HttpResponse {
    let (tx, rx) = mpsc::channel::<Bytes>(8);
    let facts = crate::core::request_ledger::current_facts();
    tokio::spawn(async move {
        let NativeCall {
            callback,
            response,
            provider,
            model,
            deployment,
            pricing,
            file_search_calls,
            reservation,
            key_reservation,
            storage,
            started,
        } = call;
        let background = storage
            .as_ref()
            .is_some_and(|storage| storage.is_background());
        let StreamResult {
            response,
            storage,
            usage,
            pricing_usage,
            terminal,
            upstream_failed,
            failure,
        } = forward(
            &state,
            &tx,
            response,
            storage,
            &provider,
            Some(&deployment),
            None,
            background.then_some(started + std::time::Duration::from_secs(540)),
            file_search_calls,
        )
        .await;
        if background
            && !terminal
            && let Some(record) = storage.as_ref().and_then(|storage| storage.record())
        {
            if let Some(error) = failure.as_ref() {
                let code = if provider_error_is_guardrail(error) {
                    "guardrail_violation"
                } else {
                    "upstream_stream_error"
                };
                let event =
                    serde_json::json!({"type":"error", "code":code, "message":error.to_string()});
                let _ = tx
                    .send(Bytes::from(format!("event: error\ndata: {event}\n\n")))
                    .await;
            }
            let initial = serde_json::from_str(&record.response_json).map_err(Into::into);
            let call = NativeCall {
                callback,
                response,
                provider,
                model,
                deployment,
                pricing,
                file_search_calls,
                reservation,
                key_reservation,
                storage,
                started,
            };
            // The client or upstream SSE connection can end while native work
            // continues. Transfer the SAME reservations to the creation's poller.
            let _ = super::background::response(state, context, call, lease, initial, true, facts)
                .await;
            return;
        }
        let tokens_used = usage.as_ref().map_or(0, |u| u64::from(u.total_tokens));
        let terminal_error = failure.clone().or_else(|| {
            upstream_failed
                .then(|| ProviderError::api_error("responses", 502, "Upstream response failed"))
        });
        let settlement = async {
            // A disconnect or malformed usage never releases a possibly consumed reservation.
            if let Some(id) = storage
                .as_ref()
                .and_then(|storage| storage.settlement_id.as_ref())
            {
                if let Err(error) =
                    super::super::responses_settlement::submit_usage(&state, id, usage.as_ref())
                        .await
                {
                    tracing::error!(%error, "Response stream settlement remains pending for recovery");
                }
            } else {
                settle(
                    &state,
                    &context,
                    &provider,
                    &model,
                    pricing,
                    usage.as_ref(),
                    pricing_usage.clone(),
                    reservation,
                    key_reservation,
                    facts,
                )
                .await;
            }
        };
        if let Some(error) = terminal_error.as_ref() {
            lease
                .settle_terminal(tokens_used, Some(error), settlement)
                .await;
        } else if terminal {
            lease.settle_terminal(tokens_used, None, settlement).await;
        } else {
            lease
                .settle_interrupted(tokens_used, None, settlement)
                .await;
        }
        if let Some(error) = failure {
            callback.fail(error.to_string(), "stream_error");
            lease.finish_failure_with_tokens(&error, tokens_used).await;
            let code = if provider_error_is_guardrail(&error) {
                "guardrail_violation"
            } else {
                "upstream_stream_error"
            };
            let event =
                serde_json::json!({"type":"error", "code":code, "message":error.to_string()});
            let _ = tx
                .send(Bytes::from(format!("event: error\ndata: {event}\n\n")))
                .await;
        } else if upstream_failed {
            callback.fail("Upstream response failed", "provider_error");
            lease
                .finish_failure_with_tokens(
                    &ProviderError::api_error("responses", 502, "Upstream response failed"),
                    tokens_used,
                )
                .await;
        } else if terminal {
            callback.complete_pricing_usage(usage.as_ref(), pricing_usage.as_ref(), "success");
            lease
                .finish_success(
                    usage
                        .as_ref()
                        .map_or(0, |usage| u64::from(usage.total_tokens)),
                )
                .await;
        } else {
            callback.fail("Client disconnected", "client_disconnect");
        }
    });
    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .insert_header(("cache-control", "no-cache"))
        .streaming(ReceiverStream::new(rx).map(Ok::<_, actix_web::Error>))
}

struct StreamResult {
    response: reqwest::Response,
    storage: Option<super::NativeResponseStorage>,
    usage: Option<super::Usage>,
    pricing_usage: Option<crate::core::pricing_service::PricingUsage>,
    terminal: bool,
    upstream_failed: bool,
    failure: Option<ProviderError>,
}

#[allow(clippy::too_many_arguments)]
async fn forward(
    state: &AppState,
    tx: &mpsc::Sender<Bytes>,
    mut response: reqwest::Response,
    mut storage: Option<super::NativeResponseStorage>,
    provider: &str,
    deployment: Option<&str>,
    expected_id: Option<&str>,
    deadline: Option<tokio::time::Instant>,
    file_search_calls: Option<u32>,
) -> StreamResult {
    let mut frames = Frames::default();
    let mut usage = None;
    let mut pricing_usage = None;
    let mut terminal = false;
    let mut upstream_failed = false;
    let mut failure = None;
    let sink = GuardrailDecisionSink::from_state(state, None, Some(provider), deployment);
    let mut guard = StreamOutputGuardrail::new(state.guardrails()).with_decision_sink(sink);
    let idle_seconds = state.config().gateway.server.stream_idle_timeout;
    'upstream: loop {
        let chunk = tokio::select! {
            biased;
            _ = tx.closed() => break,
            _ = async { match deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending::<()>().await } } => {
                failure = Some(ProviderError::timeout("responses", "Background Responses stream tracking timed out")); break;
            },
            chunk = async {
                if idle_seconds == 0 { Ok(response.chunk().await) }
                else { tokio::time::timeout(std::time::Duration::from_secs(idle_seconds), response.chunk()).await }
            } => match chunk {
                Ok(chunk) => chunk,
                Err(_) => { failure = Some(ProviderError::timeout("responses", "Responses stream idle timeout")); break; }
            },
        };
        match chunk {
            Ok(Some(chunk)) => {
                if let Err(error) = frames.push(&chunk) {
                    failure = Some(error);
                    break;
                }
                while let Some(frame) = frames.next() {
                    let value = match event_value(&frame) {
                        Ok(value) => value,
                        Err(error) => {
                            failure = Some(error);
                            break 'upstream;
                        }
                    };
                    let created = value
                        .as_ref()
                        .and_then(|value| value.get("type"))
                        .and_then(Value::as_str)
                        == Some("response.created");
                    let mut surfaces = Vec::new();
                    if let Some(value) = value {
                        if let (Some(expected), Some(id)) = (
                            expected_id,
                            value.pointer("/response/id").and_then(Value::as_str),
                        ) && id != expected
                        {
                            failure = Some(invalid("Responses stream returned a different ID"));
                            break 'upstream;
                        }
                        let event = value
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if matches!(
                            event,
                            "response.completed"
                                | "response.incomplete"
                                | "response.failed"
                                | "response.cancelled"
                        ) {
                            terminal = true;
                            upstream_failed = event == "response.failed";
                            usage = value.get("response").and_then(response_usage);
                            pricing_usage = value.get("response").zip(usage.as_ref()).and_then(
                                |(value, usage)| {
                                    super::request::pricing_usage(value, usage, file_search_calls)
                                },
                            );
                        } else if event == "error" {
                            terminal = true;
                            upstream_failed = true;
                        }
                        if matches!(
                            event,
                            "response.created"
                                | "response.completed"
                                | "response.incomplete"
                                | "response.failed"
                                | "response.cancelled"
                        ) && let Some(storage) = storage.as_mut()
                        {
                            let Some(response) = value.get("response") else {
                                failure = Some(invalid("Response event is missing its response"));
                                break 'upstream;
                            };
                            if let Err(error) =
                                storage.save(&state.storage.database, response).await
                            {
                                tracing::error!("Native response could not be stored: {error}");
                                failure = Some(ProviderError::api_error(
                                    "responses",
                                    500,
                                    "Native response storage failed",
                                ));
                                break 'upstream;
                            }
                        }
                        // Keep each output's delta sequence contiguous for split-token checks.
                        if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                            let index = value
                                .get("output_index")
                                .and_then(Value::as_u64)
                                .unwrap_or(0);
                            let Ok(index) = u32::try_from(index) else {
                                failure = Some(invalid("Invalid output index"));
                                break 'upstream;
                            };
                            surfaces.push((index, delta.to_string()));
                        }
                        // Inspect all native and future fields too; raw event bytes are retained.
                        surfaces.push((u32::MAX, native_responses_projection(&value)));
                    }
                    match guard.push_many_until_closed(tx, surfaces, frame).await {
                        Ok(Some(events)) => {
                            for event in events {
                                if tx.send(event).await.is_err() {
                                    break 'upstream;
                                }
                            }
                        }
                        Ok(None) => break 'upstream,
                        Err(error) => {
                            failure =
                                Some(ProviderError::api_error("guardrail", 403, error.message()));
                            break 'upstream;
                        }
                    }
                    if created {
                        // Release the authorized handle promptly even when later chunks
                        // never arrive; check its full frame before making it resumable.
                        match guard.flush_to_until_closed(tx).await {
                            Ok(Some(_)) => {}
                            Ok(None) => break 'upstream,
                            Err(error) => {
                                failure = Some(ProviderError::api_error(
                                    "guardrail",
                                    403,
                                    error.message(),
                                ));
                                break 'upstream;
                            }
                        }
                    }
                    if terminal {
                        break 'upstream;
                    }
                }
            }
            Err(_) => {
                failure = Some(ProviderError::network(
                    "responses",
                    "Responses stream interrupted",
                ));
                break;
            }
            Ok(None) => {
                if !terminal {
                    failure = Some(invalid("Responses stream ended before a terminal event"));
                }
                break;
            }
        }
    }
    if failure.is_none() && terminal {
        match guard.finish_until_closed(tx).await {
            Ok(Some(events)) => {
                for event in events {
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                failure = Some(ProviderError::api_error("guardrail", 403, error.message()));
            }
        }
    }

    StreamResult {
        response,
        storage,
        usage,
        pricing_usage,
        terminal,
        upstream_failed,
        failure,
    }
}

/// Replays upstream events through guardrails without reserving or recording spend.
pub(super) fn resume(
    state: AppState,
    response: reqwest::Response,
    id: String,
    provider: String,
    deployment: Option<String>,
) -> HttpResponse {
    let (tx, rx) = mpsc::channel::<Bytes>(8);
    tokio::spawn(async move {
        let result = forward(
            &state,
            &tx,
            response,
            None,
            &provider,
            deployment.as_deref(),
            Some(&id),
            None,
            None,
        )
        .await;
        if let Some(error) = result.failure {
            let code = if provider_error_is_guardrail(&error) {
                "guardrail_violation"
            } else {
                "upstream_stream_error"
            };
            let event =
                serde_json::json!({"type":"error", "code":code, "message":error.to_string()});
            let _ = tx
                .send(Bytes::from(format!("event: error\ndata: {event}\n\n")))
                .await;
        }
    });
    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .insert_header(("cache-control", "no-cache"))
        .streaming(ReceiverStream::new(rx).map(Ok::<_, actix_web::Error>))
}

fn provider_error_is_guardrail(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::ApiError {
            provider: "guardrail",
            ..
        }
    )
}

fn invalid(message: &str) -> ProviderError {
    ProviderError::response_parsing("responses", message)
}

#[derive(Default)]
struct Frames {
    pending: Vec<u8>,
}

impl Frames {
    fn push(&mut self, bytes: &[u8]) -> Result<(), ProviderError> {
        if self.pending.len().saturating_add(bytes.len()) > MAX_RESPONSE_BYTES {
            return Err(invalid("Responses SSE buffer exceeds size limit"));
        }
        self.pending.extend_from_slice(bytes);
        Ok(())
    }
    fn next(&mut self) -> Option<Bytes> {
        let lf = self
            .pending
            .windows(2)
            .position(|part| part == b"\n\n")
            .map(|position| position + 2);
        let crlf = self
            .pending
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .map(|position| position + 4);
        let end = lf.into_iter().chain(crlf).min()?;
        Some(Bytes::from(self.pending.drain(..end).collect::<Vec<_>>()))
    }
}

fn event_value(frame: &[u8]) -> Result<Option<Value>, ProviderError> {
    let text = std::str::from_utf8(frame).map_err(|_| invalid("Invalid Responses SSE UTF-8"))?;
    let data = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("data:")
                .map(|data| data.strip_prefix(' ').unwrap_or(data))
        })
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        return Ok(None);
    }
    serde_json::from_str(&data)
        .map(Some)
        .map_err(|_| invalid("Invalid Responses SSE JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_preserve_utf8_event_names_and_multiline_data() {
        let wire = "event: response.output_text.delta\r\ndata: {\"type\":\"response.output_text.delta\",\r\ndata: \"delta\":\"你好\"}\r\n\r\n";
        let mut frames = Frames::default();
        for byte in wire.as_bytes() {
            frames.push(&[*byte]).unwrap();
        }
        let frame = frames.next().unwrap();
        assert_eq!(frame.as_ref(), wire.as_bytes());
        assert_eq!(event_value(&frame).unwrap().unwrap()["delta"], "你好");
        assert!(frames.next().is_none());
    }
    #[test]
    fn malformed_frames_are_not_silently_dropped() {
        assert!(event_value(b"data: not json\n\n").is_err());
        assert!(event_value(b"data: \xff\n\n").is_err());
        assert!(event_value(b": heartbeat\n\n").unwrap().is_none());
    }
}
