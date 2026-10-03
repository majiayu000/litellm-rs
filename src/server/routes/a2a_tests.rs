use super::*;
use crate::core::{
    a2a::config::AgentConfig, models::user::types::User, net::ProviderEndpointAccess,
};
use actix_web::{App, HttpMessage, HttpServer, test};
use std::sync::Arc;
type Calls = Arc<Mutex<Vec<Value>>>;
async fn upstream(req: HttpRequest, body: web::Bytes, calls: web::Data<Calls>) -> HttpResponse {
    assert!(
        !std::str::from_utf8(&body)
            .unwrap()
            .contains("forbidden-first-id")
    );
    let body: Value = serde_json::from_slice(&body).unwrap();
    if let Some(case) = body
        .pointer("/params/metadata/test_media")
        .and_then(Value::as_str)
    {
        let value = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","status":{"state":"TASK_STATE_COMPLETED"}}}});
        let mut response = HttpResponse::Ok();
        if case == "plain" {
            response.insert_header(("content-type", "text/plain"));
        }
        return response.body(value.to_string());
    }
    if body.pointer("/params/metadata/test_bom") == Some(&Value::Bool(true)) {
        let first = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","status":{"state":"TASK_STATE_WORKING"}}}});
        let last = json!({"jsonrpc":"2.0","id":body["id"],"result":{"statusUpdate":{"taskId":"task-1","contextId":"late-context","status":{"state":"TASK_STATE_COMPLETED"}}}});
        let bytes = format!("\u{feff}data: {first}\n\ndata: {last}\n\n").into_bytes();
        return HttpResponse::Ok()
            .content_type("text/event-stream")
            .streaming(futures::stream::iter(
                bytes
                    .into_iter()
                    .map(|b| Ok::<_, std::io::Error>(web::Bytes::from(vec![b]))),
            ));
    }
    if let Some(result) = body.pointer("/params/metadata/test_result") {
        calls.lock().unwrap().push(body.clone());
        return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}));
    }
    if body.pointer("/params/metadata/test_bad_envelope") == Some(&Value::Bool(true)) {
        return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"error":null,"result":{"task":{"id":"orphan","contextId":"orphan-context"}}}));
    }
    if let Some(case) = body
        .pointer("/params/metadata/test_contract")
        .and_then(Value::as_str)
    {
        let initial = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}});
        if case == "ambiguous" {
            let mut response = initial.clone();
            response["result"]["message"] = json!({"messageId":"reply","contextId":"context-1"});
            return HttpResponse::Ok().json(response);
        }
        if case == "wrapped-message" {
            return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"message":{"messageId":"reply","taskId":"task-1","contextId":"context-1"}}}));
        }
        if case == "idle" {
            let mut initial = initial.clone();
            initial["result"]["task"]["status"]["state"] = json!("TASK_STATE_COMPLETED");
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(async_stream::stream! {
                    yield Ok::<_, std::io::Error>(web::Bytes::from_static(b": ready\n\n"));
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    yield Ok(web::Bytes::from(format!("data: {initial}\n\n")));
                });
        }
        if case == "json-result" {
            return HttpResponse::Ok().json(initial);
        }
        if case == "message-no-context" {
            return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"message":{"messageId":"reply","role":"ROLE_AGENT","parts":[{"text":"done"}]}}}));
        }
        let task = if case == "changed-task" {
            "unowned-task"
        } else {
            "task-1"
        };
        let context = if case == "changed-context" {
            "unowned-context"
        } else {
            "context-1"
        };
        let update = json!({"jsonrpc":"2.0","id":body["id"],"result":{"artifactUpdate":{"taskId":task,"contextId":context,"artifact":{"artifactId":"secret","parts":[{"text":"must not escape"}]}}}});
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .body(format!("data: {initial}\n\ndata: {update}\n\n"));
    }
    if body.pointer("/params/metadata/test_bad_stream") == Some(&Value::Bool(true)) {
        let event = json!({"jsonrpc":"2.0","id":body["id"],"result":{"statusUpdate":{"taskId":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}});
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .body(format!("data: {event}\n\n"));
    }
    if body.pointer("/params/metadata/test_direct_message") == Some(&Value::Bool(true)) {
        let event = json!({"jsonrpc":"2.0","id":body["id"],"result":{"message":{"messageId":"direct-reply","contextId":"direct-context","role":"ROLE_AGENT","parts":[{"text":"done"}]}}});
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .body(format!(
                "data: {event}\n\ndata: {{\"unexpected-extra-event\":true}}\n\n"
            ));
    }
    if let Some(state) = body
        .pointer("/params/metadata/test_terminal")
        .and_then(Value::as_str)
    {
        let task = json!({"id":"task-1","contextId":"context-1","status":{"state":state}});
        let event = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":task}});
        let initial = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}});
        let update = json!({"jsonrpc":"2.0","id":body["id"],"result":{"statusUpdate":{"taskId":"task-1","contextId":"context-1","status":{"state":state}}}});
        let events =
            if body.pointer("/params/metadata/test_status_update") == Some(&Value::Bool(true)) {
                format!("data: {initial}\n\ndata: {update}\n\n")
            } else {
                format!("data: {event}\n\n")
            };
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .streaming(async_stream::stream! {
                yield Ok::<_, std::io::Error>(web::Bytes::from(events));
                tokio::time::sleep(Duration::from_secs(10)).await;
                yield Ok(web::Bytes::from_static(b"data: invalid-post-terminal\n\n"));
            });
    }
    if body.pointer("/params/metadata/test_split_timeout") == Some(&Value::Bool(true)) {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let event = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","contextId":"context-1"}}}).to_string();
        return HttpResponse::Ok()
            .insert_header(("content-type", "application/json"))
            .streaming(async_stream::stream! {
                yield Ok::<_, std::io::Error>(web::Bytes::from_static(b" "));
                tokio::time::sleep(Duration::from_millis(150)).await;
                yield Ok(web::Bytes::from(event));
            });
    }
    if body.pointer("/params/metadata/test_big_error") == Some(&Value::Bool(true)) {
        return HttpResponse::TooManyRequests().body("x".repeat(1024));
    }
    if body.pointer("/params/metadata/test_slow_error") == Some(&Value::Bool(true)) {
        return HttpResponse::BadGateway().streaming(async_stream::stream! {
            yield Ok::<_, std::io::Error>(web::Bytes::from_static(b"first"));
            tokio::time::sleep(Duration::from_secs(5)).await;
            yield Ok(web::Bytes::from_static(b"late"));
        });
    }
    assert_eq!(
        req.headers().get("authorization").unwrap(),
        "Bearer upstream-test"
    );
    assert_eq!(req.headers().get("a2a-version").unwrap(), "1.0");
    calls.lock().unwrap().push(body.clone());
    if body.pointer("/params/metadata/test_limit") == Some(&Value::Bool(true)) {
        return HttpResponse::TooManyRequests().insert_header(("retry-after", "11")).json(json!({"jsonrpc":"2.0","id":body["id"],"error":{"code":-32000,"message":"limited"}}));
    }
    if body.pointer("/params/metadata/test_rpc_error") == Some(&Value::Bool(true)) {
        return HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"error":{"code":-32002,"message":"cannot cancel"}}));
    }
    if body.pointer("/params/metadata/test_slow") == Some(&Value::Bool(true)) {
        struct Closed(Calls);
        impl Drop for Closed {
            fn drop(&mut self) {
                self.0.lock().unwrap().push(json!({"closed":true}));
            }
        }
        let calls = calls.get_ref().clone();
        let stream = async_stream::stream! {
            let _closed = Closed(calls);
            loop { yield Ok::<_, std::io::Error>(web::Bytes::from_static(b": heartbeat\n\n")); tokio::time::sleep(Duration::from_millis(20)).await; }
        };
        return HttpResponse::Ok()
            .insert_header(("content-type", "Text/Event-Stream; charset=utf-8"))
            .streaming(stream);
    }
    let result = match body["method"].as_str().unwrap() {
        "SendMessage" => {
            json!({"task":{"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"},"artifacts":[{"artifactId":"file","parts":[{"raw":"aGVsbG8=","mediaType":"text/plain"}]}]}})
        }
        "GetTask" => {
            json!({"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}})
        }
        "CancelTask" => {
            json!({"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_CANCELED"}})
        }
        "SendStreamingMessage" | "SubscribeToTask" => {
            let first = json!({"jsonrpc":"2.0","id":body["id"],"result":{"task":{"id":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}});
            let last = json!({"jsonrpc":"2.0","id":body["id"],"result":{"artifactUpdate":{"taskId":"task-1","contextId":"context-1","artifact":{"artifactId":"one","parts":[{"text":"你好"}]},"lastChunk":true}}});
            let terminal = json!({"jsonrpc":"2.0","id":body["id"],"result":{"statusUpdate":{"taskId":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_COMPLETED"}}}});
            let bytes =
                format!("data: {first}\r\n\r\ndata: {last}\r\n\r\ndata: {terminal}\r\n\r\n")
                    .into_bytes();
            // Single-byte frames exercise UTF-8 and CRLF split across network chunks.
            return HttpResponse::Ok()
                .insert_header(("content-type", "Text/Event-Stream; charset=utf-8"))
                .insert_header(("a2a-extensions", "https://example.test/ext"))
                .streaming(futures::stream::iter(
                    bytes
                        .into_iter()
                        .map(|byte| Ok::<_, std::io::Error>(web::Bytes::from(vec![byte]))),
                ));
        }
        _ => unreachable!(),
    };
    HttpResponse::Ok().json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}))
}
async fn fixture() -> (web::Data<AppState>, Calls, actix_web::dev::ServerHandle) {
    let calls: Calls = Arc::default();
    let server_calls = calls.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/rpc", listener.local_addr().unwrap());
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(server_calls.clone()))
            .default_service(web::post().to(upstream))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let mut config = crate::server::valid_test_config();
    config.gateway.auth.allow_anonymous = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.api_key_header = "x-agent-key".into();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.server.cors.enabled = true;
    config.gateway.server.cors.allowed_origins = vec!["https://agent.example.test".into()];
    let server = crate::server::http::HttpServer::new(&config).await.unwrap();
    let mut state = server.state().clone();
    let mut agent = AgentConfig::new("test", &url).with_api_key("upstream-test");
    agent.capabilities.streaming = true;
    agent.capabilities.task_cancellation = false;
    config.gateway.a2a_agents.insert("test".into(), agent);
    let mut runtime = state.pin_runtime().as_ref().clone();
    runtime.config = Arc::new(config);
    state.runtime.store(runtime);
    state.a2a_tasks = Arc::new(TaskOwners {
        entries: Mutex::default(),
        reserved: AtomicUsize::new(0),
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
        "agent-user".into(),
        "agent@example.test".into(),
        "unused".into(),
    )
}
fn request(method: &str, params: Value, user: &User) -> actix_http::Request {
    let req = test::TestRequest::post()
        .uri("/a2a/test")
        .insert_header(("a2a-version", "1.0"))
        .set_json(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
        .to_request();
    req.extensions_mut().insert(user.clone());
    req
}
fn message() -> Value {
    json!({"configuration":{"returnImmediately":true},"message":{"role":"ROLE_USER","messageId":"m1","parts":[{"text":"hello"},{"url":"https://example.test/file.pdf","mediaType":"application/pdf"}]}})
}
#[actix_web::test]
async fn messages_tasks_cancellation_and_cards_reach_gateway() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let user = user();
    let response = test::call_service(&app, request("SendMessage", message(), &user)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(
        value["result"]["task"]["artifacts"][0]["parts"][0]["raw"],
        "aGVsbG8="
    );
    for method in ["GetTask", "CancelTask"] {
        let response =
            test::call_service(&app, request(method, json!({"id":"task-1"}), &user)).await;
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["result"]["id"], "task-1");
    }
    assert_eq!(calls.lock().unwrap()[0]["params"], message());
    let req = test::TestRequest::get()
        .uri("/a2a/test/.well-known/agent-card.json")
        .insert_header(("a2a-version", "1.0"))
        .to_request();
    req.extensions_mut().insert(user);
    let card: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(card["supportedInterfaces"][0]["protocolVersion"], "1.0");
    assert_eq!(
        card["securitySchemes"]["gateway_key"]["apiKeySecurityScheme"]["name"],
        "x-agent-key"
    );
    assert!(!card.to_string().contains("upstream-test"));
    assert_eq!(card["capabilities"]["pushNotifications"], false);
    assert_eq!(card["defaultInputModes"], json!(["text/plain"]));
    assert_eq!(card["defaultOutputModes"], json!(["text/plain"]));
    assert_eq!(card["skills"][0]["id"], "gateway");
    assert!(!card["skills"][0]["tags"].as_array().unwrap().is_empty());
    handle.stop(false).await;
}
#[actix_web::test]
async fn streaming_registers_ownership_before_task_can_be_resumed() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let user = user();
    let response =
        test::call_service(&app, request("SendStreamingMessage", message(), &user)).await;
    assert_eq!(
        response.headers().get("a2a-extensions").unwrap(),
        "https://example.test/ext"
    );
    let body = test::read_body(response).await;
    assert!(std::str::from_utf8(&body).unwrap().contains("你好"));
    let response = test::call_service(
        &app,
        request("SubscribeToTask", json!({"id":"task-1"}), &user),
    )
    .await;
    assert!(
        std::str::from_utf8(&test::read_body(response).await)
            .unwrap()
            .contains("artifactUpdate")
    );
    handle.stop(false).await;
}
#[actix_web::test]
async fn rejects_foreign_tasks_contexts_and_references_before_upstream() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let stranger = user();
    let _ =
        test::read_body(test::call_service(&app, request("SendMessage", message(), &owner)).await)
            .await;
    for (method, params) in [
        ("GetTask", json!({"id":"task-1"})),
        ("CancelTask", json!({"id":"task-1"})),
        ("SubscribeToTask", json!({"id":"task-1"})),
        (
            "SendMessage",
            json!({"message":{"contextId":"context-1","messageId":"m2","parts":[{"text":"steal"}]}}),
        ),
        (
            "SendMessage",
            json!({"message":{"referenceTaskIds":["task-1"],"messageId":"m2","parts":[{"text":"steal"}]}}),
        ),
    ] {
        let result: Value = test::read_body_json(
            test::call_service(&app, request(method, params, &stranger)).await,
        )
        .await;
        assert_eq!(result["error"]["code"], -32001);
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    handle.stop(false).await;
}
#[actix_web::test]
async fn anonymous_production_route_requires_authentication() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(crate::server::http::HttpServer::create_app(state)).await;
    let req = test::TestRequest::post()
        .uri("/a2a/test")
        .insert_header(("a2a-version", "1.0"))
        .set_json(json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":message()}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}
#[test]
async fn ownership_rejects_collisions_expiry_and_account_changes() {
    let owners = TaskOwners::default();
    let value = json!({"result":{"task":{"id":"one","contextId":"ctx","status":{"state":"TASK_STATE_WORKING"}}}});
    owners
        .observe(b"account-1", "alice", &value, None, None, None)
        .unwrap();
    assert!(
        owners
            .observe(b"account-1", "bob", &value, None, None, None)
            .is_err()
    );
    assert!(!owners.owns(b"account-2", "alice", "task", "one"));
    for entry in owners.entries.lock().unwrap().values_mut() {
        entry.expires = Instant::now();
    }
    assert!(!owners.owns(b"account-1", "alice", "task", "one"));
}

#[actix_web::test]
async fn preserves_upstream_http_and_rpc_errors() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let user = user();
    let mut params = message();
    params["metadata"] = json!({"test_limit":true});
    let response = test::call_service(&app, request("SendMessage", params, &user)).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("retry-after").unwrap(), "11");
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["message"], "limited");
    let mut params = message();
    params["metadata"] = json!({"test_rpc_error":true});
    let response = test::call_service(&app, request("SendMessage", params, &user)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["code"], -32002);
    assert_eq!(calls.lock().unwrap().len(), 2);
    handle.stop(false).await;
}
#[actix_web::test]
async fn dropping_stream_releases_upstream() {
    use actix_web::body::MessageBody;
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let mut params = message();
    params["metadata"] = json!({"test_slow":true});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &user())).await;
    let mut body = Box::pin(response.into_body());
    assert!(
        futures::future::poll_fn(|cx| body.as_mut().poll_next(cx))
            .await
            .unwrap()
            .is_ok()
    );
    drop(body);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if calls.lock().unwrap().iter().any(|v| v["closed"] == true) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    handle.stop(false).await;
}
#[actix_web::test]
async fn real_key_authentication_and_endpoint_permissions() {
    use crate::core::models::{ApiKey, user::types::UserStatus};
    use crate::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    let (state, calls, handle) = fixture().await;
    let mut user = user();
    user.status = UserStatus::Active;
    let user = state.storage.db().create_user(&user).await.unwrap();
    let raw = "gw-a2a-test-12345678901234567890";
    let mut key = ApiKey {
        metadata: Default::default(),
        name: "a2a-test".into(),
        key_hash: hash_api_key(raw, None),
        key_prefix: extract_api_key_prefix(raw),
        user_id: Some(user.id()),
        team_id: None,
        permissions: vec!["a2a.test".into()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: Default::default(),
    };
    key.metadata.set_extra(
        "__core_keys",
        json!({"permissions":{"allowed_endpoints":["/a2a/test"]}}),
    );
    state.storage.db().create_api_key(&key).await.unwrap();
    let app = test::init_service(crate::server::http::HttpServer::create_app(state)).await;
    let preflight = test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/a2a/test")
        .insert_header(("origin", "https://agent.example.test"))
        .insert_header(("access-control-request-method", "POST"))
        .insert_header((
            "access-control-request-headers",
            "x-agent-key,content-type,a2a-version,a2a-extensions",
        ))
        .to_request();
    assert!(
        test::call_service(&app, preflight)
            .await
            .status()
            .is_success()
    );
    let req = test::TestRequest::post()
        .uri("/a2a/test")
        .insert_header(("origin", "https://agent.example.test"))
        .insert_header(("a2a-version", "1.0"))
        .insert_header(("x-agent-key", raw))
        .set_json(json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":message()}))
        .to_request();
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get("access-control-expose-headers")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("a2a-version")
    );
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["result"]["task"]["id"], "task-1");
    let req = test::TestRequest::get()
        .uri("/a2a/test/.well-known/agent-card.json")
        .insert_header(("x-agent-key", raw))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[actix_web::test]
async fn config_validation_and_export_do_not_expose_agent_credentials() {
    let mut agent = AgentConfig::new("test", "https://agent.example/rpc?token=sentinel-url")
        .with_api_key("sentinel-key")
        .with_header("x-token", "sentinel-header");
    agent.validate_http_gateway("test").unwrap();
    assert!(!format!("{agent:?}").contains("sentinel"));
    let mut config = crate::server::valid_test_config();
    config
        .gateway
        .a2a_agents
        .insert("test".into(), agent.clone());
    let exported = config.to_json().unwrap();
    assert!(!exported.contains("sentinel"));
    agent.capabilities.input_types = vec!["text".into()];
    assert!(agent.validate_http_gateway("test").is_err());
    agent.capabilities = crate::core::a2a::config::AgentCapabilities::minimal();
    agent.validate_http_gateway("test").unwrap();
    agent.cost_per_request = Some(1.0);
    assert!(agent.validate_http_gateway("test").is_err());
    agent.cost_per_request = None;
    agent.headers.insert("A2A-Version".into(), "0.3".into());
    assert!(agent.validate_http_gateway("test").is_err());
    let owners = TaskOwners::default();
    owners
        .observe(
            b"account",
            "alice",
            &json!({"result":{"message":{"messageId":"reply","contextId":"reply-context","role":"ROLE_AGENT","parts":[{"text":"done"}]}}}),
            None,
            None,
            None,
        )
        .unwrap();
}

#[actix_web::test]
async fn rejects_inline_push_notification_before_upstream() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for field in ["pushNotificationConfig", "taskPushNotificationConfig"] {
        let mut params = message();
        params["configuration"] = json!({field:{"url":"https://callback.example/task"}});
        let response = test::call_service(&app, request("SendMessage", params, &user())).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn equivalent_config_reload_preserves_task_ownership() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
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
        let agent = config.gateway.a2a_agents.get_mut("test").unwrap();
        agent.headers = headers;
        if reverse {
            agent.description = Some("updated description".into());
            agent.timeout_ms += 100;
            agent.tags.push("updated".into());
        }
        revision.config = Arc::new(config);
        state.runtime.store(revision);
        let response = test::call_service(
            &app,
            request(
                if reverse { "GetTask" } else { "SendMessage" },
                if reverse {
                    json!({"id":"task-1"})
                } else {
                    message()
                },
                &owner,
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert!(value.get("error").is_none(), "{value}");
    }
    let response =
        test::call_service(&app, request("GetTask", json!({"id":"unknown"}), &owner)).await;
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    handle.stop(false).await;
}

#[actix_web::test]
async fn capacity_rejection_precedes_upstream_and_reservations_release() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    {
        let mut entries = state.a2a_tasks.entries.lock().unwrap();
        for index in 0..4094 {
            entries.insert(
                (vec![], "task".into(), index.to_string()),
                Owner {
                    principal: "other".into(),
                    expires: Instant::now() + Duration::from_secs(60),
                    context: None,
                },
            );
        }
    }
    let reservation = state.a2a_tasks.reserve(2).unwrap();
    for method in ["SendMessage", "SendStreamingMessage"] {
        let response = test::call_service(&app, request(method, message(), &user())).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
    assert!(calls.lock().unwrap().is_empty());
    drop(reservation);
    assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    state.a2a_tasks.entries.lock().unwrap().clear();
    let mut params = message();
    params["metadata"] = json!({"test_limit":true});
    assert_eq!(
        test::call_service(&app, request("SendMessage", params, &user()))
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    assert!(state.a2a_tasks.entries.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn accepts_query_version_and_forwards_authorized_json_only() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let req = test::TestRequest::post()
        .uri("/a2a/test?a2a-version=1.0")
        .set_json(json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":message()}))
        .to_request();
    req.extensions_mut().insert(owner.clone());
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    let req = test::TestRequest::post().uri("/a2a/test").insert_header(("a2a-version", "1.0"))
        .insert_header(("content-type", "application/json"))
        .set_payload(r#"{"jsonrpc":"2.0","id":1,"method":"CancelTask","params":{"id":"forbidden-first-id","id":"task-1"}}"#).to_request();
    req.extensions_mut().insert(owner);
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    assert_eq!(calls.lock().unwrap()[1]["params"]["id"], "task-1");
    handle.stop(false).await;
}

#[actix_web::test]
async fn error_bodies_obey_size_and_time_bounds() {
    let (state, _, handle) = fixture().await;
    let mut revision = state.pin_runtime().as_ref().clone();
    let mut config = revision.config.as_ref().clone();
    config.gateway.server.max_body_size = 64;
    config
        .gateway
        .a2a_agents
        .get_mut("test")
        .unwrap()
        .timeout_ms = 100;
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for field in ["test_big_error", "test_slow_error"] {
        let mut params = message();
        params["metadata"] = json!({field:true});
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            test::call_service(&app, request("SendMessage", params, &user())),
        )
        .await
        .unwrap();
        let timeout = field == "test_slow_error";
        assert_eq!(
            response.status(),
            if timeout {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            }
        );
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], if timeout { -32603 } else { -32006 });
        assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn invalid_envelopes_and_stream_sequences_release_reservations() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let mut params = message();
    params["metadata"] = json!({"test_bad_envelope":true});
    let response = test::call_service(&app, request("SendMessage", params, &owner)).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["code"], -32006);
    let mut params = message();
    params["metadata"] = json!({"test_bad_stream":true});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
    assert!(
        actix_web::body::to_bytes(response.into_body())
            .await
            .is_err()
    );
    assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    assert!(state.a2a_tasks.entries.lock().unwrap().is_empty());
    let mut params = message();
    params["metadata"] = json!({"test_direct_message":true});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
    let body = test::read_body(response).await;
    let text = std::str::from_utf8(&body).unwrap();
    assert!(text.contains("direct-reply"));
    assert!(!text.contains("unexpected-extra-event"));
    assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    handle.stop(false).await;
}

#[actix_web::test]
async fn terminal_task_events_close_without_waiting_for_upstream_eof() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for status_update in [false, true] {
        for terminal in [
            "TASK_STATE_COMPLETED",
            "TASK_STATE_FAILED",
            "TASK_STATE_CANCELED",
            "TASK_STATE_REJECTED",
        ] {
            let mut params = message();
            params["metadata"] =
                json!({"test_terminal":terminal,"test_status_update":status_update});
            let response =
                test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
            let bytes = tokio::time::timeout(Duration::from_secs(2), test::read_body(response))
                .await
                .unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            assert!(text.contains(terminal));
            assert!(!text.contains("invalid-post-terminal"));
        }
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn finite_response_uses_one_deadline_and_root_card_discovers_single_agent() {
    let (state, _, handle) = fixture().await;
    let mut revision = state.pin_runtime().as_ref().clone();
    let mut config = revision.config.as_ref().clone();
    config
        .gateway
        .a2a_agents
        .get_mut("test")
        .unwrap()
        .timeout_ms = 250;
    revision.config = Arc::new(config);
    state.runtime.store(revision);
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/.well-known/agent-card.json")
        .insert_header(("a2a-version", "1.0"))
        .to_request();
    req.extensions_mut().insert(user());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let card: Value = test::read_body_json(response).await;
    assert_eq!(card["name"], "test");
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(card["supportedInterfaces"][0]["protocolVersion"], "1.0");
    let mut params = message();
    params["metadata"] = json!({"test_split_timeout":true});
    let response = test::call_service(&app, request("SendMessage", params, &user())).await;
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["code"], -32603);
    assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    handle.stop(false).await;
}

#[actix_web::test]
async fn stream_identity_and_finite_response_contracts_fail_closed() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for case in ["changed-task", "changed-context"] {
        let mut params = message();
        params["metadata"] = json!({"test_contract":case});
        let response =
            test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
        assert!(
            actix_web::body::to_bytes(response.into_body())
                .await
                .is_err()
        );
        assert!(
            !state
                .a2a_tasks
                .entries
                .lock()
                .unwrap()
                .keys()
                .any(|(_, _, id)| id.starts_with("unowned-"))
        );
        assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    }
    for (method, case) in [
        ("SendStreamingMessage", "json-result"),
        ("SubscribeToTask", "json-result"),
    ] {
        let mut params = if method == "SubscribeToTask" {
            json!({"id":"task-1"})
        } else {
            message()
        };
        params["metadata"] = json!({"test_contract":case});
        let response = test::call_service(&app, request(method, params, &owner)).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], -32006);
    }
    let mut params = message();
    params["metadata"] = json!({"test_rpc_error":true});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["error"]["code"], -32002);
    handle.stop(false).await;
}

