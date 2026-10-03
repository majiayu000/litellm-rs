//! Stateless MCP 2026-07-28 HTTP proxy. Gateway credentials stay at this boundary.
use super::ai::{
    api_key_allows_endpoint, check_permission, get_authenticated_api_key, get_authenticated_user,
};
use crate::core::net::ProviderEndpointPolicy;
use crate::server::state::AppState;
use crate::utils::net::http::ProviderHttpClient;
use actix_web::{
    HttpRequest, HttpResponse,
    http::{
        StatusCode,
        header::{Accept, Header, Quality},
    },
    web,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

const PROTOCOL_VERSION: &str = "2026-07-28";

/// Active HTTP requests only; entries disappear when their last body is dropped.
#[derive(Default)]
pub(crate) struct Inflight {
    owners: Mutex<HashMap<String, usize>>,
}
struct RequestPermit {
    inflight: Arc<Inflight>,
    owner: String,
}
impl Inflight {
    fn acquire(self: &Arc<Self>, owner: String) -> Result<RequestPermit, HttpResponse> {
        let mut owners = self.owners.lock().map_err(|_| {
            transport_error(StatusCode::SERVICE_UNAVAILABLE, "MCP admission unavailable")
        })?;
        if owners.get(&owner).copied().unwrap_or(0) >= 128 {
            return Err(transport_error(
                StatusCode::TOO_MANY_REQUESTS,
                "MCP caller concurrency limit reached",
            ));
        }
        if owners.values().sum::<usize>() >= 4096 {
            return Err(transport_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "MCP request capacity reached",
            ));
        }
        *owners.entry(owner.clone()).or_default() += 1;
        Ok(RequestPermit {
            inflight: self.clone(),
            owner,
        })
    }
}
impl Drop for RequestPermit {
    fn drop(&mut self) {
        if let Ok(mut owners) = self.inflight.owners.lock()
            && let Some(count) = owners.get_mut(&self.owner)
        {
            *count -= 1;
            if *count == 0 {
                owners.remove(&self.owner);
            }
        }
    }
}

fn rpc_error(status: StatusCode, id: &Value, code: i32, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("cache-control", "no-store"))
        .json(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))
}
fn transport_error(status: StatusCode, message: &str) -> HttpResponse {
    rpc_error(status, &Value::Null, -32603, message)
}

/// Mount the single-server and named-server stateless POST endpoints.
pub fn configure_routes(cfg: &mut web::ServiceConfig, max_body_size: usize) {
    for path in ["/mcp", "/{server_name}/mcp"] {
        cfg.service(
            web::resource(path)
                .app_data(web::PayloadConfig::new(max_body_size))
                .route(web::post().to(proxy)),
        );
    }
}

// Reject duplicates instead of letting routing and execution select different values.
fn single_header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    let mut values = req.headers().get_all(name);
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
fn decoded_name(value: &str) -> Option<String> {
    if let Some(encoded) = value
        .strip_prefix("=?base64?")
        .and_then(|v| v.strip_suffix("?="))
    {
        return String::from_utf8(STANDARD.decode(encoded).ok()?).ok();
    }
    (value.trim_matches([' ', '\t']) == value
        && value
            .bytes()
            .all(|b| (0x20..=0x7e).contains(&b) || b == b'\t'))
    .then(|| value.to_owned())
}

fn validate_request(req: &HttpRequest, message: &Value) -> Option<HttpResponse> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    if !message.is_object()
        || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !message.get("method").is_some_and(Value::is_string)
        || message.get("result").is_some()
        || message.get("error").is_some()
        || message
            .get("id")
            .is_some_and(|id| !id.is_string() && !id.is_number())
    {
        return Some(rpc_error(
            StatusCode::BAD_REQUEST,
            &Value::Null,
            -32600,
            "Expected a single JSON-RPC request or notification",
        ));
    }
    // Core HTTP has no client notifications in this revision. Extensions can define
    // them; the specification does not impose request metadata headers on notifications.
    if !message.as_object()?.contains_key("id") {
        return None;
    }
    let mismatch = || {
        Some(rpc_error(
            StatusCode::BAD_REQUEST,
            &id,
            -32020,
            "Required MCP header is missing, malformed or does not match the request",
        ))
    };
    let Some(version) = single_header(req, "mcp-protocol-version") else {
        return mismatch();
    };
    if single_header(req, "mcp-method") != message["method"].as_str() {
        return mismatch();
    }
    let meta = &message["params"]["_meta"];
    let Some(body_version) = meta["io.modelcontextprotocol/protocolVersion"].as_str() else {
        return Some(rpc_error(
            StatusCode::BAD_REQUEST,
            &id,
            -32602,
            "Protocol metadata is required",
        ));
    };
    if version != body_version {
        return mismatch();
    }
    if version != PROTOCOL_VERSION {
        return Some(HttpResponse::BadRequest().insert_header(("cache-control", "no-store"))
            .json(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32022,"message":"Unsupported protocol version","data":{"supported":[PROTOCOL_VERSION]}}})));
    }
    if !meta["io.modelcontextprotocol/clientCapabilities"].is_object() {
        return Some(rpc_error(
            StatusCode::BAD_REQUEST,
            &id,
            -32602,
            "Client capabilities are required",
        ));
    }
    let field = match message["method"].as_str() {
        Some("tools/call" | "prompts/get") => Some("name"),
        Some("resources/read") => Some("uri"),
        _ => None,
    };
    if let Some(field) = field {
        let Some(expected) = message["params"][field].as_str() else {
            return Some(rpc_error(
                StatusCode::BAD_REQUEST,
                &id,
                -32602,
                "Request name or URI must be a string",
            ));
        };
        if single_header(req, "mcp-name")
            .and_then(decoded_name)
            .as_deref()
            != Some(expected)
        {
            return mismatch();
        }
    }
    if matches!(
        message["method"].as_str(),
        Some("initialize" | "notifications/initialized")
    ) {
        return Some(rpc_error(
            StatusCode::NOT_FOUND,
            &id,
            -32601,
            "Initialization is not part of this protocol version",
        ));
    }
    None
}

