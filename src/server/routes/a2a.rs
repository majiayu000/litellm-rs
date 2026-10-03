//! Authenticated A2A 1.0 JSON-RPC forwarding to configured upstream agents.
use super::ai::{
    api_key_allows_endpoint, check_permission, get_authenticated_api_key, get_authenticated_user,
};
use crate::{
    core::net::ProviderEndpointPolicy, server::state::AppState,
    utils::net::http::ProviderHttpClient,
};
use actix_web::{HttpRequest, HttpResponse, http::StatusCode, web};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Owner {
    principal: String,
    expires: Instant,
}
type TaskIdentity = (Vec<u8>, String, String);

/// Bounded process-local ownership. Restarted instances fail closed on old task IDs.
#[derive(Default)]
pub(crate) struct TaskOwners {
    entries: Mutex<HashMap<TaskIdentity, Owner>>,
    reserved: AtomicUsize,
    #[cfg(test)]
    client: Option<ProviderHttpClient>,
}
// Each new message reserves room for its task and context before any side effect.
// Unused slots are returned on errors, cancellation and stream disconnects.
struct TaskReservation {
    owners: Arc<TaskOwners>,
    remaining: usize,
}
impl Drop for TaskReservation {
    fn drop(&mut self) {
        self.owners
            .reserved
            .fetch_sub(self.remaining, Ordering::Relaxed);
    }
}
impl TaskOwners {
    fn reserve(self: &Arc<Self>, slots: usize) -> Result<TaskReservation, &'static str> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "A2A ownership unavailable")?;
        entries.retain(|_, owner| owner.expires > Instant::now());
        if entries.len() + self.reserved.load(Ordering::Relaxed) + slots > 4096 {
            return Err("A2A ownership capacity reached");
        }
        self.reserved.fetch_add(slots, Ordering::Relaxed);
        Ok(TaskReservation {
            owners: self.clone(),
            remaining: slots,
        })
    }

    fn owns(&self, binding: &[u8], principal: &str, kind: &str, id: &str) -> bool {
        self.entries
            .lock()
            .ok()
            .and_then(|entries| {
                entries
                    .get(&(binding.to_vec(), kind.into(), id.into()))
                    .cloned()
            })
            .is_some_and(|entry| entry.principal == principal && entry.expires > Instant::now())
    }
    fn observe(
        &self,
        binding: &[u8],
        principal: &str,
        value: &Value,
        expected_task: Option<&str>,
        expected_context: Option<&str>,
        reservation: Option<&mut TaskReservation>,
    ) -> Result<(), &'static str> {
        if let Some(error) = value.get("error") {
            if value.get("result").is_some()
                || error.get("code").and_then(Value::as_i64).is_none()
                || error.get("message").and_then(Value::as_str).is_none()
            {
                return Err("Invalid A2A error response");
            }
            return Ok(());
        }
        let result = value.get("result").ok_or("Missing A2A result")?;
        let (object, kind) = if let Some(task) = result.get("task") {
            (task, "task")
        } else if let Some(message) = result.get("message") {
            (message, "message")
        } else if let Some(update) = result.get("statusUpdate") {
            (update, "statusUpdate")
        } else if let Some(update) = result.get("artifactUpdate") {
            (update, "artifactUpdate")
        } else {
            (result, "task")
        };
        // The identity field belongs to the response variant, never to arbitrary
        // extra fields supplied by an upstream sharing several callers' tasks.
        if !object.is_object()
            || (kind == "task" && object.get("taskId").is_some())
            || (kind != "task" && object.get("id").is_some())
        {
            return Err("Ambiguous A2A response identity");
        }
        let task = identifier(object.get(if kind == "task" { "id" } else { "taskId" }))
            .map_err(|_| "Invalid A2A task identifier")?;
        let context =
            identifier(object.get("contextId")).map_err(|_| "Invalid A2A context identifier")?;
        if (matches!(kind, "statusUpdate" | "artifactUpdate") && context.is_none())
            || (kind != "message" && task.is_none())
        {
            return Err("Missing A2A task/context identifiers");
        }
        if matches!(kind, "task" | "statusUpdate")
            && object
                .pointer("/status/state")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Err("Missing A2A task status");
        }
        if kind == "message"
            && (identifier(object.get("messageId")).ok().flatten().is_none()
                || object.get("role").and_then(Value::as_str) != Some("ROLE_AGENT")
                || object
                    .get("parts")
                    .and_then(Value::as_array)
                    .is_none_or(|parts| parts.is_empty()))
        {
            return Err("Invalid A2A message");
        }
        if kind == "artifactUpdate"
            && (identifier(object.pointer("/artifact/artifactId"))
                .ok()
                .flatten()
                .is_none()
                || object
                    .pointer("/artifact/parts")
                    .and_then(Value::as_array)
                    .is_none_or(|parts| parts.is_empty()))
        {
            return Err("Invalid A2A artifact");
        }
        if expected_task.is_some_and(|expected| task != Some(expected))
            || expected_context
                .is_some_and(|expected| context.is_some_and(|actual| actual != expected))
        {
            return Err("A2A response changed task/context");
        }
        let ids: Vec<_> = [("task", task), ("context", context)]
            .into_iter()
            .filter_map(|(kind, id)| {
                id.map(|id| (binding.to_vec(), kind.to_owned(), id.to_owned()))
            })
            .collect();
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "A2A ownership unavailable")?;
        let now = Instant::now();
        // Only inspect identifiers carried by this event. Full expiry cleanup
        // belongs to per-request reservation, not every artifact chunk.
        for id in &ids {
            if entries.get(id).is_some_and(|owner| owner.expires <= now) {
                entries.remove(id);
            }
        }
        if ids.iter().any(|id| {
            entries
                .get(id)
                .is_some_and(|owner| owner.principal != principal)
        }) {
            return Err("A2A response belongs to another caller");
        }
        let new_ids = ids.iter().filter(|id| !entries.contains_key(*id)).count();
        let credit = reservation.as_ref().map_or(0, |r| r.remaining.min(new_ids));
        if entries.len() + self.reserved.load(Ordering::Relaxed) + new_ids - credit > 4096 {
            return Err("A2A ownership capacity reached");
        }
        if let Some(reservation) = reservation {
            reservation.remaining -= credit;
            self.reserved.fetch_sub(credit, Ordering::Relaxed);
        }
        for id in ids {
            entries.insert(
                id,
                Owner {
                    principal: principal.into(),
                    expires: Instant::now() + Duration::from_secs(3600),
                },
            );
        }
        Ok(())
    }
}
fn error(status: StatusCode, id: Value, code: i32, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("cache-control", "no-store"))
        .insert_header(("a2a-version", "1.0"))
        .json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))
}
pub fn configure_routes(cfg: &mut web::ServiceConfig, max_body_size: usize) {
    cfg.service(web::resource("/.well-known/agent-card.json").route(web::get().to(card)));
    cfg.service(
        web::resource("/a2a/{agent_name}")
            .app_data(web::PayloadConfig::new(max_body_size))
            .route(web::post().to(proxy)),
    );
    cfg.service(
        web::resource("/a2a/{agent_name}/.well-known/agent-card.json").route(web::get().to(card)),
    );
}
fn authenticated(req: &HttpRequest, name: &str) -> Result<String, HttpResponse> {
    let key = get_authenticated_api_key(req);
    let user = get_authenticated_user(req);
    let principal = if let Some(key) = &key {
        format!("key:{}", key.metadata.id)
    } else if let Some(user) = &user {
        format!("user:{}", user.id())
    } else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            Value::Null,
            -32000,
            "A2A requires gateway authentication",
        ));
    };
    if !check_permission(user.as_ref(), key.as_ref(), &format!("a2a.{name}"))
        || !matches!(api_key_allows_endpoint(key.as_ref(), req.path()), Ok(true))
    {
        return Err(error(
            StatusCode::FORBIDDEN,
            Value::Null,
            -32000,
            "A2A agent access denied",
        ));
    }
    Ok(principal)
}
async fn card(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let runtime = state.pin_runtime();
    let name = if let Some(name) = req.match_info().get("agent_name") {
        name
    } else {
        let mut enabled = runtime
            .config
            .gateway
            .a2a_agents
            .iter()
            .filter(|(_, agent)| agent.enabled);
        match (enabled.next(), enabled.next()) {
            (Some((name, _)), None) => name.as_str(),
            _ => {
                return error(
                    StatusCode::NOT_FOUND,
                    Value::Null,
                    -32601,
                    "Use a configured Agent Card URL",
                );
            }
        }
    };
    if let Err(response) = authenticated(&req, name) {
        return response;
    }
    let Some(agent) = runtime
        .config
        .gateway
        .a2a_agents
        .get(name)
        .filter(|a| a.enabled)
    else {
        return error(
            StatusCode::NOT_FOUND,
            Value::Null,
            -32601,
            "Agent not found",
        );
    };
    let mut schemes = serde_json::Map::new();
    let mut requirements = Vec::new();
    if runtime.config.gateway.auth.enable_api_key {
        schemes.insert(
            "gateway_key".into(),
            json!({"apiKeySecurityScheme":{"location":"header","name":runtime.config.gateway.auth.api_key_header}}),
        );
        requirements.push(json!({"schemes":{"gateway_key":{"list":[]}}}));
    }
    if runtime.config.gateway.auth.enable_jwt {
        schemes.insert(
            "gateway_jwt".into(),
            json!({"httpAuthSecurityScheme":{"scheme":"Bearer","bearerFormat":"JWT"}}),
        );
        requirements.push(json!({"schemes":{"gateway_jwt":{"list":[]}}}));
    }
    // Card describes this gateway endpoint, not the credential-bearing upstream URL.
    let connection = req.connection_info();
    let endpoint = format!("{}://{}/a2a/{name}", connection.scheme(), connection.host());
    HttpResponse::Ok().insert_header(("cache-control", "private, no-store")).json(json!({
        "name":name,"description":agent.description.as_deref().unwrap_or(name),"version":"1.0",
        "supportedInterfaces":[{"url":endpoint,"protocolBinding":"JSONRPC","protocolVersion":"1.0"}],
        "capabilities":{"streaming":agent.capabilities.streaming,"pushNotifications":false,"extendedAgentCard":false},
        "securitySchemes":schemes,
        "securityRequirements":requirements,
        "defaultInputModes":card_modes(&agent.capabilities.input_types),"defaultOutputModes":card_modes(&agent.capabilities.output_types),
        "skills":[{"id":"gateway","name":name,"description":agent.description.as_deref().unwrap_or(name),"tags":["a2a"]}]
    }))
}
fn card_modes(modes: &[String]) -> Vec<String> {
    if modes.is_empty() {
        vec!["text/plain".into()]
    } else {
        modes.to_vec()
    }
}
async fn proxy(req: HttpRequest, body: web::Bytes, state: web::Data<AppState>) -> HttpResponse {
    let name = req.match_info().get("agent_name").unwrap_or("");
    let principal = match authenticated(&req, name) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let runtime = state.pin_runtime();
    let Some(agent) = runtime
        .config
        .gateway
        .a2a_agents
        .get(name)
        .filter(|a| a.enabled)
    else {
        return error(
            StatusCode::NOT_FOUND,
            Value::Null,
            -32601,
            "Agent not found",
        );
    };
    if req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| {
            !v.split(';')
                .next()
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Value::Null,
            -32600,
            "A2A requires application/json",
        );
    }
    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return error(StatusCode::BAD_REQUEST, Value::Null, -32700, "Invalid JSON"),
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !(id.is_string() || id.is_number())
    {
        return error(
            StatusCode::BAD_REQUEST,
            id,
            -32600,
            "Expected a JSON-RPC request with ID",
        );
    }
    let query_version = url::form_urlencoded::parse(req.query_string().as_bytes())
        .find(|(name, _)| name.eq_ignore_ascii_case("A2A-Version"))
        .map(|(_, value)| value.into_owned());
    let version = req
        .headers()
        .get("a2a-version")
        .and_then(|v| v.to_str().ok())
        .or(query_version.as_deref());
    if version != Some("1.0") {
        return error(
            StatusCode::BAD_REQUEST,
            id,
            -32009,
            "Only A2A-Version: 1.0 is supported",
        );
    }
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    let send = matches!(method, "SendMessage" | "SendStreamingMessage");
    let stream = matches!(method, "SendStreamingMessage" | "SubscribeToTask");
    let subscribe = method == "SubscribeToTask";
    if !send && !matches!(method, "GetTask" | "CancelTask" | "SubscribeToTask") {
        return error(
            StatusCode::OK,
            id,
            -32601,
            "A2A method not supported by this gateway",
        );
    }
    if stream && !agent.capabilities.streaming {
        return error(StatusCode::OK, id, -32004, "Agent capability is disabled");
    }
    let params = &value["params"];
    if !params.is_object()
        || params.get("tenant").is_some()
        || params
            .pointer("/configuration/taskPushNotificationConfig")
            .is_some()
        || params
            .pointer("/configuration/pushNotificationConfig")
            .is_some()
    {
        return error(
            StatusCode::BAD_REQUEST,
            id,
            -32602,
            "Invalid or unsupported A2A parameters",
        );
    }
    // Presentation and capability edits do not change the upstream account.
    let mut account =
        json!({"name":name,"url":agent.url,"api_key":agent.api_key,"headers":agent.headers});
    account.sort_all_objects();
    let binding = match serde_json::to_vec(&account) {
        Ok(bytes) => Sha256::digest(bytes).to_vec(),
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                id,
                -32603,
                "Invalid A2A account configuration",
            );
        }
    };
    let message = &params["message"];
    let task = if send {
        message.get("taskId")
    } else {
        params.get("id")
    };
    let context = if send { message.get("contextId") } else { None };
    let task = match identifier(task) {
        Ok(t) => t,
        Err(()) => return error(StatusCode::BAD_REQUEST, id, -32602, "Invalid task ID"),
    };
    let context = match identifier(context) {
        Ok(t) => t,
        Err(()) => return error(StatusCode::BAD_REQUEST, id, -32602, "Invalid context ID"),
    };
    if !send && task.is_none()
        || task.is_some_and(|id| !state.a2a_tasks.owns(&binding, &principal, "task", id))
        || context.is_some_and(|id| !state.a2a_tasks.owns(&binding, &principal, "context", id))
    {
        return error(StatusCode::OK, id, -32001, "Task/context not found");
    }
    if let Some(references) = message.get("referenceTaskIds")
        && !references.as_array().is_some_and(|ids| {
            ids.iter().all(|id| {
                id.as_str()
                    .is_some_and(|id| state.a2a_tasks.owns(&binding, &principal, "task", id))
            })
        })
    {
        return error(StatusCode::OK, id, -32001, "Referenced task not found");
    }
    if send
        && (!message.is_object()
            || message.get("role").and_then(Value::as_str) != Some("ROLE_USER")
            || message
                .get("messageId")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            || message
                .get("parts")
                .and_then(Value::as_array)
                .is_none_or(|p| p.is_empty()))
    {
        return error(
            StatusCode::BAD_REQUEST,
            id,
            -32602,
            "Client ROLE_USER, message ID and parts are required",
        );
    }
    let client =
        match ProviderHttpClient::streaming_no_redirect(ProviderEndpointPolicy::public_only()) {
            Ok(c) => c,
            Err(_) => {
                return error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    id,
                    -32603,
                    "A2A transport unavailable",
                );
            }
        };
    #[cfg(test)]
    let client = state.a2a_tasks.client.clone().unwrap_or(client);
    let mut request = match client.request(reqwest::Method::POST, &agent.url) {
        Ok(r) => r,
        Err(_) => return error(StatusCode::BAD_GATEWAY, id, -32603, "A2A endpoint rejected"),
    };
    for (name, value) in &agent.headers {
        request = request.header(name, value);
    }
    if let Some(key) = &agent.api_key {
        request = request.bearer_auth(key);
    }
    request = request
        .header("a2a-version", "1.0")
        .header("content-type", "application/json")
        .header(
            "accept",
            if stream {
                "text/event-stream"
            } else {
                "application/json"
            },
        )
        .json(&value);
    // Extensions remain opaque, but cannot change the configured upstream endpoint/account.
    if !(runtime.config.gateway.auth.enable_api_key
        && runtime
            .config
            .gateway
            .auth
            .api_key_header
            .eq_ignore_ascii_case("a2a-extensions"))
        && let Some(value) = req.headers().get("a2a-extensions")
    {
        request = request.header("a2a-extensions", value.as_bytes());
    }
    let mut reservation = if send {
        let slots = usize::from(task.is_none()) + usize::from(context.is_none());
        match state.a2a_tasks.reserve(slots) {
            Ok(reservation) => Some(reservation),
            Err(message) => return error(StatusCode::SERVICE_UNAVAILABLE, id, -32000, message),
        }
    } else {
        None
    };
    let deadline = tokio::time::Instant::now() + Duration::from_millis(agent.timeout_ms);
    let mut upstream = match tokio::time::timeout_at(deadline, request.send()).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                -32603,
                "A2A upstream connection failed",
            );
        }
        Err(_) => {
            return error(
                StatusCode::GATEWAY_TIMEOUT,
                id,
                -32603,
                "A2A upstream timed out",
            );
        }
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = HttpResponse::build(status);
    for name in [
        "content-type",
        "retry-after",
        "a2a-version",
        "a2a-extensions",
    ] {
        if let Some(value) = upstream.headers().get(name) {
            response.insert_header((name, value.as_bytes()));
        }
    }
    response.insert_header(("cache-control", "no-store"));
    let owners = state.a2a_tasks.clone();
    let task = task.map(str::to_owned);
    let context = context.map(str::to_owned);
    let limit = runtime.config.gateway.server.max_body_size;
    let idle_timeout_secs = runtime.config.gateway.server.stream_idle_timeout;
    if status.is_success()
        && upstream
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| {
                s.split(';')
                    .next()
                    .is_some_and(|media| media.trim().eq_ignore_ascii_case("text/event-stream"))
            })
    {
        if !stream {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                -32006,
                "Unexpected A2A event stream",
            );
        }
        let events = async_stream::try_stream! {
            let mut buffer = Vec::new();
            let mut scan_from = 0;
            let mut task_seen = false;
            let mut first_frame = true;
            let mut completed = false;
            let mut expected_task = task.clone();
            let mut expected_context = context.clone();
            'events: loop {
                let next = if idle_timeout_secs == 0 {
                    upstream.chunk().await
                } else {
                    tokio::time::timeout(Duration::from_secs(idle_timeout_secs), upstream.chunk()).await
                        .map_err(|_| actix_web::error::ErrorGatewayTimeout("A2A stream idle timeout"))?
                };
                let Some(chunk) = next.map_err(|_| actix_web::error::ErrorBadGateway("A2A stream interrupted"))? else { break; };
                buffer.extend_from_slice(&chunk);
                while let Some(end) = event_boundary(&buffer, scan_from) {
                    scan_from = 0;
                    if end > limit { Err(actix_web::error::ErrorBadGateway("A2A event too large"))?; }
                    let frame: Vec<_> = buffer.drain(..end).collect();
                    let text = std::str::from_utf8(&frame).map_err(|_| actix_web::error::ErrorBadGateway("Invalid A2A event encoding"))?;
                    let text = if first_frame { text.strip_prefix('\u{feff}').unwrap_or(text) } else { text };
                    first_frame = false;
                    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
                    let data = normalized.lines().filter_map(|line| line.strip_prefix("data:").map(|s| s.strip_prefix(' ').unwrap_or(s))).collect::<Vec<_>>().join("\n");
                    if !data.is_empty() {
                        let value: Value = serde_json::from_str(&data).map_err(|_| actix_web::error::ErrorBadGateway("Invalid A2A event JSON"))?;
                        if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || value.get("id") != Some(&id) {
                            Err(actix_web::error::ErrorBadGateway("A2A response ID mismatch"))?;
                        }
                        let mut terminal = value.get("error").is_some();
                        if !terminal {
                            let result = value.get("result").ok_or_else(|| actix_web::error::ErrorBadGateway("Missing A2A stream result"))?;
                            let variants = ["task", "message", "statusUpdate", "artifactUpdate"].iter().filter(|key| result.get(**key).is_some()).count();
                            if variants != 1 || (!task_seen && result.get("task").is_none() && (subscribe || result.get("message").is_none()))
                                || (task_seen && (result.get("task").is_some() || result.get("message").is_some())) {
                                Err(actix_web::error::ErrorBadGateway("Invalid A2A stream sequence"))?;
                            }
                            terminal = result.get("message").is_some()
                                || result.get("task").or_else(|| result.get("statusUpdate"))
                                    .and_then(|task| task.pointer("/status/state"))
                                    .and_then(Value::as_str)
                                    .is_some_and(|state| matches!(state, "TASK_STATE_COMPLETED" | "TASK_STATE_FAILED" | "TASK_STATE_CANCELED" | "TASK_STATE_REJECTED"));
                            task_seen = true;
                        }
                        if let Some(initial) = value.pointer("/result/task") {
                            let initial_task = identifier(initial.get("id")).ok().flatten().ok_or_else(|| actix_web::error::ErrorBadGateway("Missing A2A task identifier"))?;
                            let initial_context = identifier(initial.get("contextId")).map_err(|_| actix_web::error::ErrorBadGateway("Invalid A2A context identifier"))?;
                            if expected_task.as_deref().is_some_and(|id| id != initial_task)
                                || expected_context.as_deref().is_some_and(|id| initial_context.is_some_and(|actual| id != actual)) {
                                Err(actix_web::error::ErrorBadGateway("A2A response changed task/context"))?;
                            }
                            expected_task = Some(initial_task.to_owned());
                            if let Some(context) = initial_context { expected_context = Some(context.to_owned()); }
                        }
                        owners.observe(&binding, &principal, &value, expected_task.as_deref(), expected_context.as_deref(), reservation.as_mut()).map_err(actix_web::error::ErrorBadGateway)?;
                        if expected_context.is_none() {
                            expected_context = value.get("result").and_then(|result| result.get("statusUpdate").or_else(|| result.get("artifactUpdate"))).and_then(|event| event.get("contextId")).and_then(Value::as_str).map(str::to_owned);
                        }
                        if terminal {
                            completed = true;
                            buffer.clear();
                            yield web::Bytes::from(frame);
                            break 'events;
                        }
                    }
                    yield web::Bytes::from(frame);
                }
                // Only revisit a possible delimiter split across chunks.
                scan_from = buffer.len().saturating_sub(3);
                if buffer.len() > limit { Err(actix_web::error::ErrorBadGateway("A2A event too large"))?; }
            }
            if !buffer.is_empty() || !completed { Err(actix_web::error::ErrorBadGateway("A2A stream ended before a final response"))?; }
        };
        return response.streaming::<_, actix_web::Error>(events);
    }
    if status.is_success()
        && !upstream
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| {
                s.split(';')
                    .next()
                    .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
            })
    {
        return error(
            StatusCode::BAD_GATEWAY,
            id,
            -32006,
            "A2A response requires application/json",
        );
    }
    let read = async {
        let mut bytes = Vec::new();
        while let Some(chunk) = upstream.chunk().await.map_err(|_| ())? {
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok::<_, ()>(bytes)
    };
    let bytes = match tokio::time::timeout_at(deadline, read).await {
        Ok(Ok(b)) => b,
        _ => {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                -32006,
                "Invalid or incomplete A2A response",
            );
        }
    };
    if !status.is_success() {
        return response.body(bytes);
    }
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                -32006,
                "Invalid A2A response JSON",
            );
        }
    };
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || value.get("id") != Some(&id) {
        return error(
            StatusCode::BAD_GATEWAY,
            id,
            -32006,
            "A2A response ID mismatch",
        );
    }
    if stream && value.get("result").is_some() {
        return error(
            StatusCode::BAD_GATEWAY,
            id,
            -32006,
            "A2A streaming method requires an event stream",
        );
    }
    if let Some(result) = value.get("result") {
        let valid = if send {
            result.is_object()
                && (result.get("task").is_some() ^ result.get("message").is_some())
                && result
                    .get("task")
                    .or_else(|| result.get("message"))
                    .is_some_and(Value::is_object)
        } else {
            result.is_object()
                && result.get("task").is_none()
                && result.get("message").is_none()
                && identifier(result.get("id")).ok().flatten().is_some()
                && result.get("status").is_some_and(Value::is_object)
        };
        if !valid {
            return error(
                StatusCode::BAD_GATEWAY,
                id,
                -32006,
                "Invalid A2A result for requested method",
            );
        }
    }
    if let Err(message) = owners.observe(
        &binding,
        &principal,
        &value,
        task.as_deref(),
        context.as_deref(),
        reservation.as_mut(),
    ) {
        return error(StatusCode::BAD_GATEWAY, id, -32006, message);
    }
    response.body(bytes)
}
fn identifier(value: Option<&Value>) -> Result<Option<&str>, ()> {
    match value {
        None => Ok(None),
        Some(Value::String(id)) if !id.is_empty() && id.len() <= 4096 => Ok(Some(id)),
        _ => Err(()),
    }
}
fn event_boundary(bytes: &[u8], start: usize) -> Option<usize> {
    (start..bytes.len()).find_map(|i| {
        if bytes[i..].starts_with(b"\r\n\r\n") {
            Some(i + 4)
        } else if bytes[i..].starts_with(b"\n\n") || bytes[i..].starts_with(b"\r\r") {
            Some(i + 2)
        } else {
            None
        }
    })
}

#[cfg(test)]
#[path = "a2a_tests.rs"]
mod tests;
