use super::*;
use crate::core::{
    mcp::config::{AuthConfig, McpAuthType, McpServerConfig},
    models::user::types::User,
    net::ProviderEndpointAccess,
};
use actix_web::{App, HttpMessage, HttpServer, http::Method, test};
use std::sync::{Arc, Mutex};
type Calls = Arc<Mutex<Vec<(Value, Vec<(String, String)>)>>>;
async fn upstream(req: HttpRequest, body: web::Bytes, calls: web::Data<Calls>) -> HttpResponse {
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Bearer upstream-only-test"
    );
    assert!(!req.headers().contains_key("x-mcp-key"));
    assert!(!req.headers().contains_key("mcp-session-id"));
    assert!(!req.headers().contains_key("last-event-id"));
    let value: Value = serde_json::from_slice(&body).unwrap();
    let headers = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap().to_owned()))
        .collect();
    calls.lock().unwrap().push((value.clone(), headers));
    let result = json!({"jsonrpc":"2.0", "id":value["id"], "result":{"echo":value["params"],"ttlMs":1000,"cacheScope":"private"}});
    match value["method"].as_str().unwrap() {
        "limited" => HttpResponse::TooManyRequests().insert_header(("retry-after","7")).json(json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32603,"message":"limit"}})),
        "redirect" => HttpResponse::Found().insert_header(("location","http://127.0.0.1:1/secret")).body("redirect not followed"),
        "unsupported" => HttpResponse::NotFound().json(json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32601,"message":"unknown"}})),
        "notification/test" => HttpResponse::Accepted().finish(),
        "input_required" => HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":value["id"],"result":{"resultType":"input_required","inputRequests":[{"id":"sample-1","method":"sampling/createMessage","params":{"messages":[]}}]}})),
        "subscriptions/listen" => HttpResponse::Ok().insert_header(("content-type","text/event-stream")).body("data: {\"jsonrpc\":\"2.0\",\"method\":\"notifications/tools/list_changed\",\"params\":{\"_meta\":{\"io.modelcontextprotocol/subscriptionId\":\"subscription-1\"}}}\n\n"),
        "sse" => HttpResponse::Ok().insert_header(("content-type","text/event-stream")).body(format!("data: {{\"jsonrpc\":\"2.0\",\"method\":\"notifications/progress\"}}\n\ndata: {result}\n\n")),
        "stalled-json" => HttpResponse::Ok().insert_header(("content-type","application/json")).streaming(async_stream::stream! {
            yield Ok::<_, std::io::Error>(web::Bytes::from_static(b"{"));
            tokio::time::sleep(Duration::from_secs(5)).await;
            yield Ok(web::Bytes::from_static(b"}"));
        }),
        "slow" | "idle" => {
            struct Closed(Calls);
            impl Drop for Closed { fn drop(&mut self) { self.0.lock().unwrap().push((json!({"closed":true}),vec![])); } }
            let guard = Closed(calls.get_ref().clone());
            let delay = if value["method"] == "idle" { Duration::from_millis(1500) } else { Duration::from_millis(20) };
            HttpResponse::Ok().insert_header(("content-type","text/event-stream")).streaming(async_stream::stream! {
                let _guard = guard;
                yield Ok::<_, std::io::Error>(web::Bytes::from_static(b"data: {}\n\n"));
                loop {
                    tokio::time::sleep(delay).await;
                    yield Ok(web::Bytes::from_static(b": heartbeat\n\n"));
                }
            })
        }
        _ => HttpResponse::Ok().insert_header(("mcp-session-id","must-not-be-echoed")).json(result),
    }
}
async fn fixture(
    auth: bool,
) -> (
    web::Data<AppState>,
    web::Data<ProviderHttpClient>,
    Calls,
    actix_web::dev::ServerHandle,
) {
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
    config
        .gateway
        .server
        .cors
        .allowed_headers
        .push("mcp-param-region".into());
    config.gateway.server.cors.allowed_origins = vec!["https://mcp.example.test".into()];
    let server = crate::server::http::HttpServer::new(&config).await.unwrap();
    let state = server.state().clone();
    config.gateway.mcp_servers.insert(
        "docs".into(),
        McpServerConfig::new("docs", &url).with_auth(AuthConfig::bearer("upstream-only-test")),
    );
    // Only the mock fixture bypasses public-network configuration validation.
    let mut revision = state.pin_runtime().as_ref().clone();
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let client = ProviderHttpClient::streaming_no_redirect(
        ProviderEndpointPolicy::for_base_url(ProviderEndpointAccess::PrivateNetwork, &url).unwrap(),
    )
    .unwrap();
    (web::Data::new(state), web::Data::new(client), calls, handle)
}
fn user() -> User {
    User::new(
        "mcp-user".into(),
        "mcp@example.test".into(),
        "unused".into(),
    )
}

