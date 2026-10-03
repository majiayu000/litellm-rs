#![cfg(all(feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
use bytes::Bytes;
use litellm_rs::{Config, server::state::AppState};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

type CapturedRequest = (Value, String, String, String);

#[derive(Clone)]
struct Upstream {
    seen: Arc<Mutex<Vec<CapturedRequest>>>,
    counted: Arc<Mutex<Vec<Value>>>,
    count_result: Arc<Mutex<Value>>,
    reported_usage: Arc<Mutex<Value>>,
    status: StatusCode,
    broken: bool,
    fault: Arc<Mutex<Option<&'static str>>>,
}
fn usage() -> Value {
    json!({"inference_geo":"global","input_tokens":40,"output_tokens":10,"cache_creation_input_tokens":50,"cache_read_input_tokens":10,"cache_creation":{"ephemeral_1h_input_tokens":20,"ephemeral_5m_input_tokens":30},"server_tool_use":{"web_search_requests":2,"web_fetch_requests":1}})
}
fn output() -> Value {
    json!({"id":"msg_native","type":"message","role":"assistant","model":"claude-opus-5","content":[{"type":"thinking","thinking":"思考中","signature":"opaque-signature"},{"type":"tool_use","id":"tool_1","name":"find","input":{"query":"你好"}}],"stop_reason":"tool_use","usage":usage(),"future_output":{"keep":true}})
}
async fn upstream(
    data: web::Data<Upstream>,
    req: HttpRequest,
    body: web::Json<Value>,
) -> HttpResponse {
    let header = |name| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string()
    };
    data.seen.lock().unwrap().push((
        body.clone(),
        header("x-api-key"),
        header("anthropic-version"),
        header("anthropic-beta"),
    ));
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "1"))
            .json(
                json!({"type":"error","error":{"type":"overloaded_error","message":"Unavailable"}}),
            );
    }
    let mut result = output();
    result["usage"] = data.reported_usage.lock().unwrap().clone();
    result["model"] = body["model"].clone();
    if body["stream"] == true {
        let mut first = result;
        first["content"] = json!([]);
        first["stop_reason"] = Value::Null;
        first["usage"]["output_tokens"] = json!(1);
        let mut events = vec![
            json!({"type":"message_start","message":first}),
            json!({"type":"ping"}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"思考中"}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"opaque-signature"}}),
            json!({"type":"content_block_stop","index":0}),
        ];
        if data.broken {
            events.push(json!({"type":"error","error":{"type":"overloaded_error","message":"Capacity exhausted"}}));
        } else {
            events.push(json!({"type":"message_delta","delta":{"stop_reason":null},"usage":{"output_tokens":5}}));
            events.push(json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":10},"future_event":true}));
            events.push(json!({"type":"message_stop"}));
        }
        match *data.fault.lock().unwrap() {
            Some("missing_start") => {
                events.remove(0);
            }
            Some("missing_stop") => {
                events.pop();
            }
            Some("missing_delta") => {
                events.retain(|event| event["type"] != "message_delta");
            }
            Some("bad_usage") => {
                for event in &mut events {
                    if event["type"] == "message_delta" {
                        event["usage"]["output_tokens"] = json!(-1);
                    }
                }
            }
            _ => {}
        }
        let wire = events
            .into_iter()
            .map(|v| {
                format!(
                    "event: {}\r\ndata: {v}\r\n\r\n",
                    v["type"].as_str().unwrap()
                )
            })
            .collect::<String>();
        let chunks = wire
            .as_bytes()
            .chunks(3)
            .map(|chunk| Ok::<_, actix_web::Error>(Bytes::copy_from_slice(chunk)))
            .collect::<Vec<_>>();
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .streaming(futures::stream::iter(chunks));
    }
    HttpResponse::Ok().json(result)
}
async fn count_tokens(
    data: web::Data<Upstream>,
    req: HttpRequest,
    body: web::Json<Value>,
) -> HttpResponse {
    assert_eq!(
        req.headers().get("x-api-key").unwrap(),
        "sk-ant-test-not-a-real-key-1234567890123456789"
    );
    data.counted.lock().unwrap().push(body.into_inner());
    HttpResponse::Ok().json(data.count_result.lock().unwrap().clone())
}
async fn fixture(
    status: StatusCode,
    broken: bool,
    mutate: impl FnOnce(&mut Config),
) -> (AppState, Upstream, actix_web::dev::ServerHandle) {
    let upstream_state = Upstream {
        seen: Arc::default(),
        counted: Arc::default(),
        count_result: Arc::new(Mutex::new(json!({"input_tokens": 100}))),
        reported_usage: Arc::new(Mutex::new(usage())),
        status,
        broken,
        fault: Arc::default(),
    };
    let data = upstream_state.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .route("/v1/messages", web::post().to(upstream))
            .route("/v1/messages/count_tokens", web::post().to(count_tokens))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "messages-test",
        "anthropic",
        "sk-ant-test-not-a-real-key-1234567890123456789",
        &format!("http://{addr}"),
        vec!["claude-opus-5".into()],
    )];
    mutate(&mut config);
    let state = litellm_rs::server::HttpServer::new(&config)
        .await
        .unwrap()
        .state()
        .clone();
    (state, upstream_state, handle)
}
fn request(stream: bool) -> Value {
    json!({"model":"claude-opus-5","messages":[{"role":"user","content":[{"type":"text","text":"你好"}]}],"system":[{"type":"text","text":"Be helpful","cache_control":{"type":"ephemeral","ttl":"1h"}}],"max_tokens":1024,"stream":stream,"tools":[{"name":"find","description":"Find something","input_schema":{"type":"object","properties":{"query":{"type":"string"}}}}],"thinking":{"type":"adaptive"},"future_input":{"keep":true}})
}