#[actix_web::test]
async fn api_key_header_cannot_replace_a2a_protocol_headers() {
    let mut config = crate::server::valid_test_config();
    config.gateway.auth.enable_api_key = true;
    config.gateway.a2a_agents.insert(
        "test".into(),
        AgentConfig::new("test", "https://agent.example/rpc"),
    );
    for header in ["Accept", "Content-Type", "A2A-Version", "A2A-Extensions"] {
        config.gateway.auth.api_key_header = header.into();
        assert!(
            config
                .gateway
                .validate()
                .unwrap_err()
                .contains("protocol header")
        );
    }
    config.gateway.auth.api_key_header = "x-agent-key".into();
    config.gateway.validate().unwrap();
    config.gateway.a2a_agents.get_mut("test").unwrap().enabled = false;
    config.gateway.auth.api_key_header = "content-type".into();
    config.gateway.validate().unwrap();
}

#[actix_web::test]
async fn finite_result_must_match_the_requested_a2a_method() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    assert_eq!(
        test::call_service(&app, request("SendMessage", message(), &owner))
            .await
            .status(),
        StatusCode::OK
    );
    for (method, case) in [
        ("SendMessage", "ambiguous"),
        ("GetTask", "wrapped-message"),
        ("CancelTask", "wrapped-message"),
    ] {
        let mut params = if method == "SendMessage" {
            message()
        } else {
            json!({"id":"task-1"})
        };
        params["metadata"] = json!({"test_contract":case});
        let response = test::call_service(&app, request(method, params, &owner)).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], -32006);
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn stream_idle_timeout_is_enforced_and_zero_disables_it() {
    for seconds in [1, 0] {
        let (state, _, handle) = fixture().await;
        let mut revision = state.pin_runtime().as_ref().clone();
        let mut config = revision.config.as_ref().clone();
        config.gateway.server.stream_idle_timeout = seconds;
        revision.config = Arc::new(config);
        state.runtime.store(revision);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(|c| configure_routes(c, 4096)),
        )
        .await;
        let mut params = message();
        params["metadata"] = json!({"test_contract":"idle"});
        let response =
            test::call_service(&app, request("SendStreamingMessage", params, &user())).await;
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            actix_web::body::to_bytes(response.into_body()),
        )
        .await
        .unwrap();
        assert_eq!(result.is_err(), seconds != 0);
        assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
        handle.stop(false).await;
    }
}

