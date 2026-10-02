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
    status: StatusCode,
    broken: bool,
    fault: Arc<Mutex<Option<&'static str>>>,
}
fn usage() -> Value {
    json!({"input_tokens":40,"output_tokens":10,"cache_creation_input_tokens":50,"cache_read_input_tokens":10,"cache_creation":{"ephemeral_1h_input_tokens":20,"ephemeral_5m_input_tokens":30},"server_tool_use":{"web_search_requests":2,"web_fetch_requests":1}})
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
    if body["stream"] == true {
        let mut first = output();
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
    HttpResponse::Ok().json(output())
}
async fn fixture(
    status: StatusCode,
    broken: bool,
    mutate: impl FnOnce(&mut Config),
) -> (AppState, Upstream, actix_web::dev::ServerHandle) {
    let upstream_state = Upstream {
        seen: Arc::default(),
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
        permissions: vec!["messages".into()],
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