async fn proxy(req: HttpRequest, body: web::Bytes, state: web::Data<AppState>) -> HttpResponse {
    let user = get_authenticated_user(&req);
    let key = get_authenticated_api_key(&req);
    if user.is_none() && key.is_none() {
        return transport_error(
            StatusCode::UNAUTHORIZED,
            "MCP requires gateway authentication",
        );
    }
    let runtime = state.pin_runtime();
    let config = &runtime.config.gateway;
    if req.headers().contains_key("origin") {
        let allowed = single_header(&req, "origin").is_some_and(|origin| {
            origin != "null"
                && config
                    .server
                    .cors
                    .allowed_origins
                    .iter()
                    .any(|allowed| allowed != "*" && allowed == origin)
        });
        if !allowed {
            return transport_error(StatusCode::FORBIDDEN, "Origin is not allowed");
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
                    return transport_error(
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
        return transport_error(StatusCode::FORBIDDEN, "MCP server access denied");
    }
    let Some(server) = config
        .mcp_servers
        .get(server_name)
        .filter(|server| server.enabled)
    else {
        return transport_error(StatusCode::NOT_FOUND, "MCP server not found");
    };
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
    if !accepts("text", "event-stream") || !accepts("application", "json") {
        return transport_error(
            StatusCode::NOT_ACCEPTABLE,
            "Accept must include the MCP response types",
        );
    }

    if !single_header(&req, "content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return transport_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "MCP POST requires application/json",
        );
    }
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(_) => {
            return rpc_error(
                StatusCode::BAD_REQUEST,
                &Value::Null,
                -32700,
                "Invalid JSON",
            );
        }
    };
    if let Some(error) = validate_request(&req, &message) {
        return error;
    }
    let owner = match (key.as_ref(), user.as_ref()) {
        (Some(key), _) => format!("key:{}", key.metadata.id),
        (_, Some(user)) => format!("user:{}", user.id()),
        _ => {
            return transport_error(
                StatusCode::UNAUTHORIZED,
                "MCP requires gateway authentication",
            );
        }
    };
    let permit = match state.mcp_inflight.acquire(owner) {
        Ok(permit) => permit,
        Err(response) => return response,
    };
    let request_id = message.get("id").cloned().unwrap_or(Value::Null);
    let fail = |status, message| rpc_error(status, &request_id, -32603, message);
    let client =
        match ProviderHttpClient::streaming_no_redirect(ProviderEndpointPolicy::public_only()) {
            Ok(client) => client,
            Err(_) => return fail(StatusCode::SERVICE_UNAVAILABLE, "MCP transport unavailable"),
        };
    #[cfg(test)]
    let client = req
        .app_data::<web::Data<ProviderHttpClient>>()
        .map(|client| client.get_ref().clone())
        .unwrap_or(client);
    let mut outgoing = match client.request(reqwest::Method::POST, &server.url) {
        Ok(request) => request,
        Err(_) => return fail(StatusCode::BAD_GATEWAY, "MCP upstream endpoint rejected"),
    };
    for (name, value) in &server.static_headers {
        outgoing = outgoing.header(name, value);
    }
    if let Some(auth) = &server.auth
        && let Some(value) = auth.get_header_value()
    {
        outgoing = outgoing.header(auth.get_header_name(), value);
    }
    for (name, value) in req.headers() {
        let name = name.as_str();
        if config.auth.enable_api_key && name.eq_ignore_ascii_case(&config.auth.api_key_header) {
            continue;
        }
        if matches!(
            name,
            "accept" | "content-type" | "mcp-protocol-version" | "mcp-method" | "mcp-name"
        ) || name.starts_with("mcp-param-")
        {
            // Tool-specific parameter schemas belong to the upstream. Unknown
            // mirrored parameter headers must pass through unchanged.
            outgoing = outgoing.header(name, value.as_bytes());
        }
    }
    let outgoing = outgoing
        .header("accept-encoding", "identity")
        .body(message.to_string());
    let mut upstream =
        match tokio::time::timeout(Duration::from_millis(server.timeout_ms), outgoing.send()).await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return fail(StatusCode::BAD_GATEWAY, "MCP upstream connection failed"),
            Err(_) => {
                return fail(
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
    let event_stream = upstream
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/event-stream"))
        });
    if event_stream {
        response.insert_header(("x-accel-buffering", "no"));
    }
    let idle_timeout_secs = config.server.stream_idle_timeout;
    let response_timeout = Duration::from_millis(server.timeout_ms);
    // Own the upstream body: dropping the downstream stream closes the request.
    // SSE is long-lived; finite bodies instead share one absolute body deadline.
    let deadline = tokio::time::Instant::now() + response_timeout;
    let stream = async_stream::try_stream! {
        let _permit = permit;
        loop {
            let next = if event_stream && idle_timeout_secs == 0 {
                upstream.chunk().await
            } else {
                let deadline = if event_stream { tokio::time::Instant::now() + Duration::from_secs(idle_timeout_secs) } else { deadline };
                tokio::time::timeout_at(deadline, upstream.chunk()).await
                    .map_err(|_| actix_web::error::ErrorGatewayTimeout("MCP response body timed out"))?
            };
            let Some(chunk) = next.map_err(|_| actix_web::error::ErrorBadGateway("MCP upstream stream interrupted"))? else { break; };
            yield chunk;
        }
    };
    response.streaming::<_, actix_web::Error>(stream)
}
#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
