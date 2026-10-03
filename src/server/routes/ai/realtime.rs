//! OpenAI GA Realtime, with explicit manual-response budget boundaries.
use super::{
    context,
    realtime_billing::{Pending, Rates},
};
use crate::{
    core::{
        providers::{Provider, ProviderError, base::BaseHttpClient},
        types::model::ProviderCapability,
    },
    server::state::AppState,
    utils::error::gateway_error::GatewayError,
};
use actix_web::{HttpRequest, HttpResponse, web};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        Message,
        handshake::derive_accept_key,
        protocol::{Role, WebSocketConfig},
    },
};

type Upstream = WebSocketStream<reqwest::Upgraded>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Query {
    model: String,
}

pub(super) async fn connect(
    req: HttpRequest,
    payload: web::Payload,
    query: web::Query<Query>,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let user = context::get_authenticated_user(&req);
    let key = context::get_authenticated_api_key(&req);
    let denied = |message: &str| {
        super::openai_errors::gateway_error_response(&GatewayError::forbidden(message))
    };
    if user.is_none() && key.is_none() {
        return Ok(HttpResponse::Unauthorized().finish());
    }
    if !context::check_permission(user.as_ref(), key.as_ref(), "realtime")
        || !context::api_key_allows_endpoint(key.as_ref(), req.path()).unwrap_or(false)
    {
        return Ok(denied("Realtime endpoint permission denied"));
    }
    if let Err(error) = context::enforce_api_key_model_and_token_limits(&req, &query.model, None) {
        return Ok(super::openai_errors::gateway_error_response(&error));
    }
    let output_limit = context::api_key_max_tokens_per_request(&req)?;
    let runtime = state.pin_runtime();
    let requests_per_minute = key
        .as_ref()
        .and_then(|key| key.rate_limits.as_ref())
        .and_then(|limits| limits.rpm)
        .or_else(|| {
            runtime
                .config
                .gateway
                .rate_limit
                .enabled
                .then(|| runtime.config.gateway.rate_limit.effective_rpm())
        });
    if runtime.guardrails.is_enabled() {
        return Ok(denied(
            "Realtime does not implement configured content guardrails",
        ));
    }
    if let Some(origin) = req.headers().get("origin") {
        let allowed = origin.to_str().ok().is_some_and(|origin| {
            origin != "null"
                && runtime
                    .config
                    .gateway
                    .server
                    .cors
                    .allowed_origins
                    .iter()
                    .any(|allowed| allowed != "*" && allowed == origin)
        });
        if !allowed {
            return Ok(denied("Realtime Origin is not allowed"));
        }
    }
    // Browser credential subprotocols and beta wire formats are not this route's scope.
    if req.headers().contains_key("sec-websocket-protocol")
        || req.headers().contains_key("openai-beta")
    {
        return Ok(HttpResponse::BadRequest().json(json!({"error":{"message":"Use gateway HTTP authentication and the GA Realtime protocol"}})));
    }
    let (response, mut session, stream) = actix_ws::handle(&req, payload)?;
    let context = context::get_request_context(&req)?;
    let pricing = state.pricing.clone();
    let max_size = runtime.config.gateway.server.max_body_size;
    let selected = super::execution::execute_stream_with_selected_deployment_matching(
        runtime.unified_router.clone(),
        &query.model,
        ProviderCapability::RealtimeApi,
        |deployment| matches!(&deployment.provider, Provider::OpenAI(_)),
        move |provider, model, _| {
            let pricing = pricing.clone();
            async move {
                let request_pricing = super::spend::request_pricing_for_provider(
                    &pricing,
                    &provider,
                    &model,
                    ProviderCapability::RealtimeApi,
                )?;
                let info = request_pricing.model_info().ok_or_else(|| {
                    ProviderError::configuration("openai", "Realtime pricing unavailable")
                })?;
                let rates = Rates::load(&info, output_limit)
                    .map_err(|e| ProviderError::configuration("openai", e))?;
                let Provider::OpenAI(openai) = &provider else {
                    return Err(ProviderError::invalid_request(
                        "openai",
                        "Realtime requires an OpenAI deployment",
                    ));
                };
                let timeout = openai.config.base.timeout_duration();
                let upstream = open_upstream(openai, &model, max_size).await?;
                Ok((upstream, rates, provider.name().to_owned(), model, timeout))
            }
        },
    )
    .await;
    let ((mut upstream, rates, provider, model, timeout), lease) = match selected {
        Ok(selected) => selected,
        Err(error) => return Ok(super::openai_errors::gateway_error_response(&error)),
    };
    // Establish the manual-generation contract before any client event reaches upstream.
    let initialize = async {
        upstream.send(Message::Text(json!({"type":"session.update","session":{"type":"realtime","tools":[],"max_output_tokens":rates.max_output,"audio":{"input":{"turn_detection":null,"transcription":null}}}}).to_string().into())).await.map_err(|_| "Realtime initialization write failed")?;
        let mut initial = Vec::new();
        while let Some(event) = upstream.next().await {
            match event.map_err(|_| "Realtime initialization interrupted")? {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text)
                        .map_err(|_| "Malformed Realtime initialization")?;
                    if value["type"] == "error" {
                        return Err("Upstream rejected manual Realtime configuration");
                    }
                    let ready = value["type"] == "session.updated";
                    if ready
                        && (value.pointer("/session/audio/input/turn_detection")
                            != Some(&Value::Null)
                            || value.pointer("/session/audio/input/transcription")
                                != Some(&Value::Null)
                            || value["session"]["max_output_tokens"].as_u64()
                                != Some(rates.max_output as u64))
                    {
                        return Err("Upstream did not enforce manual Realtime configuration");
                    }
                    initial.push(text.to_string());
                    if ready {
                        return Ok(initial);
                    }
                    if initial.len() >= 8 {
                        return Err("Unexpected Realtime initialization events");
                    }
                }
                Message::Ping(bytes) => {
                    upstream
                        .send(Message::Pong(bytes))
                        .await
                        .map_err(|_| "Realtime initialization pong failed")?;
                }
                _ => return Err("Unexpected Realtime initialization frame"),
            }
        }
        Err("Realtime initialization closed")
    };
    let initial = match tokio::time::timeout(timeout, initialize).await {
        Ok(Ok(initial)) => initial,
        _ => {
            lease.finish_failure(&ProviderError::network(
                "openai",
                "Realtime initialization failed",
            ));
            return Ok(HttpResponse::BadGateway().json(
                json!({"error":{"message":"Realtime manual-session initialization failed"}}),
            ));
        }
    };
    let state = state.get_ref().clone();
    let stream = stream
        .max_frame_size(max_size)
        .aggregate_continuations()
        .max_continuation_size(max_size);
    actix_web::rt::spawn(async move {
        for event in initial {
            if session.text(event).await.is_err() {
                return;
            }
        }
        relay(
            session,
            stream,
            upstream,
            state,
            context,
            rates,
            provider,
            model,
            lease,
            timeout,
            requests_per_minute,
        )
        .await;
    });
    Ok(response)
}