#[actix_web::test]
async fn enabled_agents_require_an_authentication_method() {
    let mut config = crate::server::valid_test_config();
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.a2a_agents.insert(
        "test".into(),
        AgentConfig::new("test", "https://agent.example/rpc"),
    );
    assert!(
        config
            .gateway
            .validate()
            .unwrap_err()
            .contains("require API key or JWT")
    );
    config.gateway.a2a_agents.get_mut("test").unwrap().enabled = false;
    config.gateway.validate().unwrap();
}

#[actix_web::test]
async fn response_variant_identity_and_required_fields_are_checked_before_ownership() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for result in [
        json!({"task":{"id":"other-task","taskId":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}),
        json!({"task":{"id":"task-1","contextId":"context-1"}}),
        json!({"task":{"id":"task-1","contextId":"context-1","status":{}}}),
        json!({"message":{"id":"other-task","taskId":"task-1","messageId":"reply","contextId":"context-1","role":"ROLE_AGENT","parts":[{"text":"bad"}]}}),
        json!({"message":{"messageId":"reply","contextId":"context-1","parts":[{"text":"bad"}]}}),
        json!({"message":{"messageId":"reply","contextId":"context-1","role":"ROLE_AGENT","parts":[]}}),
    ] {
        let mut params = message();
        params["metadata"] = json!({"test_result":result});
        let response = test::call_service(&app, request("SendMessage", params, &user())).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["code"], -32006);
        assert!(state.a2a_tasks.entries.lock().unwrap().is_empty());
    }
    let owner = user();
    let _: Value =
        test::call_and_read_body_json(&app, request("SendMessage", message(), &owner)).await;
    let response=test::call_service(&app,request("GetTask",json!({"id":"task-1","metadata":{"test_result":{"id":"other-task","taskId":"task-1","contextId":"context-1","status":{"state":"TASK_STATE_WORKING"}}}}),&owner)).await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    assert!(
        !state
            .a2a_tasks
            .entries
            .lock()
            .unwrap()
            .keys()
            .any(|(_, _, id)| id == "other-task")
    );
    handle.stop(false).await;
}

