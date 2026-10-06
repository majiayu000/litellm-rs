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
    if output_limit == Some(0) {
        return Ok(denied("Realtime output is forbidden by key policy"));
    }
    let runtime = state.pin_runtime();
    let jwt_token = match crate::server::middleware::extract_auth_method_with_api_key_header(
        req.headers(),
        &runtime.config.gateway.auth.api_key_header,
    ) {
        crate::auth::AuthMethod::Jwt(token) => Some(token),
        _ => None,
    };
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
    let budget_router = runtime.unified_router.clone();
    let selected = super::execution::execute_stream_with_selected_deployment_matching(
        runtime.unified_router.clone(),
        &query.model,
        ProviderCapability::RealtimeApi,
        |deployment| matches!(&deployment.provider, Provider::OpenAI(_)),
        move |provider, model, deployment_id| {
            let pricing = pricing.clone();
            let budget_provider = budget_router
                .configured_provider_name(&deployment_id)
                .unwrap_or_else(|| provider.name().to_string());
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
                let wire_output_limit = rates
                    .wire_output_limit(rates.max_output)
                    .map_err(|message| ProviderError::invalid_request("openai", message))?;
                let Provider::OpenAI(openai) = &provider else {
                    return Err(ProviderError::invalid_request(
                        "openai",
                        "Realtime requires an OpenAI deployment",
                    ));
                };
                let timeout = openai.config.base.timeout_duration();
                let mut upstream = open_upstream(openai, &model, max_size).await?;
                let initial = initialize_upstream(
                    &mut upstream,
                    wire_output_limit,
                    &openai.config.get_model_mapping(&model),
                    timeout,
                )
                .await?;
                Ok((
                    upstream,
                    rates,
                    budget_provider,
                    openai.config.get_model_mapping(&model),
                    model,
                    timeout,
                    initial,
                ))
            }
        },
    )
    .await;
    let ((upstream, rates, provider, wire_model, model, timeout, initial), lease) = match selected {
        Ok(selected) => selected,
        Err(error) => return Ok(super::openai_errors::gateway_error_response(&error)),
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
            query.model.clone(),
            wire_model,
            jwt_token,
        )
        .await;
    });
    Ok(response)
}

fn upstream_event_error(error: &Value) -> ProviderError {
    let status = match error["type"].as_str() {
        Some("rate_limit_error") => 429,
        Some("authentication_error") => 401,
        Some("permission_error") => 403,
        Some("invalid_request_error") => 400,
        _ => 500,
    };
    ProviderError::api_error("openai", status, error.to_string())
}

fn manual_session(value: &Value) -> bool {
    value["session"]["type"] == "realtime"
        && value["session"]["tools"]
            .as_array()
            .is_some_and(|tools| tools.iter().all(|tool| tool["type"] == "function"))
        && value.pointer("/session/audio/input/turn_detection") == Some(&Value::Null)
        && value.pointer("/session/audio/input/transcription") == Some(&Value::Null)
}