async fn open_upstream(
    provider: &crate::core::providers::openai::OpenAIProvider,
    model: &str,
    max_size: usize,
) -> Result<Upstream, ProviderError> {
    let config = &provider.config.base;
    let base = config
        .api_base
        .as_deref()
        .unwrap_or("https://api.openai.com/v1");
    let mut url = reqwest::Url::parse(&format!("{}/realtime", base.trim_end_matches('/')))
        .map_err(|_| ProviderError::configuration("openai", "Invalid Realtime endpoint"))?;
    if url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ProviderError::configuration(
            "openai",
            "Realtime endpoint must not contain credentials, query or fragment",
        ));
    }
    #[cfg(not(test))]
    if url.scheme() != "https" {
        return Err(ProviderError::configuration(
            "openai",
            "Realtime upstream credentials require HTTPS",
        ));
    }
    url.query_pairs_mut()
        .append_pair("model", &provider.config.get_model_mapping(model));
    let client = BaseHttpClient::new_for_provider_streaming_no_redirect("openai", config.clone())?;
    let ws_key = tokio_tungstenite::tungstenite::handshake::client::generate_key();
    let mut headers = reqwest::header::HeaderMap::new();
    for (name, value) in provider.get_request_headers() {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            ProviderError::configuration("openai", "Invalid Realtime provider header")
        })?;
        if matches!(
            name.as_str(),
            "host"
                | "connection"
                | "upgrade"
                | "sec-websocket-key"
                | "sec-websocket-version"
                | "sec-websocket-protocol"
                | "sec-websocket-extensions"
        ) {
            continue;
        }
        headers.insert(
            name,
            reqwest::header::HeaderValue::from_str(&value).map_err(|_| {
                ProviderError::configuration("openai", "Invalid Realtime provider header")
            })?,
        );
    }
    let request = client
        .get(url)
        .map_err(|_| ProviderError::configuration("openai", "Realtime endpoint rejected"))?
        .headers(headers)
        .header("connection", "Upgrade")
        .header("upgrade", "websocket")
        .header("sec-websocket-version", "13")
        .header("sec-websocket-key", &ws_key)
        .version(reqwest::Version::HTTP_11);
    let mut response = tokio::time::timeout(config.timeout_duration(), request.send())
        .await
        .map_err(|_| ProviderError::timeout("openai", "Realtime handshake timed out"))?
        .map_err(|_| ProviderError::network("openai", "Realtime handshake failed"))?;
    if response.status() != reqwest::StatusCode::SWITCHING_PROTOCOLS {
        let status = response.status().as_u16();
        let body = tokio::time::timeout(config.timeout_duration(), async {
            let mut body = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
                if chunk.len() > max_size.saturating_sub(body.len()) {
                    return Err(());
                }
                body.extend_from_slice(&chunk);
            }
            String::from_utf8(body).map_err(|_| ())
        })
        .await;
        let message = match body {
            Ok(Ok(body)) => body,
            _ => "Realtime upstream handshake rejected; error body unavailable".into(),
        };
        return Err(ProviderError::api_error("openai", status, message));
    }
    if response
        .headers()
        .get("sec-websocket-accept")
        .and_then(|v| v.to_str().ok())
        != Some(derive_accept_key(ws_key.as_bytes()).as_str())
        || !response
            .headers()
            .get("upgrade")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
        || !response
            .headers()
            .get("connection")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
            })
        || response.headers().contains_key("sec-websocket-protocol")
        || response.headers().contains_key("sec-websocket-extensions")
    {
        return Err(ProviderError::network(
            "openai",
            "Invalid Realtime upgrade response",
        ));
    }
    let upgraded = response
        .upgrade()
        .await
        .map_err(|_| ProviderError::network("openai", "Realtime upgrade failed"))?;
    let ws_config = WebSocketConfig::default()
        .max_message_size(Some(max_size))
        .max_frame_size(Some(max_size));
    Ok(WebSocketStream::from_raw_socket(upgraded, Role::Client, Some(ws_config)).await)
}