#[actix_web::test]
async fn stream_eof_before_terminal_state_is_an_error() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let mut params = message();
    params["metadata"] = json!({"test_contract":"early-eof"});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &user())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        actix_web::body::to_bytes(response.into_body())
            .await
            .is_err()
    );
    handle.stop(false).await;
}

#[actix_web::test]
async fn client_cannot_impersonate_agent_role() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for role in [json!("ROLE_AGENT"), Value::Null] {
        for method in ["SendMessage", "SendStreamingMessage"] {
            let mut params = message();
            params["message"]["role"] = role.clone();
            let response = test::call_service(&app, request(method, params, &user())).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn credentials_require_encrypted_upstream_transport() {
    for scheme in ["http", "https"] {
        for api_key in [None, Some("another-test-key")] {
            let mut agent =
                AgentConfig::new("test", format!("{scheme}://user:sentinel@example.com/rpc"));
            agent.api_key = api_key.map(str::to_owned);
            let error = agent.validate_http_gateway("test").unwrap_err();
            assert!(error.contains("userinfo"));
            assert!(!error.contains("sentinel"));
        }
    }
    for headers in [false, true] {
        let mut agent = AgentConfig::new("test", "http://example.com/rpc");
        if headers {
            agent.headers.insert("x-secret".into(), "test".into());
        } else {
            agent.api_key = Some("test".into());
        }
        assert!(
            agent
                .validate_http_gateway("test")
                .unwrap_err()
                .contains("HTTPS")
        );
        agent.url = "https://example.com/rpc".into();
        agent.validate_http_gateway("test").unwrap();
    }
}

#[actix_web::test]
async fn optional_task_context_and_split_sse_bom_are_supported() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    let mut params = message();
    params["metadata"] = json!({"test_contract":"message-no-context"});
    assert_eq!(
        test::call_service(&app, request("SendMessage", params, &owner))
            .await
            .status(),
        StatusCode::OK
    );
    for method in ["SendMessage", "GetTask", "CancelTask"] {
        let task = json!({"id":"task-1","status":{"state":"TASK_STATE_WORKING"}});
        let mut params = if method == "SendMessage" {
            message()
        } else {
            json!({"id":"task-1"})
        };
        params["metadata"] =
            json!({"test_result":if method == "SendMessage" {json!({"task":task})} else {task}});
        let response = test::call_service(&app, request(method, params, &owner)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let mut params = message();
    params["metadata"] = json!({"test_bom":true});
    let response = test::call_service(&app, request("SendStreamingMessage", params, &owner)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = test::read_body(response).await;
    assert!(String::from_utf8_lossy(&bytes).contains("TASK_STATE_COMPLETED"));
    handle.stop(false).await;
}

#[actix_web::test]
async fn review_rejects_unknown_task_states_before_recording_ownership() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for status in [json!("TASK_STATE_COMPLETE"), json!("unknown"), json!(42)] {
        let mut params = message();
        params["metadata"] =
            json!({"test_result":{"task":{"id":"invalid-state","status":{"state":status}}}});
        let response = test::call_service(&app, request("SendMessage", params, &user())).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(state.a2a_tasks.entries.lock().unwrap().is_empty());
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn review_rejects_mismatched_owned_task_context_pairs() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for (task, context) in [("task-1", "context-1"), ("task-2", "context-2")] {
        let mut params = message();
        params["metadata"] = json!({"test_result":{"task":{"id":task,"contextId":context,"status":{"state":"TASK_STATE_WORKING"}}}});
        let response = test::call_service(&app, request("SendMessage", params, &owner)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let before = calls.lock().unwrap().len();
    let mut params = message();
    params["message"]["taskId"] = json!("task-1");
    params["message"]["contextId"] = json!("context-2");
    let response = test::call_service(&app, request("SendMessage", params, &owner)).await;
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["code"], -32602);
    assert_eq!(calls.lock().unwrap().len(), before);
    handle.stop(false).await;
}

#[actix_web::test]
async fn review_card_negotiates_header_and_query_versions() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for path in [
        "/.well-known/agent-card.json",
        "/a2a/test/.well-known/agent-card.json",
    ] {
        for version in [None, Some("0.3"), Some("1.0")] {
            let mut req = test::TestRequest::get().uri(path);
            if let Some(version) = version {
                req = req.insert_header(("A2A-Version", version));
            }
            let req = req.to_request();
            req.extensions_mut().insert(user());
            let response = test::call_service(&app, req).await;
            if version == Some("1.0") {
                assert_eq!(response.status(), StatusCode::OK);
            } else {
                let value: Value = test::read_body_json(response).await;
                assert_eq!(value["error"]["code"], -32009);
            }
        }
        let req = test::TestRequest::get()
            .uri(&format!("{path}?a2A-vErSiOn=1.0"))
            .to_request();
        req.extensions_mut().insert(user());
        assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn review_distinguishes_protocol_method_and_media_errors() {
    let (state, calls, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    for (method, code) in [
        ("ListTasks", -32004),
        ("GetExtendedAgentCard", -32004),
        ("CreateTaskPushNotificationConfig", -32003),
        ("GetTaskPushNotificationConfig", -32003),
        ("ListTaskPushNotificationConfigs", -32003),
        ("DeleteTaskPushNotificationConfig", -32003),
        ("UnknownMethod", -32601),
    ] {
        let response = test::call_service(&app, request(method, json!({}), &user())).await;
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["error"]["code"], code, "{method}: {value}");
    }
    let req = test::TestRequest::post()
        .uri("/a2a/test")
        .insert_header(("content-type", "text/plain"))
        .set_payload("{}")
        .to_request();
    req.extensions_mut().insert(user());
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["code"], -32005);
    assert!(calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[actix_web::test]
async fn successful_finite_response_requires_json_media_type() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for media in ["plain", "missing"] {
        let mut params = message();
        params["metadata"] = json!({"test_media":media});
        let response = test::call_service(&app, request("SendMessage", params, &owner)).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn blocking_send_rejects_active_tasks_and_accepts_interrupted_or_terminal_tasks() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let owner = user();
    for immediate in [None, Some(false), Some(true)] {
        for state in [
            "TASK_STATE_UNSPECIFIED",
            "TASK_STATE_SUBMITTED",
            "TASK_STATE_WORKING",
            "TASK_STATE_COMPLETED",
            "TASK_STATE_INPUT_REQUIRED",
            "TASK_STATE_AUTH_REQUIRED",
        ] {
            let mut params = message();
            params.as_object_mut().unwrap().remove("configuration");
            if let Some(value) = immediate {
                params["configuration"] = json!({"returnImmediately":value});
            }
            params["metadata"] =
                json!({"test_result":{"task":{"id":"task-1","status":{"state":state}}}});
            let response = test::call_service(&app, request("SendMessage", params, &owner)).await;
            let expected = if immediate != Some(true)
                && matches!(
                    state,
                    "TASK_STATE_UNSPECIFIED" | "TASK_STATE_SUBMITTED" | "TASK_STATE_WORKING"
                ) {
                StatusCode::BAD_GATEWAY
            } else {
                StatusCode::OK
            };
            assert_eq!(
                response.status(),
                expected,
                "{state} immediate {immediate:?}"
            );
        }
    }
    handle.stop(false).await;
}

#[actix_web::test]
async fn card_ignores_forwarded_origin_from_untrusted_peers() {
    let (state, _, handle) = fixture().await;
    let app = test::init_service(
        App::new()
            .app_data(state)
            .configure(|c| configure_routes(c, 4096)),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/a2a/test/.well-known/agent-card.json")
        .insert_header(("a2a-version", "1.0"))
        .insert_header(("host", "gateway.example.test"))
        .insert_header(("forwarded", "host=attacker.example.test;proto=https"))
        .insert_header(("x-forwarded-host", "attacker.example.test"))
        .insert_header(("x-forwarded-proto", "https"))
        .peer_addr("192.0.2.10:1234".parse().unwrap())
        .to_request();
    req.extensions_mut().insert(user());
    let response = test::call_service(&app, req).await;
    let value: Value = test::read_body_json(response).await;
    assert_eq!(
        value["supportedInterfaces"][0]["url"],
        "http://gateway.example.test/a2a/test"
    );
    handle.stop(false).await;
}

#[actix_web::test]
async fn retained_context_ownership_is_renewed_with_its_task() {
    let owners = TaskOwners::default();
    let binding = b"binding";
    owners.observe(binding, "owner", &json!({"result":{"task":{"id":"t","contextId":"c","status":{"state":"TASK_STATE_WORKING"}}}}), None, None, None).unwrap();
    let old_expiry = Instant::now() + Duration::from_secs(5);
    owners
        .entries
        .lock()
        .unwrap()
        .get_mut(&(binding.to_vec(), "context".into(), "c".into()))
        .unwrap()
        .expires = old_expiry;
    owners
        .observe(
            binding,
            "owner",
            &json!({"result":{"id":"t","status":{"state":"TASK_STATE_WORKING"}}}),
            Some("t"),
            None,
            None,
        )
        .unwrap();
    assert!(
        owners.entries.lock().unwrap()[&(binding.to_vec(), "context".into(), "c".into())].expires
            > old_expiry + Duration::from_secs(3000)
    );
}

#[actix_web::test]
async fn private_agent_requires_named_key_permission() {
    use crate::core::models::user::types::UserStatus;
    let (state, calls, handle) = fixture().await;
    let mut owner = user();
    owner.status = UserStatus::Active;
    let owner = state.storage.db().create_user(&owner).await.unwrap();
    let mut keys = Vec::new();
    for permissions in [
        vec![],
        vec!["api.chat".into()],
        vec!["a2a.other".into()],
        vec!["a2a.test".into()],
        vec!["system.admin".into()],
    ] {
        let allowed = permissions
            .iter()
            .any(|p| p == "a2a.test" || p == "system.admin");
        let (_, raw) = state
            .auth
            .create_api_key(owner.id(), "agent-permission".into(), permissions)
            .await
            .unwrap();
        keys.push((raw, allowed));
    }
    let app = test::init_service(crate::server::http::HttpServer::create_app(state)).await;
    for (index, (raw, allowed)) in keys.into_iter().enumerate() {
        let mut params = message();
        params["metadata"] = json!({"test_result":{"task":{"id":format!("task-{index}"),"status":{"state":"TASK_STATE_COMPLETED"}}}});
        for path in ["/a2a/test", "/a2a/test/.well-known/agent-card.json"] {
            let mut req = if path.ends_with("json") {
                test::TestRequest::get()
            } else {
                test::TestRequest::post().set_json(
                    json!({"jsonrpc":"2.0","id":1,"method":"SendMessage","params":params}),
                )
            };
            req = req
                .uri(path)
                .insert_header(("a2a-version", "1.0"))
                .insert_header(("x-agent-key", raw.as_str()));
            let response = test::call_service(&app, req.to_request()).await;
            assert_eq!(
                response.status(),
                if allowed {
                    StatusCode::OK
                } else {
                    StatusCode::FORBIDDEN
                }
            );
        }
    }
    assert_eq!(calls.lock().unwrap().len(), 2);
    handle.stop(false).await;
}