async fn initialize_upstream(
    upstream: &mut Upstream,
    output_limit: Value,
    wire_model: &str,
    timeout: Duration,
) -> Result<Vec<String>, ProviderError> {
    let initialize = async {
        upstream.send(Message::Text(json!({"type":"session.update","session":{"type":"realtime","tools":[],"max_output_tokens":output_limit,"audio":{"input":{"turn_detection":null,"transcription":null}}}}).to_string().into())).await.map_err(|_| ProviderError::network("openai", "Realtime initialization write failed"))?;
        let mut initial = Vec::new();
        while let Some(event) = upstream.next().await {
            match event.map_err(|_| {
                ProviderError::network("openai", "Realtime initialization interrupted")
            })? {
                Message::Text(text) => {
                    let value: Value = serde_json::from_str(&text).map_err(|_| {
                        ProviderError::network("openai", "Malformed Realtime initialization")
                    })?;
                    if value["type"] == "error" {
                        return Err(upstream_event_error(&value["error"]));
                    }
                    if matches!(
                        value["type"].as_str(),
                        Some("session.created" | "session.updated")
                    ) && value["session"]["model"]
                        .as_str()
                        .is_some_and(|model| model != wire_model)
                    {
                        return Err(ProviderError::network(
                            "openai",
                            "Upstream Realtime session model does not match the selected deployment",
                        ));
                    }
                    let ready = value["type"] == "session.updated";
                    if ready
                        && (!manual_session(&value)
                            || value["session"]["tools"] != json!([])
                            || value["session"]["max_output_tokens"] != output_limit)
                    {
                        return Err(ProviderError::network(
                            "openai",
                            "Upstream did not enforce manual Realtime configuration",
                        ));
                    }
                    initial.push(text.to_string());
                    if ready {
                        return Ok(initial);
                    }
                    if initial.len() >= 8 {
                        return Err(ProviderError::network(
                            "openai",
                            "Unexpected Realtime initialization events",
                        ));
                    }
                }
                Message::Ping(bytes) => {
                    upstream.send(Message::Pong(bytes)).await.map_err(|_| {
                        ProviderError::network("openai", "Realtime initialization pong failed")
                    })?;
                }
                _ => {
                    return Err(ProviderError::network(
                        "openai",
                        "Unexpected Realtime initialization frame",
                    ));
                }
            }
        }
        Err(ProviderError::network(
            "openai",
            "Realtime initialization closed",
        ))
    };
    match tokio::time::timeout(timeout, initialize).await {
        Ok(Ok(initial)) => Ok(initial),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(ProviderError::network(
            "openai",
            "Realtime initialization timeout",
        )),
    }
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

fn prepare_event(
    value: &mut Value,
    rates: &Rates,
    active: bool,
    public_model: &str,
    wire_model: &str,
) -> Result<bool, &'static str> {
    let kind = value["type"]
        .as_str()
        .ok_or("Realtime event type is required")?;
    let creates = kind == "response.create";
    let creates_item = kind == "conversation.item.create";
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
            if let Some(model) = options.get("model") {
                if creates
                    || !model
                        .as_str()
                        .is_some_and(|model| model == public_model || model == wire_model)
                {
                    return Err("Realtime model cannot change within a session");
                }
                options.remove("model");
            }
            if options
                .get("max_output_tokens")
                .is_some_and(|limit| limit == "inf")
            {
                options.insert(
                    "max_output_tokens".into(),
                    rates.wire_output_limit(rates.model_max_output)?,
                );
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
                    if v == "inf" {
                        return Ok(rates.model_max_output as u64);
                    }
                    v.as_u64()
                        .filter(|limit| *limit <= 4096)
                        .ok_or("max_output_tokens must be an integer from 1 to 4096 or inf")
                })
                .transpose()?
                .unwrap_or(rates.model_max_output as u64);
            if limit == 0 || limit > rates.model_max_output as u64 {
                return Err("Realtime output limit exceeds model or key policy");
            }
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
    let image_content = |item: &Value| {
        item["content"]
            .as_array()
            .is_some_and(|parts| parts.iter().any(|part| part["type"] == "input_image"))
    };
    let has_image = if creates_item {
        image_content(&value["item"])
    } else if creates {
        value["response"]["input"]
            .as_array()
            .is_some_and(|items| items.iter().any(image_content))
    } else {
        false
    };
    if has_image {
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
    mut context: crate::core::types::context::RequestContext,
    rates: Rates,
    provider: String,
    model: String,
    mut lease: super::execution::StreamingDeploymentLease,
    timeout: Duration,
    public_model: String,
    wire_model: String,
    jwt_token: Option<String>,
) {
    let mut pending: Option<Pending> = None;
    let mut response_id: Option<String> = None;
    let mut response_event_id: Option<String> = None;
    let mut tokens = 0u64;
    let mut failure = false;
    let mut error_type = "server_error";
    let mut session_output_limit = rates.max_output;
    let mut pending_session_update: Option<(String, Value)> = None;
    lease.cancel_response().await;
    let outcome: Result<(), String> = async {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(timeout) => { failure = pending.is_some(); return Err("Realtime connection idle timeout".into()); },
                event = input.next() => match event {
                    Some(Ok(actix_ws::AggregatedMessage::Text(text))) => {
                        let mut value: Value = serde_json::from_str(&text).map_err(|_| { error_type = "invalid_request_error"; "Invalid Realtime JSON" })?;
                        if pending_session_update.is_some() && matches!(value["type"].as_str(), Some("response.create" | "session.update")) {
                            send_client(&mut downstream, json!({"type":"error","error":{"type":"invalid_request_error","message":"Wait for session.updated before creating a response or updating the session","event_id":value.get("event_id")}}).to_string(), timeout).await?;
                            continue;
                        }
                        let unbounded_output = value["response"]["max_output_tokens"] == "inf";
                        let creates = match prepare_event(&mut value, &rates, pending.is_some(), &public_model, &wire_model) {
                            Ok(creates) => creates,
                            Err(message) => { send_client(&mut downstream, json!({"type":"error","error":{"type":"invalid_request_error","message":message,"event_id":value.get("event_id")}}).to_string(), timeout).await?; continue; }
                        };
                        if creates {
                            let current_runtime = state.pin_runtime();
                            if current_runtime.guardrails.is_enabled() {
                                error_type = "permission_error";
                                return Err("Realtime does not implement configured content guardrails".into());
                            }
                            let rate_policy = &current_runtime.config.gateway.rate_limit;
                            let requests_per_minute = rate_policy.enabled.then(|| rate_policy.effective_rpm());
                            let mut key_rpm = requests_per_minute;
                            let mut output_limit = rates.model_max_output;
                            if let Some(token) = &jwt_token {
                                let authenticated = state.auth.authenticate(crate::auth::AuthMethod::Jwt(token.clone()), context.clone()).await.map_err(|_| "Realtime user reauthorization unavailable")?;
                                if !authenticated.success || !context::check_permission(authenticated.user.as_ref(), None, "realtime") {
                                    error_type = "authentication_error";
                                    return Err("Realtime user is no longer authorized".into());
                                }
                                context = authenticated.context;
                            }
                            if let Some(key_id) = context.api_key_id() {
                                let key = state.storage.db().find_api_key_by_id(key_id).await.map_err(|_| "Realtime key reauthorization unavailable")?.ok_or_else(|| { error_type = "authentication_error"; "Realtime API key no longer exists" })?;
                                if !key.is_active || key.expires_at.is_some_and(|expires| expires <= chrono::Utc::now())
                                    || !context::check_permission(None, Some(&key), "realtime")
                                    || !context::api_key_allows_endpoint(Some(&key), "/v1/realtime").map_err(|_| "Invalid Realtime key policy")? {
                                    error_type = "authentication_error";
                                    return Err("Realtime API key is no longer authorized".into());
                                }
                                if let Some(user_id) = key.user_id {
                                    let owner = state.storage.db().find_user_by_id(user_id).await.map_err(|_| "Realtime key owner verification unavailable")?;
                                    if !owner.is_some_and(|owner| owner.is_active()) { error_type = "authentication_error"; return Err("Realtime API key owner is no longer authorized".into()); }
                                }
                                context::enforce_key_model_and_token_limits(&key, &public_model, if unbounded_output { None } else { value["response"]["max_output_tokens"].as_u64().and_then(|v| u32::try_from(v).ok()) }).map_err(|_| { error_type = "authentication_error"; "Realtime model or output policy denied" })?;
                                output_limit = context::api_key_output_limit(&key).map_err(|_| "Invalid Realtime key policy")?.map_or(output_limit, |limit| limit.min(output_limit));
                                key_rpm = key.rate_limits.as_ref().and_then(|limits| limits.rpm).or(requests_per_minute);
                                if let Some(budget_id) = crate::auth::api_key_budget_id(&key) { context.set_api_key_budget_id(budget_id); } else { context.clear_api_key_budget_id(); }
                            }
                            if output_limit == 0 { error_type = "authentication_error"; return Err("Realtime output is forbidden by current key policy".into()); }
                            let effective_output = if unbounded_output {
                                output_limit
                            } else {
                                value["response"]["max_output_tokens"].as_u64().map_or(session_output_limit.min(output_limit), |limit| limit as u32)
                            };
                            let wire_limit = match rates.wire_output_limit(effective_output) {
                                Ok(limit) => limit,
                                Err(message) => { send_client(&mut downstream, json!({"type":"error","error":{"type":"invalid_request_error","message":message,"event_id":value.get("event_id")}}).to_string(), timeout).await?; continue; }
                            };
                            value["response"]["max_output_tokens"] = wire_limit;
                            if let Some(rpm) = key_rpm
                                && let Err(retry_after) = crate::server::middleware::enforce_socket_request_rate(&context, rpm).await {
                                send_client(&mut downstream, json!({"type":"error","error":{"type":"rate_limit_error","message":"Realtime request rate exceeded","retry_after":retry_after}}).to_string(), timeout).await?;
                                continue;
                            }
                            lease.refresh_realtime_deployment(current_runtime.unified_router.clone()).await.map_err(|error| error.redacted().to_string())?;
                            if let Err(error) = lease.begin_response(1).await {
                                let kind = if matches!(error, ProviderError::RateLimit { .. }) { "rate_limit_error" } else { "server_error" };
                                send_client(&mut downstream, json!({"type":"error","error":{"type":kind,"message":error.redacted().to_string()}}).to_string(), timeout).await?;
                                continue;
                            }
                            match Pending::reserve(&state, &provider, &model, context.api_key_budget_id(), rates.bound(effective_output), effective_output).await {
                                Ok(reservation) => {
                                    let event_id = value["event_id"].as_str().map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                    value["event_id"] = json!(event_id);
                                    response_event_id = Some(event_id);
                                    pending = Some(reservation);
                                    // No trusted terminal usage may arrive; admission retains the reserved upper bound.
                                    tokens = rates.max_input as u64 + effective_output as u64;
                                },
                                Err(error) => { lease.cancel_response().await; let kind = if matches!(error, ProviderError::QuotaExceeded { .. }) { "insufficient_quota" } else { "server_error" }; send_client(&mut downstream, json!({"type":"error","error":{"type":kind,"message":error.to_string()}}).to_string(), timeout).await?; continue; }
                            }
                        }
                        if value["type"] == "session.update" {
                            let event_id = value["event_id"].as_str().map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                            value["event_id"] = json!(event_id);
                            let expected_output = value["session"].get("max_output_tokens").cloned()
                                .unwrap_or(rates.wire_output_limit(session_output_limit).map_err(str::to_owned)?);
                            pending_session_update = Some((event_id, expected_output));
                        }
                        tokio::time::timeout(timeout, upstream.send(Message::Text(value.to_string().into()))).await.map_err(|_| { failure = true; "Realtime upstream write timeout" })?.map_err(|_| { failure = true; "Realtime upstream write failed" })?;
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
                    _ => { error_type = "invalid_request_error"; return Err("Unsupported or invalid Realtime client frame".into()); },
                },
                event = upstream.next() => match event {
                    Some(Ok(Message::Text(text))) => {
                        let value: Value = serde_json::from_str(&text).map_err(|_| { failure = true; "Malformed upstream Realtime event" })?;
                        if value["type"] == "error" && value["error"]["type"] != "invalid_request_error"
                            && !(pending.is_some() && response_id.is_none()
                                && response_event_id.as_deref() == value["error"]["event_id"].as_str()) {
                            lease.record_provider_event_failure(&upstream_event_error(&value["error"])).await;
                        }
                        if value["type"] == "error" && pending_session_update.as_ref().is_some_and(|(id, _)| Some(id.as_str()) == value["error"]["event_id"].as_str()) {
                            pending_session_update = None;
                        }
                        if value["type"] == "session.updated" {
                            let expected_output = pending_session_update.as_ref().map(|(_, limit)| limit.clone())
                                .unwrap_or(rates.wire_output_limit(session_output_limit).map_err(str::to_owned)?);
                            if !manual_session(&value) || value["session"]["max_output_tokens"] != expected_output
                                || value["session"]["model"].as_str().is_some_and(|model| model != wire_model) {
                                failure = true;
                                return Err("Upstream did not enforce the requested Realtime session configuration".into());
                            }
                            pending_session_update = None;
                            if let Some(limit) = value["session"]["max_output_tokens"].as_u64().and_then(|limit| u32::try_from(limit).ok()) {
                                session_output_limit = limit.min(rates.model_max_output);
                            } else if value["session"]["max_output_tokens"] == "inf" {
                                session_output_limit = rates.model_max_output;
                            }
                        } else if value["type"] == "response.created" {
                            if pending.is_none() || response_id.is_some() { failure = true; return Err("Unreserved upstream Realtime response".into()); }
                            response_id = Some(value["response"]["id"].as_str().ok_or_else(|| { failure = true; "Missing Realtime response ID" })?.to_owned());
                        } else if value["type"] == "response.done" {
                            if response_id.as_deref() != value["response"]["id"].as_str() || response_id.is_none() { failure = true; return Err("Mismatched Realtime response ID".into()); }
                            let status = value["response"]["status"].as_str().filter(|status| matches!(*status, "completed" | "failed" | "cancelled" | "incomplete"))
                                .ok_or_else(|| { failure = true; "Unknown Realtime terminal response status" })?;
                            let reservation = pending.take().ok_or("Missing Realtime reservation")?;
                            let usage = rates.cost(&value["response"]["usage"], reservation.max_output);
                            match usage {
                                Ok(usage) => {
                                    tokens = usage.map_or(tokens, |(_, tokens)| tokens);
                                    let provider_failed = status == "failed"
                                        && value["response"]["status_details"]["error"]["type"] != "invalid_request_error";
                                    let terminal_error = provider_failed.then(|| upstream_event_error(&value["response"]["status_details"]["error"]));
                                    let settlement = reservation.settle(&state, context.api_key_id(), usage);
                                    let settled = if status == "completed" {
                                        lease.settle_terminal(tokens, None, settlement).await
                                    } else {
                                        lease.settle_interrupted(tokens, terminal_error.as_ref(), settlement).await
                                    };
                                    if let Err(error) = settled {
                                        tracing::error!(%error, %provider, %model, key_id = ?context.api_key_id(), tokens, cost = ?usage.map(|(cost, _)| cost), "Realtime terminal response budget settlement failed");
                                    }
                                    if provider_failed {
                                        lease.finish_interrupted(tokens, Some(&upstream_event_error(&value["response"]["status_details"]["error"]))).await;
                                    } else if matches!(status, "cancelled" | "incomplete" | "failed") {
                                        lease.finish_interrupted(tokens, None).await;
                                    } else {
                                        lease.complete_response(tokens, None).await;
                                    }
                                }
                                Err(error) => { failure = true; return Err(error); }
                            }
                            tokens = 0;
                            response_id = None;
                            response_event_id = None;
                        } else if value["type"] == "error" && pending.is_some() && response_id.is_none()
                            && response_event_id.as_deref() == value["error"]["event_id"].as_str() {
                            // No trusted usage accompanies this error; retain the documented reservation fallback.
                            if let Some(reservation) = pending.take()
                                && let Err(error) = reservation.settle(&state, context.api_key_id(), None).await
                            {
                                tracing::error!(%error, %provider, %model, "Realtime pre-creation error settlement failed");
                            }
                            if value["error"]["type"] == "invalid_request_error" {
                                lease.finish_interrupted(0, None).await;
                            } else {

                                lease.finish_interrupted(0, Some(&upstream_event_error(&value["error"]))).await;
                            }
                            response_event_id = None;
                            tokens = 0;
                        }
                        tokio::time::timeout(timeout, downstream.text(text.to_string())).await.map_err(|_| "Realtime downstream write timeout")?.map_err(|_| "Client disconnected")?;
                    }
                    Some(Ok(Message::Ping(bytes))) => { tokio::time::timeout(timeout, upstream.send(Message::Pong(bytes))).await.map_err(|_| { failure = true; "Realtime pong timeout" })?.map_err(|_| { failure = true; "Realtime pong failed" })?; }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(reason))) => {
                        failure = pending.is_some() || reason.as_ref().is_some_and(|reason| !matches!(u16::from(reason.code), 1000 | 1001));
                        let reason = reason.map(|reason| actix_ws::CloseReason { code: u16::from(reason.code).into(), description: Some(reason.reason.to_string()) });
                        let _ = tokio::time::timeout(timeout, downstream.clone().close(reason)).await;
                        return Ok(());
                    }
                    None => { failure = true; return Err("Realtime upstream closed without a close frame".into()); },
                    _ => { failure = true; return Err("Realtime upstream transport failed".into()); },
                }
            }
        }
    }.await;
    if let Some(reservation) = pending
        && let Err(error) = reservation.settle(&state, context.api_key_id(), None).await
    {
        tracing::error!(%error, "Realtime interrupted usage settlement failed");
    }
    if let Err(error) = outcome {
        let _ = tokio::time::timeout(
            timeout,
            downstream.text(
                json!({"type":"error","error":{"type":error_type,"message":error}}).to_string(),
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
    let error = ProviderError::network("openai", "Realtime transport failed");
    if tokens > 0 {
        lease
            .finish_interrupted(tokens, failure.then_some(&error))
            .await;
    } else if failure {
        // An idle socket has already finalized its generation lease. A new
        // transport failure is a separate provider event, not a second attempt
        // to complete that generation.
        lease.record_provider_event_failure(&error).await;
        lease.finish_neutral(0).await;
    } else {
        lease.finish_neutral(0).await;
    }
}

#[cfg(test)]
#[path = "realtime_tests.rs"]
mod tests;
