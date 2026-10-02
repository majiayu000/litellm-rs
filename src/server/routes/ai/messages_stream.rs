use super::{
    AppState, MAX_BODY_BYTES, MessageCall, ProviderError, RequestContext, StreamingDeploymentLease,
    native_usage, settle,
};
use crate::core::providers::base::sse::{AnthropicTransformer, SSETransformer};
use crate::server::guardrails::{GuardrailDecisionSink, messages_projection};
use crate::server::routes::ai::stream_output_guardrail::StreamOutputGuardrail;
use actix_web::HttpResponse;
use bytes::Bytes;
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

pub(super) fn response(
    state: AppState,
    context: RequestContext,
    call: MessageCall,
    lease: StreamingDeploymentLease,
) -> HttpResponse {
    let (tx, rx) = mpsc::channel::<Bytes>(8);
    let facts = crate::core::request_ledger::current_facts();
    tokio::spawn(async move {
        let MessageCall {
            mut response,
            callback,
            provider,
            model,
            deployment,
            pricing,
            reservation,
            key_reservation,
        } = call;
        let sink = GuardrailDecisionSink::from_state(
            &state,
            Some(&model),
            Some(&provider),
            Some(&deployment),
        );
        let mut guard = StreamOutputGuardrail::new(state.guardrails()).with_decision_sink(sink);
        let validator = AnthropicTransformer::new(&model);
        let mut pending = Vec::new();
        let mut usage = json!({});
        let mut started = false;
        let mut terminal = false;
        let mut upstream_failed = false;
        let mut final_usage = false;
        let mut failure = None;
        let idle = state.config().gateway.server.stream_idle_timeout;
        'upstream: loop {
            let chunk = tokio::select! {
                biased;
                _ = tx.closed() => break,
                chunk = async {
                    if idle == 0 { Ok(response.chunk().await) } else {
                        tokio::time::timeout(std::time::Duration::from_secs(idle), response.chunk()).await
                    }
                } => match chunk {
                    Ok(chunk) => chunk,
                    Err(_) => { failure = Some(ProviderError::timeout("anthropic", "Messages stream idle timeout")); break; }
                }
            };
            match chunk {
                Ok(Some(chunk)) => {
                    if pending.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                        failure = Some(invalid("Messages SSE buffer exceeds size limit"));
                        break;
                    }
                    pending.extend_from_slice(&chunk);
                    while let Some(frame) = next_frame(&mut pending) {
                        let value = match event(&frame) {
                            Ok(Some(value)) => value,
                            Ok(None) => {
                                if tx.send(frame).await.is_err() {
                                    break 'upstream;
                                }
                                continue;
                            }
                            Err(error) => {
                                failure = Some(error);
                                break 'upstream;
                            }
                        };
                        let kind = value
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        if kind != "error" {
                            // Reuse the provider's tested lifecycle validation; emit the
                            // original frame, never the transformer's Chat chunk.
                            if let Err(error) = validator.transform_stream_chunk(&value.to_string())
                            {
                                failure = Some(error);
                                break 'upstream;
                            }
                        }
                        match kind {
                            "message_start" => {
                                if started {
                                    failure = Some(invalid("Duplicate message_start"));
                                    break 'upstream;
                                }
                                started = true;
                                usage = value
                                    .pointer("/message/usage")
                                    .cloned()
                                    .unwrap_or(Value::Null);
                            }
                            "message_delta" => {
                                if !started {
                                    failure = Some(invalid("message_delta before message_start"));
                                    break 'upstream;
                                }
                                if let Some(delta) = value.get("usage") {
                                    final_usage = true;
                                    if delta.is_object() {
                                        merge_usage(&mut usage, delta);
                                    } else {
                                        usage = Value::Null;
                                    }
                                }
                            }
                            "message_stop" => {
                                if !started {
                                    failure = Some(invalid("message_stop before message_start"));
                                    break 'upstream;
                                }
                                if !final_usage || native_usage(&usage).is_none() {
                                    failure =
                                        Some(invalid("Messages stream has no valid final usage"));
                                    break 'upstream;
                                }
                                terminal = true;
                            }
                            "error" => {
                                terminal = true;
                                upstream_failed = true;
                            }
                            _ => {}
                        }
                        let mut surfaces = vec![(u32::MAX, messages_projection(&value))];
                        if kind == "content_block_delta" {
                            let Some(index) = value
                                .get("index")
                                .and_then(Value::as_u64)
                                .and_then(|v| u32::try_from(v).ok())
                                .filter(|v| *v != u32::MAX)
                            else {
                                failure = Some(invalid("Invalid content block index"));
                                break 'upstream;
                            };
                            for field in ["text", "thinking", "partial_json"] {
                                if let Some(text) = value
                                    .get("delta")
                                    .and_then(|v| v.get(field))
                                    .and_then(Value::as_str)
                                {
                                    surfaces.push((index, text.to_string()));
                                }
                            }
                        }
                        match guard.push_many_until_closed(&tx, surfaces, frame).await {
                            Ok(Some(events)) => {
                                for event in events {
                                    if tx.send(event).await.is_err() {
                                        break 'upstream;
                                    }
                                }
                            }
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
                        if matches!(kind, "ping" | "message_start") {
                            match guard.flush_to_until_closed(&tx).await {
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
                Ok(None) => {
                    if !terminal {
                        failure = Some(invalid("Messages stream ended before message_stop"));
                    }
                    break;
                }
                Err(_) => {
                    failure = Some(ProviderError::network(
                        "anthropic",
                        "Messages stream interrupted",
                    ));
                    break;
                }
            }
        }
        if terminal
            && failure.is_none()
            && let Err(error) = guard.flush_to_until_closed(&tx).await
        {
            failure = Some(ProviderError::api_error("guardrail", 403, error.message()));
        }
        let usage = (terminal && final_usage && !upstream_failed)
            .then(|| native_usage(&usage))
            .flatten();
        settle(
            &state,
            &context,
            &provider,
            &model,
            pricing,
            usage.as_ref(),
            reservation,
            key_reservation,
            facts,
        )
        .await;
        if let Some(error) = failure {
            callback.fail(error.to_string(), "stream_error");
            lease.finish_failure(&error);
            let kind = if matches!(
                error,
                ProviderError::ApiError {
                    provider: "guardrail",
                    ..
                }
            ) {
                "permission_error"
            } else {
                "api_error"
            };
            let event = json!({"type":"error", "error":{"type":kind,"message":error.redacted().to_string()}});
            let _ = tx
                .send(Bytes::from(format!("event: error\ndata: {event}\n\n")))
                .await;
        } else if upstream_failed {
            callback.fail("Upstream Messages stream failed", "provider_error");
            lease.finish_failure(&ProviderError::api_error(
                "anthropic",
                502,
                "Upstream Messages stream failed",
            ));
        } else if terminal {
            callback.complete_pricing_usage(usage.as_ref().map(|u| &u.pricing), "success");
            lease.finish_success(
                usage
                    .as_ref()
                    .map_or(0, |u| u64::from(u.normalized.total_tokens)),
            );
        } else {
            callback.fail("Client disconnected", "client_disconnect");
        }
    });
    HttpResponse::Ok()
        .insert_header(("content-type", "text/event-stream"))
        .insert_header(("cache-control", "no-cache"))
        .streaming(ReceiverStream::new(rx).map(Ok::<_, actix_web::Error>))
}

fn invalid(message: &str) -> ProviderError {
    ProviderError::response_parsing("anthropic", message)
}
fn next_frame(pending: &mut Vec<u8>) -> Option<Bytes> {
    let lf = pending.windows(2).position(|w| w == b"\n\n").map(|p| p + 2);
    let crlf = pending
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4);
    let end = lf.into_iter().chain(crlf).min()?;
    Some(Bytes::from(pending.drain(..end).collect::<Vec<_>>()))
}
fn event(frame: &[u8]) -> Result<Option<Value>, ProviderError> {
    let text = std::str::from_utf8(frame).map_err(|_| invalid("Invalid Messages SSE UTF-8"))?;
    let data = text
        .lines()
        .filter_map(|line| {
            line.strip_prefix("data:")
                .map(|v| v.strip_prefix(' ').unwrap_or(v))
        })
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&data)
        .map(Some)
        .map_err(|_| invalid("Invalid Messages SSE JSON"))
}
fn merge_usage(target: &mut Value, delta: &Value) {
    if let (Some(target), Some(delta)) = (target.as_object_mut(), delta.as_object()) {
        for (key, value) in delta {
            if value.is_object() && target.get(key).is_some_and(Value::is_object) {
                merge_usage(&mut target[key], value);
            } else {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}