fn prepare_event(value: &mut Value, rates: &Rates, active: bool) -> Result<bool, &'static str> {
    let kind = value["type"]
        .as_str()
        .ok_or("Realtime event type is required")?;
    let creates = kind == "response.create";
    if creates && active {
        return Err("Only one Realtime response may be active");
    }
    match kind {
        "session.update" | "response.create" => {
            let field = if creates { "response" } else { "session" };
            if value.get(field).is_none() {
                value[field] = json!({});
            }
            let options = value[field]
                .as_object_mut()
                .ok_or("Realtime options must be an object")?;
            if options.contains_key("model") {
                return Err("Realtime model cannot change within a session");
            }
            if !creates && options.get("type").is_some_and(|v| v != "realtime") {
                return Err("Only realtime sessions are supported");
            }
            if options.get("tools").is_some_and(|tools| {
                tools
                    .as_array()
                    .is_none_or(|tools| tools.iter().any(|tool| tool["type"] != "function"))
            }) {
                return Err("Only local function tools are supported");
            }
            if !creates {
                let input = &options.get("audio").unwrap_or(&Value::Null)["input"];
                if !input["turn_detection"].is_null()
                    || !input["transcription"].is_null()
                    || options.contains_key("turn_detection")
                    || options.contains_key("input_audio_transcription")
                {
                    return Err(
                        "Automatic VAD and input transcription are unsupported; use manual response.create",
                    );
                }
            }
            let limit = options
                .get("max_output_tokens")
                .map(|v| {
                    v.as_u64()
                        .ok_or("max_output_tokens must be a positive integer")
                })
                .transpose()?
                .unwrap_or(rates.max_output as u64);
            if limit == 0 || limit > rates.max_output as u64 {
                return Err("Realtime output limit exceeds model or key policy");
            }
            options.insert("max_output_tokens".into(), json!(limit));
        }
        "conversation.item.create"
        | "conversation.item.delete"
        | "conversation.item.retrieve"
        | "conversation.item.truncate"
        | "input_audio_buffer.append"
        | "input_audio_buffer.commit"
        | "input_audio_buffer.clear"
        | "response.cancel" => {}
        _ => {
            return Err(
                "Realtime client event is outside the supported manual text/audio/function scope",
            );
        }
    }
    fn has_image(value: &Value) -> bool {
        match value {
            Value::Object(map) => {
                map.get("type").is_some_and(|v| v == "input_image") || map.values().any(has_image)
            }
            Value::Array(values) => values.iter().any(has_image),
            _ => false,
        }
    }
    if has_image(value) {
        return Err("Realtime image input is not yet metered by this gateway");
    }
    Ok(creates)
}