#[tokio::test]
async fn native_messages_json_and_fragmented_sse_preserve_protocol_and_bill_cumulative_usage() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    state.budget_limits.models.set_model_limit(
        "claude-opus-5",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for stream in [false, true] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .insert_header(("x-api-key", "gateway-secret-must-not-reach-upstream"))
                .insert_header(("anthropic-version", "2023-06-01"))
                .insert_header(("anthropic-beta", "test-beta-2026"))
                .set_json(request(stream))
                .to_request(),
        )
        .await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        if stream {
            let wire = String::from_utf8(bytes.to_vec()).unwrap();
            for expected in [
                "event: message_start",
                "event: message_stop",
                "thinking_delta",
                "思考中",
                "opaque-signature",
                "future_event",
            ] {
                assert!(wire.contains(expected), "missing {expected}: {wire}");
            }
            assert!(!wire.contains("event: error"), "{wire}");
        } else {
            assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), output());
        }
    }
    let expected = 40.0 * 0.000005
        + 30.0 * 0.00000625
        + 20.0 * 0.00001
        + 10.0 * 0.0000005
        + 10.0 * 0.000025
        + 0.02;
    let usage = state
        .budget_limits
        .models
        .get_model_usage("claude-opus-5")
        .unwrap();
    assert_eq!(usage.request_count, 2);
    assert!(
        (usage.current_spend - expected * 2.0).abs() < 0.00000001,
        "{}",
        usage.current_spend
    );
    let seen = upstream.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    for (index, (body, key, version, beta)) in seen.iter().enumerate() {
        assert_eq!(*body, request(index == 1));
        assert_eq!(key, "sk-ant-test-not-a-real-key-1234567890123456789");
        assert_eq!(version, "2023-06-01");
        assert!(beta.contains("test-beta-2026"));
    }
    handle.stop(false).await;
}