fn message(method: &str) -> Value {
    json!({"jsonrpc":"2.0","id":1,"method":method,"params":{"name":"original","uri":"test://resource","cursor":"cursor-original","arguments":{"x":1},"_meta":{"io.modelcontextprotocol/protocolVersion":PROTOCOL_VERSION,"io.modelcontextprotocol/clientCapabilities":{}}}})
}
fn builder(method: &str) -> test::TestRequest {
    test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .insert_header(("mcp-protocol-version", PROTOCOL_VERSION))
        .insert_header(("mcp-method", method))
        .insert_header((
            "mcp-name",
            if method == "resources/read" {
                "test://resource"
            } else {
                "original"
            },
        ))
        .set_json(message(method))
}
fn request(method: &str, owner: &User) -> actix_http::Request {
    let req = builder(method).to_request();
    req.extensions_mut().insert(owner.clone());
    req
}
#[actix_web::test]
async fn rejects_unsupported_gateway_configuration() {
    let config = McpServerConfig::new("docs", "https://1.1.1.1/mcp");
    let insecure = McpServerConfig::new("docs", "http://example.com/mcp")
        .with_auth(AuthConfig::bearer("test"));
    assert!(
        insecure
            .validate_http_gateway("docs")
            .unwrap_err()
            .contains("HTTPS")
    );
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
    let query = McpServerConfig::new("docs", "http://example.com/mcp?api_key=test");
    assert!(
        query
            .validate_http_gateway("docs")
            .unwrap_err()
            .contains("HTTPS")
    );
    let secure_query = McpServerConfig::new("docs", "https://example.com/mcp?api_key=test");
    assert!(secure_query.validate_http_gateway("docs").is_ok());
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
async fn exported_mcp_configuration_redacts_credentials() {
    let mut config = crate::server::valid_test_config();
    let server = McpServerConfig::new("docs", "https://1.1.1.1/mcp?secret=mcp-query-sentinel")
        .with_auth(AuthConfig::bearer("mcp-token-sentinel"))
        .with_header("x-api-key", "mcp-header-sentinel");
    config.gateway.mcp_servers.insert("docs".into(), server);
    config.gateway.mcp_servers.insert(
        "malformed".into(),
        McpServerConfig::new("malformed", "https://[?token=mcp-malformed-sentinel"),
    );
    for output in [
        config.to_json().unwrap(),
        config.to_yaml().unwrap(),
        format!("{:?}", config),
    ] {
        for secret in [
            "mcp-malformed-sentinel",
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
    let (state, client, calls, server) = fixture(true).await;
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
    let app =
        test::init_service(crate::server::http::HttpServer::create_app(state).app_data(client))
            .await;
    let preflight = test::TestRequest::default()
        .method(Method::OPTIONS)
        .uri("/docs/mcp")
        .insert_header(("origin", "https://mcp.example.test"))
        .insert_header(("access-control-request-method", "POST"))
        .insert_header((
            "access-control-request-headers",
            "x-mcp-key,content-type,mcp-method,mcp-name,mcp-protocol-version,mcp-param-region",
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
        .insert_header(("mcp-method", "tools/list"))
        .insert_header(("mcp-protocol-version", PROTOCOL_VERSION))
        .set_json(message("tools/list"))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("mcp-session-id"));
    let exposed = response
        .headers()
        .get("access-control-expose-headers")
        .unwrap()
        .to_str()
        .unwrap();
    for name in ["mcp-protocol-version", "retry-after"] {
        assert!(exposed.contains(name));
    }
    drop(test::read_body(response).await);
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.stop(true).await;
}

#[actix_web::test]
async fn stateless_first_requests_preserve_native_methods_and_ignore_old_sessions() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for method in [
        "server/discover",
        "tools/list",
        "tools/call",
        "resources/list",
        "resources/templates/list",
        "resources/read",
        "prompts/list",
        "prompts/get",
    ] {
        let req = builder(method)
            .insert_header(("mcp-session-id", "obsolete"))
            .insert_header(("last-event-id", "obsolete"))
            .insert_header(("mcp-param-region", "us-west1"))
            .to_request();
        req.extensions_mut().insert(user());
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK, "{method}");
        assert!(!response.headers().contains_key("mcp-session-id"));
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["result"]["echo"], message(method)["params"]);
        assert_eq!(body["result"]["cacheScope"], "private");
    }
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 8);
    assert!(
        recorded
            .iter()
            .all(|(_, headers)| headers.contains(&("mcp-param-region".into(), "us-west1".into())))
    );
    handle.stop(false).await;
}

