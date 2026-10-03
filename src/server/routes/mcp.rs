//! Streamable HTTP forwarding. Gateway credentials never become upstream credentials.
use super::ai::{
    api_key_allows_endpoint, check_permission, get_authenticated_api_key, get_authenticated_user,
};
use crate::core::mcp::config::McpServerConfig;
use crate::core::net::ProviderEndpointPolicy;
use crate::server::state::AppState;
use crate::utils::net::http::ProviderHttpClient;
use actix_web::{
    HttpRequest, HttpResponse,
    http::{
        Method, StatusCode,
        header::{Accept, Header, Quality},
    },
    web,
};
use futures::StreamExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Session {
    owner: String,
    binding: Vec<u8>,
    upstream: Option<String>,
    protocol_version: Option<String>,
    expires: Instant,
    pending: bool,
    server: Arc<McpServerConfig>,
}

/// Bounded process-local sessions. A restart requires clients to initialize again.
#[derive(Default)]
pub(crate) struct Sessions {
    entries: Mutex<HashMap<String, Session>>,
    reaper_started: AtomicBool,
    #[cfg(test)]
    client: Option<ProviderHttpClient>,
}

fn account_binding(name: &str, server: &McpServerConfig) -> Vec<u8> {
    let mut account =
        json!({"name":name,"url":server.url,"auth":server.auth,"headers":server.static_headers});
    account.sort_all_objects();
    Sha256::digest(account.to_string().as_bytes()).to_vec()
}

fn start_reaper(state: &AppState) {
    if state
        .mcp_sessions
        .reaper_started
        .swap(true, Ordering::AcqRel)
    {
        return;
    }
    let sessions = Arc::downgrade(&state.mcp_sessions);
    let runtime = Arc::downgrade(&state.runtime);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let (Some(sessions), Some(runtime)) = (sessions.upgrade(), runtime.upgrade()) else {
                break;
            };
            let revision = runtime.load();
            reap_sessions(&sessions, &revision.config.gateway.mcp_servers).await;
        }
    });
}

// Retain the occupied slot until the bounded DELETE attempt completes. One reaper
// and 32 concurrent requests cap cleanup work, including repeated config rotations.
async fn reap_sessions(sessions: &Sessions, servers: &HashMap<String, McpServerConfig>) {
    let stale: Vec<_> = match sessions.entries.lock() {
        Ok(entries) => entries
            .iter()
            .filter(|(_, session)| {
                !session.pending
                    && (session.expires <= Instant::now()
                        || servers
                            .get(&session.server.name)
                            .filter(|server| server.enabled)
                            .is_none_or(|server| {
                                account_binding(&server.name, server) != session.binding
                            }))
            })
            .map(|(token, session)| (token.clone(), session.clone()))
            .collect(),
        Err(_) => return,
    };
    futures::stream::iter(stale)
        .for_each_concurrent(32, |(token, session)| async move {
            if let Some(upstream) = &session.upstream {
                let client = ProviderHttpClient::streaming_no_redirect(
                    ProviderEndpointPolicy::public_only(),
                );
                #[cfg(test)]
                let client = sessions.client.clone().map(Ok).unwrap_or(client);
                let cleanup = async {
                    let client = client.map_err(|_| ())?;
                    let mut request = client
                        .request(reqwest::Method::DELETE, &session.server.url)
                        .map_err(|_| ())?;
                    for (name, value) in &session.server.static_headers {
                        request = request.header(name, value);
                    }
                    if let Some(auth) = &session.server.auth
                        && let Some(value) = auth.get_header_value()
                    {
                        request = request.header(auth.get_header_name(), value);
                    }
                    let response = request
                        .header("mcp-session-id", upstream)
                        .header(
                            "mcp-protocol-version",
                            session.protocol_version.as_deref().ok_or(())?,
                        )
                        .send()
                        .await
                        .map_err(|_| ())?;
                    if response.status().is_success()
                        || response.status() == reqwest::StatusCode::NOT_FOUND
                    {
                        Ok(())
                    } else {
                        Err(())
                    }
                };
                if !matches!(
                    tokio::time::timeout(
                        Duration::from_millis(session.server.timeout_ms.min(5000)),
                        cleanup
                    )
                    .await,
                    Ok(Ok(()))
                ) {
                    tracing::warn!(
                        "MCP upstream session cleanup failed; remote expiration is required"
                    );
                }
            }
            if let Ok(mut entries) = sessions.entries.lock() {
                entries.remove(&token);
            }
        })
        .await;
}

