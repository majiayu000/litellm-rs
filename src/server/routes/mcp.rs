//! Streamable HTTP forwarding. Gateway credentials never become upstream credentials.
use super::ai::{
    api_key_allows_endpoint, check_permission, get_authenticated_api_key, get_authenticated_user,
};
use crate::core::net::ProviderEndpointPolicy;
use crate::server::state::AppState;
use crate::utils::net::http::ProviderHttpClient;
use actix_web::{
    HttpRequest, HttpResponse,
    http::{Method, StatusCode},
    web,
};
use futures::StreamExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Session {
    owner: String,
    binding: Vec<u8>,
    upstream: Option<String>,
    expires: Instant,
}

/// Bounded process-local sessions. A restart requires clients to initialize again.
#[derive(Default)]
pub(crate) struct Sessions {
    entries: Mutex<HashMap<String, Session>>,
    #[cfg(test)]
    client: Option<ProviderHttpClient>,
}

fn error(status: StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status)
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
    // Include the entire configured account in the binding. A credential or URL
    // change must never send an existing session to a different upstream account.
    let binding = match serde_json::to_vec(server) {
        Ok(bytes) => Sha256::digest(bytes).to_vec(),
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid MCP configuration",
            );
        }
    };
    let accept = req
        .headers()
        .get("accept")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if req.method() != Method::DELETE
        && (!accept.contains("text/event-stream")
            || (req.method() == Method::POST && !accept.contains("application/json")))
    {
        return error(
            StatusCode::NOT_ACCEPTABLE,
            "Accept must include the MCP response types",
        );
    }
    let initialize = if req.method() == Method::POST {
        if !req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim() == "application/json")
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
        match entries
            .get(token)
            .filter(|s| s.owner == owner && s.binding == binding && s.expires > Instant::now())
        {
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
        if let Some(value) = req.headers().get(name) {
            outgoing = outgoing.header(name, value.as_bytes());
        }
    }
    if let Some(upstream) = session.as_ref().and_then(|s| s.upstream.as_ref()) {
        outgoing = outgoing.header("mcp-session-id", upstream);
    }
    let outgoing = outgoing.body(body.to_vec());
    let upstream =
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
        "retry-after",
        "mcp-protocol-version",
        "allow",
    ] {
        if let Some(value) = upstream.headers().get(name) {
            response.insert_header((name, value.as_bytes()));
        }
    }
    response.insert_header(("cache-control", "no-store"));
    if initialize && status == StatusCode::OK {
        let upstream_id = match upstream.headers().get("mcp-session-id") {
            Some(value) => match value.to_str() {
                Ok(value)
                    if value.len() <= 4096 && value.bytes().all(|b| (0x21..=0x7e).contains(&b)) =>
                {
                    Some(value.to_owned())
                }
                _ => return error(StatusCode::BAD_GATEWAY, "Invalid upstream MCP session ID"),
            },
            None => None,
        };
        let mut entries = match state.mcp_sessions.entries.lock() {
            Ok(entries) => entries,
            Err(_) => return error(StatusCode::SERVICE_UNAVAILABLE, "MCP sessions unavailable"),
        };
        entries.retain(|_, session| session.expires > Instant::now());
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
                upstream: upstream_id,
                expires: Instant::now() + Duration::from_secs(3600),
            },
        );
        response.insert_header(("mcp-session-id", token));
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
    // The response stream owns reqwest's body. Dropping the downstream stream
    // closes the upstream body without buffering SSE or rewriting RPC errors.
    response.streaming(upstream.bytes_stream().map(|chunk| {
        chunk.map_err(|_| actix_web::error::ErrorBadGateway("MCP upstream stream interrupted"))
    }))
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