#[actix_web::test]
async fn protocol_and_mirrored_headers_are_validated_before_upstream() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for (header, value) in [
        ("mcp-method", "tools/list"),
        ("mcp-protocol-version", "2025-11-25"),
        ("mcp-name", "wrong"),
        ("mcp-name", "=?base64?invalid!?="),
    ] {
        let req = builder("tools/call")
            .insert_header((header, value))
            .to_request();
        req.extensions_mut().insert(user());
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], -32020);
        assert_eq!(body["id"], 1);
    }
    for header in ["mcp-method", "mcp-protocol-version", "mcp-name"] {
        let req = builder("tools/call")
            .append_header((header, "duplicate"))
            .to_request();
        req.extensions_mut().insert(user());
        let body: Value = test::call_and_read_body_json(&app, req).await;
        assert_eq!(body["error"]["code"], -32020);
    }
    let mut old = message("tools/list");
    old["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"] = json!("2025-11-25");
    let req = builder("tools/list")
        .insert_header(("mcp-protocol-version", "2025-11-25"))
        .set_json(old)
        .to_request();
    req.extensions_mut().insert(user());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["code"], -32022);
    assert_eq!(
        body["error"]["data"]["supported"],
        json!([PROTOCOL_VERSION])
    );
    for field in [
        "io.modelcontextprotocol/protocolVersion",
        "io.modelcontextprotocol/clientCapabilities",
    ] {
        let mut incomplete = message("tools/list");
        incomplete["params"]["_meta"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let req = builder("tools/list").set_json(incomplete).to_request();
        req.extensions_mut().insert(user());
        let body: Value = test::call_and_read_body_json(&app, req).await;
        assert_eq!(body["error"]["code"], -32602);
    }
    let req = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .set_json(message("tools/list"))
        .to_request();
    req.extensions_mut().insert(user());
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["error"]["code"], -32020);
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn encoded_names_and_large_numbers_survive_without_precision_loss() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for name in ["中文工具", " padded ", "=?base64?literal?=", "line\nbreak"] {
        let mut value = message("tools/call");
        value["id"] = serde_json::from_str("184467440737095516170000").unwrap();
        value["params"]["name"] = json!(name);
        value["params"]["arguments"] = serde_json::from_str(
            r#"{"large":184467440737095516170000,"decimal":0.1234567890123456789012345}"#,
        )
        .unwrap();
        let req = builder("tools/call")
            .insert_header(("mcp-name", format!("=?base64?{}?=", STANDARD.encode(name))))
            .set_json(&value)
            .to_request();
        req.extensions_mut().insert(user());
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["result"]["echo"], value["params"]);
    }
    assert_eq!(calls.lock().unwrap().len(), 4);
    handle.stop(false).await;
}