// Reserve before contacting the upstream. Cancellation and every error path release
// an uncommitted reservation, so concurrent initializations cannot exceed the limit.
struct PendingSession {
    sessions: Arc<Sessions>,
    token: String,
    committed: bool,
}
impl Drop for PendingSession {
    fn drop(&mut self) {
        if !self.committed
            && let Ok(mut entries) = self.sessions.entries.lock()
        {
            if let Some(session) = entries.get_mut(&self.token)
                && session.upstream.is_some()
            {
                session.pending = false;
                session.expires = Instant::now();
            } else {
                entries.remove(&self.token);
            }
        }
    }
}

fn error(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("cache-control", "no-store"))
        .json(json!({"jsonrpc":"2.0","id":null,"error":{"code":-32000,"message":message}}))
}

/// Mount the single-server and named-server Streamable HTTP endpoints.
pub fn configure_routes(cfg: &mut web::ServiceConfig, max_body_size: usize) {
    for path in ["/mcp", "/{server_name}/mcp"] {
        cfg.service(
            web::resource(path)
                .app_data(web::PayloadConfig::new(max_body_size))
                .route(web::post().to(proxy))
                .route(web::get().to(proxy))
                .route(web::delete().to(proxy)),
        );
    }
}

async fn proxy(req: HttpRequest, body: web::Bytes, state: web::Data<AppState>) -> HttpResponse {
    let user = get_authenticated_user(&req);
    let key = get_authenticated_api_key(&req);
    let owner = if let Some(key) = &key {
        format!("key:{}", key.metadata.id)
    } else if let Some(user) = &user {
        format!("user:{}", user.id())
    } else {
        return error(
            StatusCode::UNAUTHORIZED,
            "MCP requires gateway authentication",
        );
    };
    let runtime = state.pin_runtime();
    let config = &runtime.config.gateway;
    if let Some(origin) = req.headers().get("origin") {
        let allowed = origin.to_str().ok().is_some_and(|origin| {
            origin != "null"
                && config
                    .server
                    .cors
                    .allowed_origins
                    .iter()
                    .any(|allowed| allowed != "*" && allowed == origin)
        });
        if !allowed {
            return error(StatusCode::FORBIDDEN, "Origin is not allowed");
        }
    }
    let server_name = match req.match_info().get("server_name") {
        Some(name) => name,
        None => {
            let mut enabled = config
                .mcp_servers
                .iter()
                .filter(|(_, server)| server.enabled);
            match (enabled.next(), enabled.next()) {
                (Some((name, _)), None) => name,
                _ => {
                    return error(
                        StatusCode::BAD_REQUEST,
                        "Select a configured server at /{server_name}/mcp",
                    );
                }
            }
        }
    };
    if !check_permission(user.as_ref(), key.as_ref(), &format!("mcp.{server_name}"))
        || !matches!(api_key_allows_endpoint(key.as_ref(), req.path()), Ok(true))
    {
        return error(StatusCode::FORBIDDEN, "MCP server access denied");
    }
    let Some(server) = config
        .mcp_servers
        .get(server_name)
        .filter(|server| server.enabled)
    else {
        return error(StatusCode::NOT_FOUND, "MCP server not found");
    };
    start_reaper(&state);
    // Bind only upstream identity; harmless timeout/description edits preserve sessions.
    let binding = account_binding(server_name, server);
    let accept = Accept::parse(&req).ok();
    let accepts = |kind: &str, subtype: &str| {
        accept.as_ref().is_some_and(|accept| {
            accept.0.iter().any(|entry| {
                entry.quality > Quality::ZERO
                    && entry.item.type_().as_str().eq_ignore_ascii_case(kind)
                    && entry.item.subtype().as_str().eq_ignore_ascii_case(subtype)
            })
        })
    };
    if req.method() != Method::DELETE
        && (!accepts("text", "event-stream")
            || (req.method() == Method::POST && !accepts("application", "json")))
    {
        return error(
            StatusCode::NOT_ACCEPTABLE,
            "Accept must include the MCP response types",
        );
    }
    let mut outgoing_body = body.to_vec();
    let mut request_id = Value::Null;
    let mut requested_version = None;
    let initialize = if req.method() == Method::POST {
        if !req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
            })
        {
            return error(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "MCP POST requires application/json",
            );
        }
        let message: Value = match serde_json::from_slice(&body) {
            Ok(message) => message,
            Err(_) => return error(StatusCode::BAD_REQUEST, "Invalid JSON-RPC body"),
        };
        if !message.is_object()
            || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !(message.get("method").is_some_and(Value::is_string)
                || message.get("result").is_some()
                || message.get("error").is_some())
        {
            return error(
                StatusCode::BAD_REQUEST,
                "Expected a single JSON-RPC message",
            );
        }
        request_id = message.get("id").cloned().unwrap_or(Value::Null);
        requested_version = message
            .pointer("/params/protocolVersion")
            .and_then(Value::as_str)
            .filter(|version| valid_protocol_version(version))
            .map(str::to_owned);
        outgoing_body = message.to_string().into_bytes();
        message.get("method").and_then(Value::as_str) == Some("initialize")
    } else {
        false
    };
    let token = req
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok());
    let session = if initialize {
        if token.is_some() {
            return error(StatusCode::BAD_REQUEST, "Initialize without a session ID");
        }
        None
    } else {
        let Some(token) = token else {
            return error(StatusCode::BAD_REQUEST, "MCP session ID required");
        };
        let entries = match state.mcp_sessions.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "MCP sessions unavailable"),
        };
        match entries.get(token).filter(|s| {
            !s.pending && s.owner == owner && s.binding == binding && s.expires > Instant::now()
        }) {
            Some(session) => Some(session.clone()),
            None => {
                return error(
                    StatusCode::NOT_FOUND,
                    "MCP session not found; initialize again",
                );
            }
        }
    };
    // Stateless upstreams cannot safely identify replay cursors for shared credentials.
    if req.headers().contains_key("last-event-id")
        && session.as_ref().is_none_or(|s| s.upstream.is_none())
    {
        return error(
            StatusCode::BAD_REQUEST,
            "Resumption requires an upstream session",
        );
    }
    let client =
        match ProviderHttpClient::streaming_no_redirect(ProviderEndpointPolicy::public_only()) {
            Ok(client) => client,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "MCP transport unavailable"),
        };
    #[cfg(test)]
    let client = state.mcp_sessions.client.clone().unwrap_or(client);
    let method = match *req.method() {
        Method::GET => reqwest::Method::GET,
        Method::DELETE => reqwest::Method::DELETE,
        _ => reqwest::Method::POST,
    };
    let mut outgoing = match client.request(method, &server.url) {
        Ok(request) => request,
        Err(_) => return error(StatusCode::BAD_GATEWAY, "MCP upstream endpoint rejected"),
    };
    for (name, value) in &server.static_headers {
        outgoing = outgoing.header(name, value);
    }
    if let Some(auth) = &server.auth
        && let Some(value) = auth.get_header_value()
    {
        outgoing = outgoing.header(auth.get_header_name(), value);
    }
    for name in [
        "accept",
        "content-type",
        "mcp-protocol-version",
        "last-event-id",
    ] {
        if config.auth.enable_api_key && name.eq_ignore_ascii_case(&config.auth.api_key_header) {
            continue;
        }
        for value in req.headers().get_all(name) {
            outgoing = outgoing.header(name, value.as_bytes());
        }
    }
    if let Some(upstream) = session.as_ref().and_then(|s| s.upstream.as_ref()) {
        outgoing = outgoing.header("mcp-session-id", upstream);
    }
    let mut reservation = if initialize {
        let mut entries = match state.mcp_sessions.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "MCP sessions unavailable"),
        };
        if entries
            .values()
            .filter(|session| session.owner == owner)
            .count()
            >= 128
        {
            return error(
                StatusCode::TOO_MANY_REQUESTS,
                "MCP session capacity reached for this caller",
            );
        }
        if entries.len() >= 4096 {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MCP session capacity reached",
            );
        }
        let token = uuid::Uuid::new_v4().to_string();
        entries.insert(
            token.clone(),
            Session {
                owner,
                binding,
                upstream: None,
                protocol_version: None,
                expires: Instant::now() + Duration::from_secs(3600),
                pending: true,
                server: Arc::new(server.clone()),
            },
        );
        Some(PendingSession {
            sessions: state.mcp_sessions.clone(),
            token,
            committed: false,
        })
    } else {
        None
    };
    let outgoing = outgoing
        .header("accept-encoding", "identity")
        .body(outgoing_body);
    let mut upstream =
        match tokio::time::timeout(Duration::from_millis(server.timeout_ms), outgoing.send()).await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return error(StatusCode::BAD_GATEWAY, "MCP upstream connection failed"),
            Err(_) => {
                return error(
                    StatusCode::GATEWAY_TIMEOUT,
                    "MCP upstream response timed out",
                );
            }
        };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = HttpResponse::build(status);
    for name in [
        "content-type",
        "content-encoding",
        "retry-after",
        "mcp-protocol-version",
        "allow",
    ] {
        if let Some(value) = upstream.headers().get(name) {
            response.insert_header((name, value.as_bytes()));
        }
    }
    response.insert_header(("cache-control", "no-store"));
    let mut initialized_body = None;
    let mut protocol_version = None;
    let event_stream = upstream
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
        });
    if initialize && status == StatusCode::OK {
        // Capture allocated upstream state before reading a body that can fail or be cancelled.
        let upstream_id = match upstream.headers().get("mcp-session-id") {
            Some(value) => match value.to_str() {
                Ok(value)
                    if !value.is_empty()
                        && value.len() <= 4096
                        && value.bytes().all(|b| (0x21..=0x7e).contains(&b)) =>
                {
                    Some(value.to_owned())
                }
                _ => return error(StatusCode::BAD_GATEWAY, "Invalid upstream MCP session ID"),
            },
            None => None,
        };
        if let Some(reservation) = &reservation {
            let mut entries = match state.mcp_sessions.entries.lock() {
                Ok(entries) => entries,
                Err(_) => {
                    return error(StatusCode::SERVICE_UNAVAILABLE, "MCP sessions unavailable");
                }
            };
            if let Some(session) = entries.get_mut(&reservation.token) {
                session.upstream = upstream_id;
                // Before negotiation completes, cleanup can only use the requested
                // version (or the transport specification's missing-header default).
                session.protocol_version = Some(
                    requested_version
                        .clone()
                        .or_else(|| {
                            req.headers()
                                .get("mcp-protocol-version")
                                .and_then(|v| v.to_str().ok())
                                .filter(|v| valid_protocol_version(v))
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| "2025-03-26".into()),
                );
            }
        }
        let read = async {
            let mut bytes = Vec::new();
            let mut parsed = 0;
            let mut scan_from = 0;
            while let Some(chunk) = upstream.chunk().await.map_err(|_| ())? {
                if bytes.len().saturating_add(chunk.len()) > config.server.max_body_size {
                    return Err(());
                }
                bytes.extend_from_slice(&chunk);
                if event_stream {
                    while let Some(end) = event_boundary(&bytes, scan_from) {
                        if let Some(result) =
                            initialize_result(&bytes[parsed..end], true, &request_id)?
                        {
                            return Ok((bytes, result));
                        }
                        parsed = end;
                        scan_from = end;
                    }
                    scan_from = bytes.len().saturating_sub(3).max(parsed);
                }
            }
            if event_stream {
                return Err(());
            }
            let result = initialize_result(&bytes, false, &request_id)?.ok_or(())?;
            Ok::<_, ()>((bytes, result))
        };
        let (bytes, result) =
            match tokio::time::timeout(Duration::from_millis(server.timeout_ms), read).await {
                Ok(Ok(result)) => result,
                _ => {
                    return error(
                        StatusCode::BAD_GATEWAY,
                        "Invalid or incomplete MCP initialize response",
                    );
                }
            };
        match result {
            Some(version) => {
                protocol_version = Some(version);
                initialized_body = Some(bytes);
            }
            None => return response.body(bytes),
        }
    }
    if initialize && status == StatusCode::OK {
        let mut entries = match state.mcp_sessions.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "MCP sessions unavailable"),
        };
        let Some(reservation) = reservation.as_mut() else {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MCP reservation unavailable",
            );
        };
        let Some(session) = entries.get_mut(&reservation.token) else {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MCP reservation unavailable",
            );
        };
        session.protocol_version = protocol_version;
        session.pending = false;
        session.expires = Instant::now() + Duration::from_secs(3600);
        reservation.committed = true;
        response.insert_header(("mcp-session-id", reservation.token.clone()));
    } else if let Some(token) = token {
        if status == StatusCode::NOT_FOUND
            || (req.method() == Method::DELETE && status.is_success())
        {
            if let Ok(mut entries) = state.mcp_sessions.entries.lock() {
                entries.remove(token);
            }
        } else if let Some(returned) = upstream.headers().get("mcp-session-id") {
            if returned.to_str().ok() != session.as_ref().and_then(|s| s.upstream.as_deref()) {
                return error(StatusCode::BAD_GATEWAY, "Upstream changed MCP session ID");
            }
            response.insert_header(("mcp-session-id", token));
        }
    }
    if !event_stream && let Some(bytes) = initialized_body.take() {
        return response.body(bytes);
    }
    let idle_timeout_secs = config.server.stream_idle_timeout;
    // Own the upstream body so downstream disconnects close its connection.
    let stream = async_stream::try_stream! {
        if let Some(bytes) = initialized_body { yield web::Bytes::from(bytes); }
        loop {
            let next = if !event_stream || idle_timeout_secs == 0 {
                upstream.chunk().await
            } else {
                tokio::time::timeout(Duration::from_secs(idle_timeout_secs), upstream.chunk()).await
                    .map_err(|_| actix_web::error::ErrorGatewayTimeout("MCP stream idle timeout"))?
            };
            let Some(chunk) = next.map_err(|_| actix_web::error::ErrorBadGateway("MCP upstream stream interrupted"))? else { break; };
            yield chunk;
        }
    };
    response.streaming::<_, actix_web::Error>(stream)
}