#[tokio::test]
async fn native_message_stream_error_is_not_retried_or_duplicated() {
    let (state, upstream, handle) = fixture(StatusCode::OK, true, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(request(true))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let wire = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
    assert!(wire.contains("overloaded_error"), "{wire}");
    assert_eq!(wire.matches("event: error").count(), 1, "{wire}");
    assert!(!wire.contains("event: message_stop"));
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn native_messages_validate_auth_body_and_model_before_dispatch() {
    for auth in [false, true] {
        let (state, upstream, handle) = fixture(StatusCode::OK, false, |config| {
            config.gateway.auth.allow_anonymous = !auth;
            config.gateway.auth.enable_api_key = auth;
        })
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut cases = Vec::new();
        if auth {
            cases.push((request(false), StatusCode::UNAUTHORIZED));
        } else {
            for invalid in [json!(-1), json!(1.5), json!("1")] {
                let mut body = request(false);
                body["max_tokens"] = invalid;
                cases.push((body, StatusCode::BAD_REQUEST));
            }
        }
        for (body, expected) in cases {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
            let value: Value = test::read_body_json(response).await;
            assert_eq!(value["type"], "error");
        }
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn messages_http_status_and_retry_after_use_anthropic_error_envelope() {
    for status in [
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::from_u16(529).unwrap(),
    ] {
        let (state, _, handle) = fixture(status, false, |_| {}).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(request(false))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), status);
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert_eq!(response.headers().get("retry-after").unwrap(), "1");
        }
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["type"], "error");
        assert!(value["error"]["type"].is_string());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn messages_input_masking_preserves_opaque_blocks_and_json_errors_are_native() {
    use litellm_rs::core::guardrails::{GuardrailAction, PIIConfig};
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |config| {
        config.gateway.guardrails.pii = Some(PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[MASKED]".into()),
            ..Default::default()
        });
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["messages"][0]["content"][0]["text"] = json!("Contact alice@example.com");
    body["messages"].as_array_mut().unwrap().push(json!({"role":"assistant","content":[{"type":"thinking","thinking":"safe","signature":"opaque-alice@example.com"},{"type":"redacted_thinking","data":"opaque-alice@example.com"}]}));
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(&body)
            .to_request(),
    )
    .await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let sent = upstream.seen.lock().unwrap()[0].0.clone();
    assert_eq!(
        sent["messages"][0]["content"][0]["text"],
        "Contact [MASKED]"
    );
    assert_eq!(sent["messages"][1], body["messages"][1]);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("content-type", "application/json"))
            .set_payload("{bad")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["type"], "error");
    assert_eq!(value["error"]["type"], "invalid_request_error");
    handle.stop(false).await;
}

#[tokio::test]
async fn messages_real_auth_middleware_enforces_key_token_policy_and_uses_provider_credentials() {
    use litellm_rs::core::models::user::types::{User, UserStatus};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |config| {
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = false;
    })
    .await;
    let mut user = User::new(
        "native-messages-test".into(),
        "messages-test@example.test".into(),
        "hashed-password".into(),
    );
    user.status = UserStatus::Active;
    let user = state.storage.db().create_user(&user).await.unwrap();
    let raw_key = "gw-native-messages-test-12345678901234567890";
    let mut metadata = Metadata::new();
    metadata.set_extra("__core_keys",json!({"permissions":{"allowed_models":["claude-opus-5"],"allowed_endpoints":["/v1/messages"],"max_tokens_per_request":128}}));
    let key = ApiKey {
        metadata,
        name: "messages-test".into(),
        key_hash: hash_api_key(raw_key, None),
        key_prefix: extract_api_key_prefix(raw_key),
        user_id: Some(user.id()),
        team_id: None,
        permissions: vec!["api.chat".into()],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };
    state.storage.db().create_api_key(&key).await.unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(Arc::clone(&state.budget_limits)))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(request(false))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["type"], "error");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("x-api-key", raw_key))
            .set_json(request(false))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["type"], "permission_error");
    assert!(upstream.seen.lock().unwrap().is_empty());
    let mut body = request(false);
    body["max_tokens"] = json!(128);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .insert_header(("x-api-key", raw_key))
            .set_json(body)
            .to_request(),
    )
    .await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    assert_ne!(upstream.seen.lock().unwrap()[0].1, raw_key);
    handle.stop(false).await;
}