#[actix_web::test]
async fn forwards_mrtr_retries_subscriptions_and_request_scoped_sse() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let body: Value = test::call_and_read_body_json(&app, request("input_required", &user())).await;
    assert_eq!(body["result"]["resultType"], "input_required");
    assert_eq!(
        body["result"]["inputRequests"][0]["method"],
        "sampling/createMessage"
    );
    let mut retry = message("tools/call");
    retry["params"]["inputResponses"] = json!([{"id":"sample-1","result":{"model":"test","role":"assistant","content":{"type":"text","text":"answer"}}}]);
    let req = builder("tools/call").set_json(&retry).to_request();
    req.extensions_mut().insert(user());
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["result"]["echo"], retry["params"]);
    for method in ["subscriptions/listen", "sse"] {
        let response = test::call_service(&app, request(method, &user())).await;
        assert_eq!(response.headers().get("x-accel-buffering").unwrap(), "no");
        let body = test::read_body(response).await;
        let text = std::str::from_utf8(&body).unwrap();
        assert!(text.contains(if method == "sse" {
            "notifications/progress"
        } else {
            "io.modelcontextprotocol/subscriptionId"
        }));
    }
    assert_eq!(calls.lock().unwrap().len(), 4);
    handle.stop(false).await;
}

#[actix_web::test]
async fn upstream_errors_notifications_and_redirect_status_are_preserved() {
    let (state, client, _, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let response = test::call_service(&app, request("limited", &user())).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("retry-after").unwrap(), "7");
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["message"], "limit");
    let response = test::call_service(&app, request("unsupported", &user())).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["code"], -32601);
    let response = test::call_service(&app, request("redirect", &user())).await;
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(test::read_body(response).await, "redirect not followed");
    let req = test::TestRequest::post()
        .uri("/docs/mcp")
        .insert_header(("accept", "application/json, text/event-stream"))
        .set_json(json!({"jsonrpc":"2.0","method":"notification/test"}))
        .to_request();
    req.extensions_mut().insert(user());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(test::read_body(response).await.is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn rejects_anonymous_bad_origin_legacy_verbs_and_invalid_envelopes() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let response = test::call_service(&app, builder("tools/list").to_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    for origin in ["null", "https://evil.example"] {
        let req = builder("tools/list")
            .insert_header(("origin", origin))
            .to_request();
        req.extensions_mut().insert(user());
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    for verb in [Method::GET, Method::DELETE] {
        let req = test::TestRequest::default()
            .method(verb)
            .uri("/docs/mcp")
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }
    for value in [
        json!([]),
        json!({"jsonrpc":"2.0","id":1,"result":{}}),
        json!({"jsonrpc":"2.0","id":null,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":1.5,"method":"tools/list"}),
    ] {
        let req = builder("tools/list").set_json(value).to_request();
        req.extensions_mut().insert(user());
        let body: Value = test::call_and_read_body_json(&app, req).await;
        assert_eq!(body["error"]["code"], -32600);
    }
    let req = builder("tools/list").set_payload("{").to_request();
    req.extensions_mut().insert(user());
    let body: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(body["error"]["code"], -32700);
    let req = builder("tools/list")
        .insert_header(("accept", "application/json"))
        .to_request();
    req.extensions_mut().insert(user());
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_ACCEPTABLE
    );
    let req = builder("tools/list")
        .insert_header(("content-type", "text/plain"))
        .to_request();
    req.extensions_mut().insert(user());
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn configured_credentials_cannot_replace_mirrored_transport_headers() {
    let (state, _, _, handle) = fixture(false).await;
    let mut gateway = state.pin_runtime().config.gateway.clone();
    gateway.auth.enable_api_key = true;
    gateway.mcp_servers.get_mut("docs").unwrap().url = "https://example.com/mcp".into();
    for header in [
        "Accept",
        "Mcp-Method",
        "Mcp-Name",
        "MCP-Protocol-Version",
        "mCp-PaRaM-tenant",
        "Mcp-Session-Id",
        "Last-Event-ID",
    ] {
        gateway.auth.api_key_header = header.into();
        assert!(
            gateway
                .validate()
                .unwrap_err()
                .contains("conflicts with MCP")
        );
        let server =
            McpServerConfig::new("docs", "https://example.com/mcp").with_header(header, "test");
        assert!(server.validate_http_gateway("docs").is_err());
        let mut server = McpServerConfig::new("docs", "https://example.com/mcp")
            .with_auth(AuthConfig::api_key("test"));
        server.auth.as_mut().unwrap().header_name = Some(header.into());
        assert!(server.validate_http_gateway("docs").is_err());
    }
    for kind in [
        McpAuthType::ApiKey,
        McpAuthType::BearerToken,
        McpAuthType::Basic,
    ] {
        for value in [None, Some("".into()), Some("  ".into())] {
            let mut server = McpServerConfig::new("docs", "https://example.com/mcp")
                .with_auth(AuthConfig::bearer("test"));
            let auth = server.auth.as_mut().unwrap();
            auth.auth_type = kind;
            auth.value = value;
            assert!(server.validate_http_gateway("docs").is_err());
        }
    }
    gateway.auth.enable_api_key = false;
    gateway.auth.enable_jwt = false;
    assert!(
        gateway
            .validate()
            .unwrap_err()
            .contains("require API key or JWT")
    );
    handle.stop(false).await;
}

#[actix_web::test]
async fn streams_obey_idle_timeout_and_close_upstream_on_client_disconnect() {
    use actix_web::body::MessageBody;
    for idle_timeout in [0, 1] {
        let (state, client, calls, handle) = fixture(false).await;
        let mut revision = state.pin_runtime().as_ref().clone();
        let mut config = revision.config.as_ref().clone();
        config.gateway.server.stream_idle_timeout = idle_timeout;
        revision.config = Arc::new(config);
        state.runtime.store(revision);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .app_data(client)
                .configure(|c| configure_routes(c, 4096)),
        )
        .await;
        let response = test::call_service(
            &app,
            request(if idle_timeout == 0 { "slow" } else { "idle" }, &user()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = Box::pin(response.into_body());
        assert_eq!(
            futures::future::poll_fn(|cx| body.as_mut().poll_next(cx))
                .await
                .unwrap()
                .unwrap(),
            "data: {}\n\n"
        );
        if idle_timeout == 1 {
            assert!(
                tokio::time::timeout(
                    Duration::from_secs(3),
                    futures::future::poll_fn(|cx| body.as_mut().poll_next(cx))
                )
                .await
                .unwrap()
                .unwrap()
                .is_err()
            );
        }
        drop(body);
        tokio::time::timeout(Duration::from_secs(3), async {
            while !calls
                .lock()
                .unwrap()
                .iter()
                .any(|(value, _)| value["closed"] == true)
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("upstream body should close");
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn public_network_policy_blocks_private_targets_before_transport() {
    let (state, _, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    assert_eq!(
        test::call_service(&app, request("tools/list", &user()))
            .await
            .status(),
        StatusCode::BAD_GATEWAY
    );
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn server_permission_and_endpoint_restrictions_block_before_upstream() {
    use crate::core::models::ApiKey;
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for (permission, path) in [("mcp.other", "/docs/mcp"), ("mcp.docs", "/other/mcp")] {
        let mut key = ApiKey {
            metadata: Default::default(),
            name: "restricted".into(),
            key_hash: "test".into(),
            key_prefix: "test".into(),
            user_id: None,
            team_id: None,
            permissions: vec![permission.into()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: Default::default(),
        };
        key.metadata.set_extra(
            "__core_keys",
            json!({"permissions":{"allowed_endpoints":[path]}}),
        );
        let req = builder("tools/list").to_request();
        req.extensions_mut().insert(key);
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn active_request_limits_release_on_unread_body_drop_and_error() {
    let (state, client, calls, handle) = fixture(false).await;
    let owner = user();
    let principal = format!("user:{}", owner.id());
    let permits = (0..128)
        .map(|_| state.mcp_inflight.acquire(principal.clone()).unwrap())
        .collect::<Vec<_>>();
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    assert_eq!(
        test::call_service(&app, request("tools/list", &owner))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert!(calls.lock().unwrap().is_empty());
    drop(permits);
    let response = test::call_service(&app, request("tools/list", &owner)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.mcp_inflight.owners.lock().unwrap()[&principal], 1);
    drop(response);
    assert!(state.mcp_inflight.owners.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn finite_body_deadline_releases_admission_and_url_userinfo_is_rejected() {
    for auth in [false, true] {
        let mut config = McpServerConfig::new("docs", "https://user:secret@example.com/mcp");
        if auth {
            config.auth = Some(AuthConfig::bearer("test"));
        }
        assert!(
            config
                .validate_http_gateway("docs")
                .unwrap_err()
                .contains("userinfo")
        );
    }
    let (state, client, _, handle) = fixture(false).await;
    let mut revision = state.pin_runtime().as_ref().clone();
    let mut config = revision.config.as_ref().clone();
    config
        .gateway
        .mcp_servers
        .get_mut("docs")
        .unwrap()
        .timeout_ms = 100;
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let response = test::call_service(&app, request("stalled-json", &user())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        tokio::time::timeout(
            Duration::from_secs(2),
            actix_web::body::to_bytes(response.into_body())
        )
        .await
        .unwrap()
        .is_err()
    );
    assert!(state.mcp_inflight.owners.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn rejects_removed_initialization_with_or_without_id() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .app_data(client)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for method in ["initialize", "notifications/initialized"] {
        for include_id in [false, true] {
            let mut value = message(method);
            if !include_id {
                value.as_object_mut().unwrap().remove("id");
            }
            let req = builder(method).set_json(value).to_request();
            req.extensions_mut().insert(user());
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            let body: Value = test::read_body_json(response).await;
            assert_eq!(body["error"]["code"], -32601);
        }
    }
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn mcp_key_can_be_provisioned_through_public_auth_api() {
    use crate::core::models::user::types::UserStatus;
    let (state, client, calls, handle) = fixture(true).await;
    let mut owner = user();
    owner.status = UserStatus::Active;
    let owner = state.storage.db().create_user(&owner).await.unwrap();
    let (_, raw) = state
        .auth
        .create_api_key(owner.id(), "scoped-mcp".into(), vec!["mcp.docs".into()])
        .await
        .unwrap();
    let app =
        test::init_service(crate::server::http::HttpServer::create_app(state).app_data(client))
            .await;
    let req = builder("tools/list")
        .insert_header(("x-mcp-key", raw))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    drop(test::read_body(response).await);
    assert_eq!(calls.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[actix_web::test]
async fn stalled_request_bodies_are_admitted_before_reading_and_time_out() {
    let (state, _, calls, handle) = fixture(false).await;
    let mut revision = state.pin_runtime().as_ref().clone();
    let mut config = revision.config.as_ref().clone();
    config
        .gateway
        .mcp_servers
        .get_mut("docs")
        .unwrap()
        .timeout_ms = 50;
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let owner = user();
    let principal = format!("user:{}", owner.id());
    let (req, _) = builder("tools/list").uri("/mcp").to_http_parts();
    req.extensions_mut().insert(owner.clone());
    let pending = Box::pin(futures::stream::pending::<
        Result<web::Bytes, actix_web::error::PayloadError>,
    >())
        as std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<web::Bytes, actix_web::error::PayloadError>>>,
        >;
    let mut payload = actix_web::dev::Payload::from(pending);
    let payload = web::Payload::from_request(&req, &mut payload)
        .await
        .unwrap();
    let request = proxy(req, payload, state.clone());
    tokio::pin!(request);
    assert!(futures::poll!(request.as_mut()).is_pending());
    assert_eq!(state.mcp_inflight.owners.lock().unwrap()[&principal], 1);
    let permits = (1..128)
        .map(|_| state.mcp_inflight.acquire(principal.clone()).unwrap())
        .collect::<Vec<_>>();
    let (req, _) = builder("tools/list").uri("/mcp").to_http_parts();
    // A never-ready payload must not delay rejection when this owner is saturated.
    req.extensions_mut().insert(owner);
    let pending = Box::pin(futures::stream::pending::<
        Result<web::Bytes, actix_web::error::PayloadError>,
    >())
        as std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<web::Bytes, actix_web::error::PayloadError>>>,
        >;
    let mut payload = actix_web::dev::Payload::from(pending);
    let payload = web::Payload::from_request(&req, &mut payload)
        .await
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(1), proxy(req, payload, state.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let response = request.await;
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    drop(permits);
    assert!(state.mcp_inflight.owners.lock().unwrap().is_empty());
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn request_body_size_limit_still_returns_payload_too_large() {
    let (state, client, calls, handle) = fixture(false).await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .app_data(client)
            .configure(|c| configure_routes(c, 16)),
    )
    .await;
    let response = test::call_service(&app, request("tools/list", &user())).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(state.mcp_inflight.owners.lock().unwrap().is_empty());
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}