// Outer None: no matching response in this complete SSE frame. Inner None: RPC error.
fn initialize_result(
    bytes: &[u8],
    event_stream: bool,
    id: &Value,
) -> Result<Option<Option<String>>, ()> {
    let messages: Vec<Value> = if event_stream {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| ())?
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        text.split("\n\n")
            .filter_map(|event| {
                let data = event
                    .lines()
                    .filter_map(|line| {
                        line.strip_prefix("data:")
                            .map(|v| v.strip_prefix(' ').unwrap_or(v))
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (!data.is_empty()).then(|| serde_json::from_str(&data).map_err(|_| ()))
            })
            .collect::<Result<_, _>>()?
    } else {
        vec![serde_json::from_slice(bytes).map_err(|_| ())?]
    };
    let response = messages.iter().find(|message| {
        message.get("id") == Some(id)
            && message.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
    });
    let Some(response) = response else {
        return if event_stream { Ok(None) } else { Err(()) };
    };
    if response.get("error").is_some() {
        return Ok(Some(None));
    }
    let result = response.get("result").ok_or(())?;
    if result.get("protocolVersion").is_some_and(Value::is_string)
        && result.get("capabilities").is_some_and(Value::is_object)
        && result.get("serverInfo").is_some_and(Value::is_object)
    {
        let version = result["protocolVersion"].as_str().ok_or(())?;
        if !valid_protocol_version(version) {
            return Err(());
        }
        Ok(Some(Some(version.to_owned())))
    } else {
        Err(())
    }
}

fn valid_protocol_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && reqwest::header::HeaderValue::from_str(version).is_ok()
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
#[path = "mcp_tests.rs"]
mod tests;
