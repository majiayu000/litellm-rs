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
            let bytes = format!("data: {first}\r\n\r\ndata: {last}\r\n\r\n").into_bytes();
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
    agent.capabilities.task_cancellation = true;
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
    json!({"message":{"role":"ROLE_USER","messageId":"m1","parts":[{"text":"hello"},{"url":"https://example.test/file.pdf","mediaType":"application/pdf"}]}})
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
    let value = json!({"result":{"task":{"id":"one","contextId":"ctx"}}});
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
            &json!({"result":{"message":{"messageId":"reply","parts":[{"text":"done"}]}}}),
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
    let mut params = message();
    params["configuration"] =
        json!({"taskPushNotificationConfig":{"url":"https://callback.example/task"}});
    let response = test::call_service(&app, request("SendMessage", params, &user())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
        .uri("/a2a/test?A2A-Version=1.0")
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
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(state.a2a_tasks.reserved.load(Ordering::Relaxed), 0);
    }
    handle.stop(false).await;
}