#[tokio::test]
async fn malformed_native_message_streams_cannot_complete_successfully() {
    for fault in [
        "missing_start",
        "missing_stop",
        "missing_delta",
        "bad_usage",
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
        *upstream.fault.lock().unwrap() = Some(fault);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(request(true))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let wire = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
        assert!(wire.contains("event: error"), "{fault}: {wire}");
        assert!(!wire.contains("event: message_stop"), "{fault}: {wire}");
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_messages_body_limit_keeps_413_and_native_error_type() {
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes_with_body_limit(cfg, 128)
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(request(false))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let value: Value = test::read_body_json(response).await;
    assert_eq!(value["error"]["type"], "request_too_large");
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_counted_media_and_cold_cache_costs_gate_generation() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    for (input_tokens, cache) in [(2000, false), (1000, true)] {
        let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
        *upstream.count_result.lock().unwrap() = json!({"input_tokens": input_tokens});
        state.budget_limits.models.set_model_limit(
            "claude-opus-5",
            ModelLimitConfig::new(0.006, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(false);
        body["max_tokens"] = json!(1);
        body["messages"][0]["content"] = json!([
            {"type":"image","source":{"type":"url","url":"https://example.test/image.png"}},
            {"type":"document","source":{"type":"file","file_id":"file-count-test"}}
        ]);
        if !cache {
            body["system"][0]
                .as_object_mut()
                .unwrap()
                .remove("cache_control");
        }
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(&body)
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::PAYMENT_REQUIRED,
            "{}",
            String::from_utf8_lossy(&test::read_body(response).await)
        );
        assert!(upstream.seen.lock().unwrap().is_empty());
        let counted = upstream.counted.lock().unwrap().clone();
        assert_eq!(counted.len(), 1);
        assert_eq!(counted[0]["messages"], body["messages"]);
        assert_eq!(counted[0]["tools"], body["tools"]);
        assert_eq!(counted[0]["thinking"], body["thinking"]);
        for excluded in ["stream", "max_tokens", "future_input"] {
            assert!(counted[0].get(excluded).is_none());
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn malformed_native_token_count_never_starts_generation() {
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for value in [
        json!({}),
        json!({"input_tokens":-1}),
        json!({"input_tokens":"100"}),
        json!({"input_tokens":4294967296u64}),
    ] {
        *upstream.count_result.lock().unwrap() = value;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(request(false))
                .to_request(),
        )
        .await;
        assert!(response.status().is_server_error());
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    assert_eq!(upstream.counted.lock().unwrap().len(), 4);
    handle.stop(false).await;
}

#[tokio::test]
async fn web_tool_budget_includes_repeated_context_and_search_fees() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    for kind in ["web_search_20260209", "web_fetch_20260209"] {
        let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
        state.budget_limits.models.set_model_limit(
            "claude-opus-5",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(false);
        body["tools"] = json!([{ "type": kind, "name": if kind.starts_with("web_search") {"web_search"} else {"web_fetch"}, "max_uses": 2, "allowed_callers": ["direct"] }]);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn web_tools_require_bounds_and_preserve_bounded_wire_requests() {
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["tools"] = json!([{ "type": "web_search_20260209", "name": "web_search", "allowed_callers": ["direct"] }]);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(&body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(upstream.seen.lock().unwrap().is_empty());
    body["tools"][0]["max_uses"] = json!(3);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(&body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.seen.lock().unwrap()[0].0["tools"], body["tools"]);
    handle.stop(false).await;
}

#[tokio::test]
async fn unsupported_native_billing_modes_fail_before_any_upstream_call() {
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let cases = [
        json!({"speed":"fast"}),
        json!({"inference_geo":"eu"}),
        json!({"mcp_servers":[{"type":"url","url":"https://mcp.example.test","name":"remote"}]}),
        json!({"tools":[{"type":"code_execution_20260120","name":"code_execution"}]}),
        json!({"tools":[{"type":"tool_search_tool_bm25_20251119","name":"tool_search_tool_bm25"}]}),
        json!({"tools":[{"type":"web_search_20260209","name":"web_search","max_uses":1}]}),
        json!({"tools":[{"type":"web_fetch_20260318","name":"web_fetch","max_uses":1}]}),
        json!({"tools":[{"type":"web_search_20250305","name":"web_search","max_uses":1,"allowed_callers":["code_execution_20260120"]}]}),
    ];
    for fields in cases {
        let mut body = request(false);
        body.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/messages")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["error"]["type"], "invalid_request_error");
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    assert!(upstream.counted.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_geo_uses_reported_region_for_json_and_sse_without_rewriting_request() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    for stream in [false, true] {
        for explicit in [false, true] {
            let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
            upstream.reported_usage.lock().unwrap()["inference_geo"] = json!("us");
            state.budget_limits.models.set_model_limit(
                "claude-opus-5",
                ModelLimitConfig::new(1.0, ResetPeriod::Never),
            );
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut body = request(stream);
            if explicit {
                body["inference_geo"] = json!("us");
            }
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .set_json(&body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = test::read_body(response).await;
            if stream {
                let wire = String::from_utf8_lossy(&bytes);
                assert!(wire.contains("event: message_stop"), "{wire}");
                assert!(!wire.contains("event: error"), "{wire}");
            }
            let tokens = 40.0 * 0.000005
                + 30.0 * 0.00000625
                + 20.0 * 0.00001
                + 10.0 * 0.0000005
                + 10.0 * 0.000025;
            let spend = state
                .budget_limits
                .models
                .get_model_usage("claude-opus-5")
                .unwrap();
            assert!((spend.current_spend - (tokens * 1.1 + 0.02)).abs() < 1e-12);
            assert_eq!(upstream.seen.lock().unwrap()[0].0, body);
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn workspace_geo_upper_bound_is_reserved_before_generation() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    let (state, upstream, handle) = fixture(StatusCode::OK, false, |_| {}).await;
    state.budget_limits.models.set_model_limit(
        "claude-opus-5",
        ModelLimitConfig::new(0.00055, ResetPeriod::Never),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["max_tokens"] = json!(1);
    body["system"][0]
        .as_object_mut()
        .unwrap()
        .remove("cache_control");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(&body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(upstream.counted.lock().unwrap().len(), 1);
    assert!(upstream.seen.lock().unwrap().is_empty());
    body["inference_geo"] = json!("global");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/messages")
            .set_json(&body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn geo_capable_models_reject_missing_billing_region_but_haiku45_accepts_it() {
    for (model, valid) in [
        ("claude-opus-5", false),
        ("claude-haiku-4-5-20251001", true),
    ] {
        for stream in [false, true] {
            let (state, upstream, handle) = fixture(StatusCode::OK, false, |config| {
                config.gateway.providers[0].models = vec![model.into()];
            })
            .await;
            upstream
                .reported_usage
                .lock()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("inference_geo");
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut body = request(stream);
            body["model"] = json!(model);
            if valid {
                body.as_object_mut().unwrap().remove("thinking");
                body["system"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("cache_control");
                *upstream.reported_usage.lock().unwrap() =
                    json!({"input_tokens":40,"output_tokens":10});
            }
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/messages")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            if stream {
                let wire = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
                assert_eq!(wire.contains("event: error"), !valid, "{wire}");
                assert_eq!(wire.contains("event: message_stop"), valid, "{wire}");
            } else {
                let status = response.status();
                let body = test::read_body(response).await;
                assert_eq!(
                    status,
                    if valid {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_GATEWAY
                    },
                    "{}",
                    String::from_utf8_lossy(&body)
                );
            }
            handle.stop(false).await;
        }
    }
}