async fn send_client(
    session: &mut actix_ws::Session,
    text: String,
    timeout: Duration,
) -> Result<(), &'static str> {
    tokio::time::timeout(timeout, session.text(text))
        .await
        .map_err(|_| "Realtime downstream write timeout")?
        .map_err(|_| "Client disconnected")
}

#[allow(clippy::too_many_arguments)]
async fn relay(
    mut downstream: actix_ws::Session,
    mut input: actix_ws::AggregatedMessageStream,
    mut upstream: Upstream,
    state: AppState,
    context: crate::core::types::context::RequestContext,
    rates: Rates,
    provider: String,
    model: String,
    lease: super::execution::StreamingDeploymentLease,
    timeout: Duration,
    requests_per_minute: Option<u32>,
) {
    let mut pending: Option<Pending> = None;
    let mut response_id: Option<String> = None;
    let mut response_event_id: Option<String> = None;
    let mut tokens = 0u64;
    let mut failure = false;
    let outcome: Result<(), String> = async {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => return Err("Realtime connection idle timeout".into()),
                event = input.next() => match event {
                    Some(Ok(actix_ws::AggregatedMessage::Text(text))) => {
                        let mut value: Value = serde_json::from_str(&text).map_err(|_| "Invalid Realtime JSON")?;
                        let creates = match prepare_event(&mut value, &rates, pending.is_some()) {
                            Ok(creates) => creates,
                            Err(message) => { send_client(&mut downstream, json!({"type":"error","error":{"type":"invalid_request_error","message":message,"event_id":value.get("event_id")}}).to_string(), timeout).await?; continue; }
                        };
                        if creates {
                            if let Some(rpm) = requests_per_minute
                                && let Err(retry_after) = crate::server::middleware::enforce_socket_request_rate(&context, rpm).await {
                                send_client(&mut downstream, json!({"type":"error","error":{"type":"rate_limit_error","message":"Realtime request rate exceeded","retry_after":retry_after}}).to_string(), timeout).await?;
                                continue;
                            }
                            match Pending::reserve(&state, &provider, &model, context.api_key_budget_id(), rates.bound()) {
                                Ok(reservation) => {
                                    let event_id = value["event_id"].as_str().map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                    value["event_id"] = json!(event_id);
                                    response_event_id = Some(event_id);
                                    pending = Some(reservation);
                                },
                                Err(message) => { send_client(&mut downstream, json!({"type":"error","error":{"type":"insufficient_quota","message":message}}).to_string(), timeout).await?; continue; }
                            }
                        }
                        tokio::time::timeout(timeout, upstream.send(Message::Text(value.to_string().into()))).await.map_err(|_| "Realtime upstream write timeout")?.map_err(|_| "Realtime upstream write failed")?;
                    }
                    Some(Ok(actix_ws::AggregatedMessage::Ping(bytes))) => { tokio::time::timeout(timeout, downstream.pong(&bytes)).await.map_err(|_| "Realtime pong timeout")?.map_err(|_| "Client disconnected")?; }
                    Some(Ok(actix_ws::AggregatedMessage::Pong(_))) => {}
                    Some(Ok(actix_ws::AggregatedMessage::Close(reason))) => {
                        let close = reason.clone().map(|reason| tokio_tungstenite::tungstenite::protocol::CloseFrame { code: (u16::from(reason.code)).into(), reason: reason.description.unwrap_or_default().into() });
                        let _ = tokio::time::timeout(timeout, upstream.close(close)).await;
                        let _ = tokio::time::timeout(timeout, downstream.clone().close(reason)).await;
                        return Ok(());
                    }
                    None => return Ok(()),
                    _ => return Err("Unsupported or invalid Realtime client frame".into()),
                },
                event = upstream.next() => match event {
                    Some(Ok(Message::Text(text))) => {
                        let value: Value = serde_json::from_str(&text).map_err(|_| "Malformed upstream Realtime event")?;
                        if value["type"] == "response.created" {
                            if pending.is_none() || response_id.is_some() { return Err("Unreserved upstream Realtime response".into()); }
                            response_id = Some(value["response"]["id"].as_str().ok_or("Missing Realtime response ID")?.to_owned());
                        } else if value["type"] == "response.done" {
                            if response_id.as_deref() != value["response"]["id"].as_str() || response_id.is_none() { return Err("Mismatched Realtime response ID".into()); }
                            let usage = rates.cost(&value["response"]["usage"])?;
                            let reservation = pending.take().ok_or("Missing Realtime reservation")?;
                            tokens = tokens.saturating_add(usage.1);
                            reservation.settle(&state, context.api_key_id(), Some(usage)).await?;
                            response_id = None;
                            response_event_id = None;
                        } else if value["type"] == "error" && pending.is_some() && response_id.is_none()
                            && response_event_id.as_deref() == value["error"]["event_id"].as_str() {
                            // No trusted usage accompanies this error; retain the documented reservation fallback.
                            if let Some(reservation) = pending.take() { reservation.settle(&state, context.api_key_id(), None).await?; }
                            response_event_id = None;
                        }
                        tokio::time::timeout(timeout, downstream.text(text.to_string())).await.map_err(|_| "Realtime downstream write timeout")?.map_err(|_| "Client disconnected")?;
                    }
                    Some(Ok(Message::Ping(bytes))) => { tokio::time::timeout(timeout, upstream.send(Message::Pong(bytes))).await.map_err(|_| "Realtime pong timeout")?.map_err(|_| "Realtime pong failed")?; }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(reason))) => {
                        failure = reason.as_ref().is_some_and(|reason| !matches!(u16::from(reason.code), 1000 | 1001));
                        let reason = reason.map(|reason| actix_ws::CloseReason { code: u16::from(reason.code).into(), description: Some(reason.reason.to_string()) });
                        let _ = tokio::time::timeout(timeout, downstream.clone().close(reason)).await;
                        return Ok(());
                    }
                    None => return Err("Realtime upstream closed without a close frame".into()),
                    _ => return Err("Realtime upstream transport failed".into()),
                }
            }
        }
    }.await;
    if let Some(reservation) = pending
        && let Err(error) = reservation.settle(&state, context.api_key_id(), None).await
    {
        tracing::error!(%error, "Realtime interrupted usage settlement failed");
        failure = true;
    }
    if let Err(error) = outcome {
        failure = true;
        let _ = tokio::time::timeout(
            timeout,
            downstream.text(
                json!({"type":"error","error":{"type":"server_error","message":error}}).to_string(),
            ),
        )
        .await;
        let _ = tokio::time::timeout(
            timeout,
            downstream.close(Some(actix_ws::CloseReason {
                code: actix_ws::CloseCode::Error,
                description: Some("Realtime transport ended".into()),
            })),
        )
        .await;
    }
    if failure {
        lease.finish_failure_with_tokens(
            &ProviderError::network("openai", "Realtime transport failed"),
            tokens,
        );
    } else {
        lease.finish_success(tokens);
    }
}

#[cfg(test)]
#[path = "realtime_tests.rs"]
mod tests;
