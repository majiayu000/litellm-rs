use super::*;
use crate::core::{
    mcp::config::{AuthConfig, McpServerConfig},
    models::user::types::User,
    net::ProviderEndpointAccess,
};
use actix_web::{App, HttpMessage, HttpServer, test};
use std::sync::Arc;

type Calls = Arc<Mutex<Vec<(String, Value, Option<String>)>>>;
const COMPRESSED_REPLY: &[u8] = &[
    31, 139, 8, 0, 0, 0, 0, 0, 2, 19, 171, 86, 202, 42, 206, 207, 43, 42, 72, 86, 178, 82, 50, 210,
    51, 80, 210, 81, 202, 76, 81, 178, 50, 212, 81, 42, 74, 45, 46, 205, 41, 81, 178, 170, 86, 202,
    207, 86, 178, 42, 41, 42, 77, 173, 173, 5, 0, 230, 64, 26, 162, 45, 0, 0, 0,
];
async fn upstream(req: HttpRequest, body: web::Bytes, calls: web::Data<Calls>) -> HttpResponse {
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Bearer upstream-only-test"
    );
    let value = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
    let session = req
        .headers()
        .get("mcp-session-id")
        .map(|v| v.to_str().unwrap().to_owned());
    calls
        .lock()
        .unwrap()
        .push((req.method().to_string(), value.clone(), session.clone()));
    assert_ne!(
        req.headers()
            .get("mcp-protocol-version")
            .and_then(|v| v.to_str().ok()),
        Some("gateway-only-sentinel")
    );
    if value.pointer("/params/test_initialize_error") == Some(&Value::Bool(true)) {
        return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32602,"message":"unsupported version"}}));
    }
    if value.pointer("/params/test_initialize_sse") == Some(&Value::Bool(true)) {
        let result = json!({"jsonrpc":"2.0","id":value["id"],"result":{"protocolVersion":"2025-11-25","capabilities":{},"serverInfo":{"name":"test","version":"1"}}});
        return HttpResponse::Ok()
            .insert_header(("content-type", "Text/Event-Stream"))
            .insert_header(("mcp-session-id", "upstream-session"))
            .body(format!("data: {result}\n\n"));
    }
    if value.get("method").and_then(Value::as_str) == Some("compressed") {
        return HttpResponse::Ok()
            .insert_header(("content-type", "application/json"))
            .insert_header(("content-encoding", "gzip"))
            .body(COMPRESSED_REPLY);
    }
    if req.method() == Method::GET {
        assert_eq!(session.as_deref(), Some("upstream-session"));
        assert_eq!(req.headers().get("last-event-id").unwrap(), "event-1");
        return HttpResponse::Ok().insert_header(("content-type","text/event-stream")).body("id: event-2\ndata: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\"}\n\n");
    }
    if req.method() == Method::DELETE {
        return HttpResponse::NoContent().finish();
    }
    let method = value["method"].as_str().unwrap_or("");
    match method {
        "initialize" => HttpResponse::Ok().insert_header(("mcp-session-id","upstream-session")).json(json!({"jsonrpc":"2.0","id":value["id"],"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"test","version":"1"}}})),
        "notifications/initialized" => HttpResponse::Accepted().finish(),
        "limited" => HttpResponse::TooManyRequests().insert_header(("retry-after","7")).json(json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32000,"message":"limit"}})),
        "slow" => {
            struct Closed(Calls);
            impl Drop for Closed {
                fn drop(&mut self) { self.0.lock().unwrap().push(("CLOSED".into(), Value::Null, None)); }
            }
            let calls = calls.get_ref().clone();
            let stream = async_stream::stream! {
                let _closed = Closed(calls);
                yield Ok::<_, std::io::Error>(web::Bytes::from_static(b"data: {}\n\n"));
                loop {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    yield Ok(web::Bytes::from_static(b": heartbeat\n\n"));
                }
            };
            HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).streaming(stream)
        },
        "tools/call" => HttpResponse::Ok().insert_header(("content-type","text/event-stream")).body(format!("data: {}\n\n",json!({"jsonrpc":"2.0","id":value["id"],"result":{"content":[{"type":"text","text":"done"}]}}))),
        _ => HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":value["id"],"result":{"echo":value}})),
    }
}
async fn fixture(auth: bool) -> (web::Data<AppState>, Calls, actix_web::dev::ServerHandle) {
    let calls: Calls = Arc::default();
    let server_calls = calls.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server_calls.clone()))
            .default_service(web::to(upstream))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let mut config = crate::server::valid_test_config();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = auth;
    config.gateway.auth.api_key_header = "x-mcp-key".into();
    config.gateway.auth.allow_anonymous = !auth;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.server.cors.enabled = true;
    config.gateway.server.cors.allowed_origins = vec!["https://mcp.example.test".into()];
    let server = crate::server::http::HttpServer::new(&config).await.unwrap();
    let mut state = server.state().clone();
    config.gateway.mcp_servers.insert(
        "docs".into(),
        McpServerConfig::new("docs", &url).with_auth(AuthConfig::bearer("upstream-only-test")),
    );
    // Only the mock fixture bypasses public-network configuration validation.
    let mut revision = state.pin_runtime().as_ref().clone();
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    state.mcp_sessions = Arc::new(Sessions {
        entries: Mutex::default(),
        reaper_started: AtomicBool::new(false),
        client: Some(
            ProviderHttpClient::streaming_no_redirect(
                ProviderEndpointPolicy::for_base_url(ProviderEndpointAccess::PrivateNetwork, &url)
                    .unwrap(),
            )
            .unwrap(),
        ),
    });
    (web::Data::new(state), calls, handle)
}
fn user() -> User {
    User::new(
        "mcp-user".into(),
        "mcp@example.test".into(),
        "unused".into(),
    )
}
fn request(method: &str, session: Option<&str>, user: &User) -> actix_http::Request {
    let mut req = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .insert_header(("mcp-protocol-version", "2025-11-25"));
    if let Some(session) = session {
        req = req.insert_header(("mcp-session-id", session));
    }
    let req = req.set_json(json!({"jsonrpc":"2.0","id":1,"method":method,"params":{"cursor":"cursor-original","uri":"test://resource","name":"original","arguments":{"x":1}}})).to_request();
    req.extensions_mut().insert(user.clone());
    req
}
#[actix_web::test]
async fn forwards_tools_resources_prompts_notifications_and_sse() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let user = user();
    let response = test::call_service(&app, request("initialize", None, &user)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert_ne!(token, "upstream-session");
    let result: Value = test::read_body_json(response).await;
    assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
    for method in [
        "tools/list",
        "resources/list",
        "resources/read",
        "resources/templates/list",
        "prompts/list",
        "prompts/get",
    ] {
        let response = test::call_service(&app, request(method, Some(&token), &user)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["result"]["echo"]["method"], method);
        assert_eq!(
            value["result"]["echo"]["params"]["cursor"],
            "cursor-original"
        );
    }
    let notification = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .insert_header(("mcp-session-id", token.clone()))
        .set_json(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .to_request();
    notification.extensions_mut().insert(user.clone());
    let response = test::call_service(&app, notification).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(test::read_body(response).await.is_empty());
    let response = test::call_service(&app, request("tools/call", Some(&token), &user)).await;
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let body = test::read_body(response).await;
    assert!(std::str::from_utf8(&body).unwrap().contains("done"));
    assert_eq!(calls.lock().unwrap().len(), 9);
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .skip(1)
            .all(|(_, _, s)| s.as_deref() == Some("upstream-session"))
    );
    server.stop(true).await;
}
#[actix_web::test]
async fn isolates_sessions_and_forwards_error_resume_and_delete() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let response = test::call_service(&app, request("initialize", None, &owner)).await;
    let token = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    drop(test::read_body(response).await);
    let other = user();
    let response = test::call_service(&app, request("tools/list", Some(&token), &other)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(calls.lock().unwrap().len(), 1);
    let response = test::call_service(&app, request("limited", Some(&token), &owner)).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("retry-after").unwrap(), "7");
    assert_eq!(
        test::read_body_json::<Value, _>(response).await["error"]["message"],
        "limit"
    );
    let req = test::TestRequest::get()
        .uri("/docs/mcp")
        .insert_header(("accept", "text/event-stream"))
        .insert_header(("mcp-session-id", token.clone()))
        .insert_header(("last-event-id", "event-1"))
        .to_request();
    req.extensions_mut().insert(owner.clone());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        std::str::from_utf8(&test::read_body(response).await)
            .unwrap()
            .contains("event-2")
    );
    let req = test::TestRequest::delete()
        .uri("/docs/mcp")
        .insert_header(("accept", "text/event-stream"))
        .insert_header(("mcp-session-id", token.clone()))
        .to_request();
    req.extensions_mut().insert(owner.clone());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(test::read_body(response).await.is_empty());
    let response = test::call_service(&app, request("tools/list", Some(&token), &owner)).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(state.mcp_sessions.entries.lock().unwrap().is_empty());
    server.stop(true).await;
}
#[actix_web::test]
async fn rejects_anonymous_origin_body_and_missing_session_before_upstream() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 256)),
    )
    .await;
    let user = user();
    let req = request("initialize", None, &user);
    req.extensions_mut().clear();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let mut req = request("initialize", None, &user);
    req.headers_mut().insert(
        actix_web::http::header::ORIGIN,
        "https://untrusted.test".parse().unwrap(),
    );
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        test::call_service(&app, request("tools/list", None, &user))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let req = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .set_json(json!([{"jsonrpc":"2.0","method":"initialize"}]))
        .to_request();
    req.extensions_mut().insert(user.clone());
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::BAD_REQUEST
    );
    let req = test::TestRequest::post()
        .uri("/docs/mcp")
        .set_payload("x".repeat(1024))
        .to_request();
    req.extensions_mut().insert(user);
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(calls.lock().unwrap().is_empty());
    let app = test::init_service(crate::server::http::HttpServer::create_app(state)).await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/docs/mcp")
            .set_json(json!({"jsonrpc":"2.0","method":"initialize","id":1}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    server.stop(true).await;
}
#[actix_web::test]
async fn rejects_unsupported_gateway_configuration() {
    let config = McpServerConfig::new("docs", "https://1.1.1.1/mcp");
    assert!(config.validate_http_gateway("docs").is_ok());
    assert!(config.validate_http_gateway("different").is_err());
    let mut other = config.clone();
    other.transport = crate::core::mcp::transport::Transport::Sse;
    assert!(other.validate_http_gateway("docs").is_err());
    let mut other = config.clone();
    other.forward_headers.push("authorization".into());
    assert!(other.validate_http_gateway("docs").is_err());
    for url in [
        "htps://user:sentinel@example.test/?key=sentinel",
        "https://[?key=sentinel",
    ] {
        let malformed = McpServerConfig::new("docs", url);
        let error = malformed.validate_http_gateway("docs").unwrap_err();
        assert!(!error.contains("sentinel"));
    }
    let collision = config
        .clone()
        .with_auth(AuthConfig::bearer("test"))
        .with_header("aUtHoRiZaTiOn", "other");
    assert!(
        collision
            .validate_http_gateway("docs")
            .unwrap_err()
            .contains("one MCP")
    );
    let other = config.with_header("mcp-session-id", "shared-session");
    assert!(other.validate_http_gateway("docs").is_err());
}

#[actix_web::test]
async fn dropping_gateway_stream_closes_upstream_body() {
    use actix_web::body::MessageBody;
    let (state, calls, server) = fixture(false).await;
    let owner = user();
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .insert_header(("content-type", "application/json"))
        .to_http_request();
    req.extensions_mut().insert(owner.clone());
    let response = proxy(
        req,
        web::Bytes::from_static(br#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#),
        state.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    drop(
        actix_web::body::to_bytes(response.into_body())
            .await
            .unwrap(),
    );
    let req = test::TestRequest::post()
        .uri("/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .insert_header(("content-type", "application/json"))
        .insert_header(("mcp-session-id", token))
        .to_http_request();
    req.extensions_mut().insert(owner);
    let response = proxy(
        req,
        web::Bytes::from_static(br#"{"jsonrpc":"2.0","id":2,"method":"slow"}"#),
        state,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = Box::pin(response.into_body());
    let chunk = futures::future::poll_fn(|cx| body.as_mut().poll_next(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(chunk, "data: {}\n\n");
    drop(body);
    tokio::time::timeout(Duration::from_secs(2), async {
        while !calls
            .lock()
            .unwrap()
            .iter()
            .any(|(method, _, _)| method == "CLOSED")
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("upstream body should close after client disconnect");
    server.stop(true).await;
}

#[actix_web::test]
async fn expires_sessions_and_invalidates_changed_accounts() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for expired in [true, false] {
        let response = test::call_service(&app, request("initialize", None, &owner)).await;
        let token = response
            .headers()
            .get("mcp-session-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        drop(test::read_body(response).await);
        if expired {
            state
                .mcp_sessions
                .entries
                .lock()
                .unwrap()
                .get_mut(&token)
                .unwrap()
                .expires = Instant::now();
        } else {
            let mut revision = state.pin_runtime().as_ref().clone();
            let mut config = revision.config.as_ref().clone();
            config.gateway.mcp_servers.get_mut("docs").unwrap().auth =
                Some(AuthConfig::bearer("changed-upstream-test"));
            revision.config = Arc::new(config);
            state.runtime.store(revision);
        }
        let response = test::call_service(&app, request("tools/list", Some(&token), &owner)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let runtime = state.pin_runtime();
    reap_sessions(&state.mcp_sessions, &runtime.config.gateway.mcp_servers).await;
    assert!(state.mcp_sessions.entries.lock().unwrap().is_empty());
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 4);
    assert_eq!(
        calls
            .iter()
            .filter(|(method, _, _)| method == "DELETE")
            .count(),
        2
    );
    server.stop(true).await;
}

#[actix_web::test]
async fn enforces_key_operation_and_endpoint_restrictions() {
    use crate::core::models::ApiKey;
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let user = user();
    let mut key = ApiKey {
        metadata: Default::default(),
        name: "restricted".into(),
        key_hash: "unused".into(),
        key_prefix: "test".into(),
        user_id: Some(user.id()),
        team_id: None,
        permissions: vec!["chat".into()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: Default::default(),
    };
    let req = request("initialize", None, &user);
    req.extensions_mut().insert(key.clone());
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    key.permissions = vec!["mcp.docs".into()];
    key.metadata.extra.insert(
        "__core_keys".into(),
        json!({"permissions":{"allowed_endpoints":["chat"]}}),
    );
    let req = request("initialize", None, &user);
    req.extensions_mut().insert(key.clone());
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    assert!(calls.lock().unwrap().is_empty());
    key.metadata.extra.clear();
    let req = request("initialize", None, &user);
    req.extensions_mut().insert(key.clone());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let token = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    drop(test::read_body(response).await);
    key.metadata.id = uuid::Uuid::new_v4();
    let req = request("tools/list", Some(&token), &user);
    req.extensions_mut().insert(key);
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.stop(true).await;
}

#[actix_web::test]
async fn exported_mcp_configuration_redacts_credentials() {
    let mut config = crate::server::valid_test_config();
    let server = McpServerConfig::new("docs", "https://1.1.1.1/mcp?secret=mcp-query-sentinel")
        .with_auth(AuthConfig::bearer("mcp-token-sentinel"))
        .with_header("x-api-key", "mcp-header-sentinel");
    config.gateway.mcp_servers.insert("docs".into(), server);
    for output in [
        config.to_json().unwrap(),
        config.to_yaml().unwrap(),
        format!("{:?}", config),
    ] {
        for secret in [
            "mcp-query-sentinel",
            "mcp-token-sentinel",
            "mcp-header-sentinel",
        ] {
            assert!(!output.contains(secret));
        }
    }
    assert_eq!(
        config.gateway.mcp_servers["docs"]
            .auth
            .as_ref()
            .unwrap()
            .value
            .as_deref(),
        Some("mcp-token-sentinel")
    );
}

#[actix_web::test]
async fn production_middleware_accepts_key_with_named_endpoint_permission() {
    use crate::core::models::{ApiKey, user::types::UserStatus};
    use crate::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    let (state, calls, server) = fixture(true).await;
    let mut owner = user();
    owner.status = UserStatus::Active;
    let owner = state.storage.db().create_user(&owner).await.unwrap();
    let raw = "gw-mcp-integration-test-12345678901234567890";
    let mut key = ApiKey {
        metadata: Default::default(),
        name: "mcp-http".into(),
        key_hash: hash_api_key(raw, None),
        key_prefix: extract_api_key_prefix(raw),
        user_id: Some(owner.id()),
        team_id: None,
        permissions: vec!["mcp.docs".into()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: Default::default(),
    };
    key.metadata.set_extra(
        "__core_keys",
        json!({"permissions":{"allowed_endpoints":["/docs/mcp"]}}),
    );
    state.storage.db().create_api_key(&key).await.unwrap();
    let app = test::init_service(crate::server::http::HttpServer::create_app(state)).await;
    let preflight = test::TestRequest::default()
        .method(Method::OPTIONS)
        .uri("/docs/mcp")
        .insert_header(("origin", "https://mcp.example.test"))
        .insert_header(("access-control-request-method", "POST"))
        .insert_header((
            "access-control-request-headers",
            "x-mcp-key,content-type,mcp-session-id,mcp-protocol-version,last-event-id",
        ))
        .to_request();
    let preflight = test::call_service(&app, preflight).await;
    let status = preflight.status();
    let body = test::read_body(preflight).await;
    assert!(status.is_success(), "preflight {status}: {body:?}");
    let req = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("origin", "https://mcp.example.test"))
        .insert_header(("x-mcp-key", raw))
        .insert_header(("accept", "application/json, text/event-stream"))
        .set_json(json!({"jsonrpc":"2.0","method":"initialize","id":1}))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("mcp-session-id"));
    let exposed = response
        .headers()
        .get("access-control-expose-headers")
        .unwrap()
        .to_str()
        .unwrap();
    for name in ["mcp-session-id", "mcp-protocol-version", "retry-after"] {
        assert!(exposed.contains(name));
    }
    drop(test::read_body(response).await);
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.stop(true).await;
}

#[actix_web::test]
async fn capacity_is_reserved_before_upstream_and_cancellation_releases_it() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    {
        let mut entries = state.mcp_sessions.entries.lock().unwrap();
        for index in 0..4096 {
            entries.insert(
                index.to_string(),
                Session {
                    owner: "other".into(),
                    binding: vec![],
                    upstream: None,
                    expires: Instant::now() + Duration::from_secs(60),
                    pending: false,
                    server: Arc::new(McpServerConfig::new("docs", "https://1.1.1.1/mcp")),
                },
            );
        }
    }
    let response = test::call_service(&app, request("initialize", None, &user())).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    assert!(calls.lock().unwrap().is_empty());
    state.mcp_sessions.entries.lock().unwrap().clear();
    // Cancellation drops the guard even without reaching an HTTP error handler.
    let sessions = state.mcp_sessions.clone();
    let future = async {
        sessions.entries.lock().unwrap().insert(
            "pending".into(),
            Session {
                owner: "other".into(),
                binding: vec![],
                upstream: None,
                expires: Instant::now(),
                pending: true,
                server: Arc::new(McpServerConfig::new("docs", "https://1.1.1.1/mcp")),
            },
        );
        let _reservation = PendingSession {
            sessions: sessions.clone(),
            token: "pending".into(),
            committed: false,
        };
        std::future::pending::<()>().await;
    };
    assert!(
        tokio::time::timeout(Duration::from_millis(1), future)
            .await
            .is_err()
    );
    assert!(sessions.entries.lock().unwrap().is_empty());
    server.stop(true).await;
}

#[actix_web::test]
async fn accept_handles_repeated_fields_case_and_quality() {
    let (state, calls, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for (values, expected) in [
        (
            vec!["Application/JSON", "Text/Event-Stream; q=0.5"],
            StatusCode::OK,
        ),
        (
            vec!["application/json, text/event-stream;q=0"],
            StatusCode::NOT_ACCEPTABLE,
        ),
        (
            vec!["application/jsonp, text/event-stream"],
            StatusCode::NOT_ACCEPTABLE,
        ),
    ] {
        let mut builder = test::TestRequest::post().uri("/docs/mcp");
        for value in values {
            builder = builder.append_header(("accept", value));
        }
        let req = builder
            .set_json(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}))
            .to_request();
        req.extensions_mut().insert(owner.clone());
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), expected);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        drop(test::read_body(response).await);
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.stop(true).await;
}

#[actix_web::test]
async fn equivalent_config_reload_preserves_session() {
    let (state, _, server) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let mut token = None;
    for reverse in [false, true] {
        let mut revision = state.pin_runtime().as_ref().clone();
        let mut config = revision.config.as_ref().clone();
        let mut headers = HashMap::new();
        let indexes: Vec<_> = if reverse {
            (0..32).rev().collect()
        } else {
            (0..32).collect()
        };
        for index in indexes {
            headers.insert(format!("x-test-{index}"), index.to_string());
        }
        config
            .gateway
            .mcp_servers
            .get_mut("docs")
            .unwrap()
            .static_headers = headers;
        if reverse {
            let server = config.gateway.mcp_servers.get_mut("docs").unwrap();
            server.description = Some("changed description".into());
            server.timeout_ms += 100;
        }
        revision.config = Arc::new(config);
        state.runtime.store(revision);
        let response = test::call_service(
            &app,
            request(
                if reverse { "tools/list" } else { "initialize" },
                token.as_deref(),
                &owner,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        if !reverse {
            token = Some(
                response
                    .headers()
                    .get("mcp-session-id")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        drop(test::read_body(response).await);
    }
    server.stop(true).await;
}

#[actix_web::test]
async fn initialize_rpc_errors_do_not_commit_sessions_and_sse_results_do() {
    let (state, _, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for sse in [false, true] {
        let req = test::TestRequest::post().uri("/docs/mcp")
            .insert_header(("accept", "application/json, text/event-stream"))
            .set_json(json!({"jsonrpc":"2.0","id":7,"method":"initialize","params":{"test_initialize_error":!sse,"test_initialize_sse":sse}})).to_request();
        req.extensions_mut().insert(owner.clone());
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().contains_key("mcp-session-id"), sse);
        let bytes = test::read_body(response).await;
        assert_eq!(
            state.mcp_sessions.entries.lock().unwrap().len(),
            usize::from(sse)
        );
        if sse {
            assert!(std::str::from_utf8(&bytes).unwrap().starts_with("data: "));
        } else {
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
                -32602
            );
        }
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn one_owner_cannot_fill_global_session_capacity() {
    let (state, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let response = test::call_service(&app, request("initialize", None, &owner)).await;
    drop(test::read_body(response).await);
    {
        let mut entries = state.mcp_sessions.entries.lock().unwrap();
        let mut session = entries.values().next().unwrap().clone();
        session.upstream = None;
        for index in 1..128 {
            entries.insert(format!("fixture-{index}"), session.clone());
        }
    }
    assert_eq!(
        test::call_service(&app, request("initialize", None, &owner))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    let response = test::call_service(&app, request("initialize", None, &user())).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(test::read_body(response).await);
    handle.stop(false).await;
}

#[actix_web::test]
async fn configured_gateway_credential_header_is_not_forwarded() {
    let (state, _, handle) = fixture(false).await;
    let mut revision = state.pin_runtime().as_ref().clone();
    let mut config = revision.config.as_ref().clone();
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.api_key_header = "MCP-Protocol-Version".into();
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let mut req = request("initialize", None, &user());
    req.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("mcp-protocol-version"),
        "gateway-only-sentinel".parse().unwrap(),
    );
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(test::read_body(response).await);
    handle.stop(false).await;
}

#[actix_web::test]
async fn unchanged_encoded_bodies_preserve_encoding_metadata() {
    let (state, _, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let response = test::call_service(&app, request("initialize", None, &owner)).await;
    let token = response
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    drop(test::read_body(response).await);
    let response = test::call_service(&app, request("compressed", Some(&token), &owner)).await;
    assert_eq!(response.headers().get("content-encoding").unwrap(), "gzip");
    assert_eq!(test::read_body(response).await.as_ref(), COMPRESSED_REPLY);
    handle.stop(false).await;
}
