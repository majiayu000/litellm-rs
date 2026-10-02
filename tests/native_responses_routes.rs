#![cfg(all(feature = "gateway", feature = "storage"))]

#[path = "common/providers.rs"]
pub mod provider_fixtures;

use actix_web::{
    App, HttpMessage, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web,
};
use litellm_rs::{Config, server::state::AppState};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
struct Upstream {
    seen: Arc<Mutex<Vec<Value>>>,
    status: StatusCode,
    output: Value,
    lifecycle_calls: Arc<Mutex<Vec<String>>>,
    background_state: Arc<AtomicUsize>,
}

async fn upstream(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.seen.lock().unwrap().push(body.clone());
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "9"))
            .json(json!({"error":{"message":"capacity unavailable"}}));
    }
    if body["background"] == true {
        data.background_state.store(1, Ordering::SeqCst);
        let mut value = data.output.clone();
        value["status"] = json!("queued");
        value["output"] = json!([]);
        value["usage"] = Value::Null;
        if body["stream"] == true {
            let event = json!({"type":"response.created", "sequence_number":0, "response":value});
            // Simulate an upstream disconnect after accepting background work.
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .body(format!("id: 0\nevent: response.created\ndata: {event}\n\n"));
        }
        return HttpResponse::Ok().json(value);
    }
    if body["stream"] == true {
        let created = json!({"type":"response.created", "response":{"id":data.output["id"],"object":"response","status":"in_progress","output":[]}});
        let delta = json!({"type":"response.output_text.delta","output_index":0,"delta":"你好","future_event_field":{"native":true}});
        let completed = json!({"type":"response.completed", "response":data.output});
        return HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).body(format!("event: response.created\ndata: {created}\n\nevent: response.output_text.delta\ndata: {delta}\n\nevent: response.completed\ndata: {completed}\n\n"));
    }
    HttpResponse::Ok().json(&data.output)
}

async fn upstream_compact(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.seen.lock().unwrap().push(body.clone());
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "9"))
            .json(json!({"error":{"message":"capacity unavailable"}}));
    }
    let mut value = data.output.clone();
    value["id"] = json!("resp_compacted");
    value["object"] = json!("response.compaction");
    value["output"] = json!([{"type":"compaction","id":"cmp_native","encrypted_content":"opaque==","future_compaction_field":true}]);
    value.as_object_mut().unwrap().remove("status");
    if body["invalid_upstream_usage"] == true {
        value["usage"] = Value::Null;
    }
    HttpResponse::Ok().json(value)
}

async fn upstream_lifecycle(data: web::Data<Upstream>, req: HttpRequest) -> HttpResponse {
    data.lifecycle_calls
        .lock()
        .unwrap()
        .push(format!("{} {}", req.method(), req.uri()));
    if req.query_string().contains("stream=true") {
        let event =
            json!({"type":"response.completed", "sequence_number":42, "response":data.output});
        return HttpResponse::Ok()
            .insert_header(("content-type", "text/event-stream"))
            .body(format!(
                "id: 42\nevent: response.completed\ndata: {event}\n\n"
            ));
    }
    if req.path().ends_with("/input_items") {
        return HttpResponse::Ok().json(json!({"object":"list", "data":[{"id":"item_native", "type":"message", "role":"user", "content":[{"type":"input_text","text":"Hello"}], "future_input_field":true}], "has_more":false}));
    }
    if req.method() == actix_web::http::Method::DELETE {
        return HttpResponse::Ok()
            .json(json!({"id":"resp_native", "object":"response.deleted", "deleted":true}));
    }
    if req.path().ends_with("/cancel") {
        data.background_state.store(3, Ordering::SeqCst);
    }
    let mut value = data.output.clone();
    match data.background_state.load(Ordering::SeqCst) {
        1 => {
            value["status"] = json!("queued");
            value["usage"] = Value::Null;
            value["output"] = json!([]);
        }
        3 => {
            value["status"] = json!("cancelled");
        }
        _ => {}
    }
    HttpResponse::Ok().json(value)
}

async fn fixture(
    status: StatusCode,
    mutate: impl FnOnce(&mut Config),
) -> (AppState, Upstream, actix_web::dev::ServerHandle) {
    let upstream_state = Upstream {
        seen: Arc::default(),
        status,
        lifecycle_calls: Arc::default(),
        background_state: Arc::default(),
        output: json!({"id":"resp_native","object":"response","status":"completed","model":"gpt-4o-mini","output":[{"type":"reasoning","encrypted_content":"opaque"},{"type":"message","content":[{"type":"output_text","text":"你好"}]}],"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":1}},"future_response_field":{"preserved":true}}),
    };
    let data = upstream_state.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .route("/v1/responses", web::post().to(upstream))
            .route("/v1/responses/compact", web::post().to(upstream_compact))
            .route("/v1/responses/{id}", web::get().to(upstream_lifecycle))
            .route("/v1/responses/{id}", web::delete().to(upstream_lifecycle))
            .route(
                "/v1/responses/{id}/cancel",
                web::post().to(upstream_lifecycle),
            )
            .route(
                "/v1/responses/{id}/input_items",
                web::get().to(upstream_lifecycle),
            )
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
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.providers = vec![provider_fixtures::mock_provider_config(
        "native-test",
        "openai",
        "sk-test-not-a-real-key-12345678901234567890",
        &format!("http://{address}/v1"),
        vec!["gpt-4o-mini".into()],
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
    json!({"model":"gpt-4o-mini","input":"Hello","stream":stream,"store":false,"tools":[{"type":"web_search_preview"}],"reasoning":{"effort":"low"},"include":["reasoning.encrypted_content"],"future_request_field":{"preserved":true},"max_output_tokens":16})
}

#[tokio::test]
async fn native_json_and_sse_preserve_tools_reasoning_and_extension_fields() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for streaming in [false, true] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(request(streaming))
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
        if streaming {
            let wire = String::from_utf8(bytes.to_vec()).unwrap();
            assert!(wire.contains("event: response.output_text.delta"), "{wire}");
            assert!(wire.contains("你好"), "{wire}");
            assert!(wire.contains("future_event_field"), "{wire}");
            assert!(wire.contains("future_response_field"), "{wire}");
        } else {
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value, upstream.output);
        }
    }
    let seen = upstream.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], request(false));
    assert_eq!(seen[1], request(true));
    handle.stop(false).await;
}

#[tokio::test]
async fn native_auth_fails_before_upstream() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.auth.allow_anonymous = false;
        config.gateway.auth.enable_api_key = true;
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for path in ["/v1/responses", "/v1/responses/compact"] {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(path)
                .set_json(request(false))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_rate_limit_keeps_status_and_retry_after() {
    let (state, upstream, handle) = fixture(StatusCode::TOO_MANY_REQUESTS, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(request(false))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers().get("retry-after").unwrap(), "9");
    assert!(!upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_invalid_token_limit_fails_before_upstream() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for invalid in [json!(-1), json!(1.5), json!("16"), json!(u64::MAX)] {
        let mut body = request(false);
        body["max_output_tokens"] = invalid;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_extension_fields_are_masked_before_dispatch() {
    use litellm_rs::core::guardrails::{GuardrailAction, PIIConfig};
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
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
    body["future_request_field"] = json!({"text":"Contact alice@example.com"});
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
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
    let seen = upstream.seen.lock().unwrap().clone();
    assert_eq!(seen[0]["future_request_field"]["text"], "Contact [MASKED]");
    handle.stop(false).await;
}

#[tokio::test]
async fn responses_only_model_uses_native_endpoint() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.providers[0].models = vec!["gpt-5.5-pro".into()];
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["model"] = json!("gpt-5.5-pro");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
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
    handle.stop(false).await;
}

#[tokio::test]
async fn chat_only_model_does_not_fall_back_from_native_responses() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.providers[0].models = vec!["gpt-audio-1.5".into()];
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["model"] = json!("gpt-audio-1.5");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(body)
            .to_request(),
    )
    .await;
    assert!(!response.status().is_success());
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_stored_json_and_sse_are_owner_scoped_across_gateways_and_account_changes() {
    use litellm_rs::core::types::context::RequestContext;
    for streaming in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
            config.gateway.storage.database.enabled = true;
            config.gateway.storage.database.auto_migrate = true;
            config.gateway.storage.database.url = format!(
                "sqlite://{}?mode=rwc",
                dir.path().join("native.db").display()
            );
        })
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(streaming);
        body.as_object_mut().unwrap().remove("store");
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(body)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let wire = test::read_body(response).await;
        assert!(String::from_utf8_lossy(&wire).contains("future_response_field"));
        drop(app);
        let mut second_config = state.config().as_ref().clone();
        let mut unrelated = second_config.gateway.providers[0].clone();
        unrelated.name = "other-native".into();
        unrelated.base_url = unrelated
            .base_url
            .map(|base| format!("{base}/wrong-account"));
        unrelated.priority = 0;
        second_config.gateway.providers[0].priority = 10;
        second_config.gateway.providers.push(unrelated);
        let second = litellm_rs::server::HttpServer::new(&second_config)
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(second.state().clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/v1/responses/resp_native")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("bob"));
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::NOT_FOUND
        );
        assert!(upstream.lifecycle_calls.lock().unwrap().is_empty());
        let req = test::TestRequest::get()
            .uri("/v1/responses/resp_native?include%5B%5D=reasoning.encrypted_content")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value, upstream.output);
        assert!(
            upstream.lifecycle_calls.lock().unwrap()[0]
                .contains("include%5B%5D=reasoning.encrypted_content")
        );
        let req = test::TestRequest::get()
            .uri("/v1/responses/resp_native/input_items?limit=1&order=asc")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["data"][0]["future_input_field"], true);

        let mut continuation = request(false);
        continuation["previous_response_id"] = json!("resp_native");
        continuation["input"] = json!("Continue on the original account");
        let before_posts = upstream.seen.lock().unwrap().len();
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(&continuation)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("bob"));
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(upstream.seen.lock().unwrap().len(), before_posts);
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(&continuation)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        let status = response.status();
        let value = test::read_body(response).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&value)
        );
        assert_eq!(
            upstream.seen.lock().unwrap().last().unwrap()["previous_response_id"],
            "resp_native"
        );

        let mut changed = state.config().as_ref().clone();
        changed.gateway.providers[0].api_key =
            "sk-another-test-account-12345678901234567890".into();
        let changed = litellm_rs::server::HttpServer::new(&changed).await.unwrap();
        let changed_app = test::init_service(
            App::new()
                .app_data(web::Data::new(changed.state().clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let before = upstream.lifecycle_calls.lock().unwrap().len();
        let req = test::TestRequest::get()
            .uri("/v1/responses/resp_native")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        assert_eq!(
            test::call_service(&changed_app, req).await.status(),
            StatusCode::CONFLICT
        );
        assert_eq!(upstream.lifecycle_calls.lock().unwrap().len(), before);

        let req = test::TestRequest::delete()
            .uri("/v1/responses/resp_native")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["object"], "response.deleted");
        let before = upstream.lifecycle_calls.lock().unwrap().len();
        let req = test::TestRequest::get()
            .uri("/v1/responses/resp_native")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(upstream.lifecycle_calls.lock().unwrap().len(), before);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_storage_requires_owner_and_store_false_never_creates_a_handle() {
    use litellm_rs::core::types::context::RequestContext;
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut stored = request(false);
    stored.as_object_mut().unwrap().remove("store");
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(stored)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(upstream.seen.lock().unwrap().is_empty());
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(request(false))
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let req = test::TestRequest::get()
        .uri("/v1/responses/resp_native")
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
    assert!(upstream.lifecycle_calls.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn previous_response_reserves_retained_tokens_and_rejects_expired_handles() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    use litellm_rs::core::types::context::RequestContext;
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["store"] = json!(true);
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let now = chrono::Utc::now().timestamp();
    let db = &state.storage.database;
    let mut record = db
        .owned_response("resp_native", "user:alice", now)
        .await
        .unwrap()
        .unwrap();
    // Large retained usage can include hidden reasoning, so JSON length alone
    // cannot reserve a continuation's actual input cost.
    let mut value: Value = serde_json::from_str(&record.response_json).unwrap();
    value["usage"]["input_tokens"] = json!(199997);
    value["usage"]["total_tokens"] = json!(200000);
    record.response_json = value.to_string();
    db.delete_owned_response(&record.id, &record.owner, now)
        .await
        .unwrap();
    db.insert_response(record.clone(), now).await.unwrap();
    state.budget_limits.models.set_model_limit(
        "gpt-4o-mini",
        ModelLimitConfig::new(0.001, ResetPeriod::Monthly),
    );
    body["store"] = json!(false);
    body["previous_response_id"] = json!("resp_native");
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    assert_eq!(
        status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    db.delete_owned_response(&record.id, &record.owner, now)
        .await
        .unwrap();
    record.expires_at = now;
    db.insert_response(record, now).await.unwrap();
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn previous_response_cannot_bypass_new_guardrails_for_stored_context() {
    use litellm_rs::core::guardrails::{GuardrailAction, PIIConfig};
    use litellm_rs::core::types::context::RequestContext;
    let dir = tempfile::tempdir().unwrap();
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.storage.database.enabled = true;
        config.gateway.storage.database.auto_migrate = true;
        config.gateway.storage.database.url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("guardrails.db").display()
        );
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["store"] = json!(true);
    body["input"] = json!("Contact alice@example.com");
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let mut config = state.config().as_ref().clone();
    config.gateway.guardrails.pii = Some(PIIConfig {
        enabled: true,
        action: GuardrailAction::Mask,
        mask_pattern: Some("[MASKED]".into()),
        ..Default::default()
    });
    let changed = litellm_rs::server::HttpServer::new(&config).await.unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(changed.state().clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    body["store"] = json!(false);
    body["input"] = json!("Continue");
    body["previous_response_id"] = json!("resp_native");
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let value: Value = test::read_body_json(response).await;
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot be masked")
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn native_background_settles_once_after_cross_gateway_reads_and_cancel() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    use litellm_rs::core::types::context::RequestContext;
    for cancel in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
            config.gateway.storage.database.enabled = true;
            config.gateway.storage.database.auto_migrate = true;
            config.gateway.storage.database.url = format!(
                "sqlite://{}?mode=rwc",
                dir.path().join("background.db").display()
            );
        })
        .await;
        state.budget_limits.models.set_model_limit(
            "gpt-4o-mini",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(false);
        body["background"] = json!(true);
        body["store"] = json!(true);
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(&body)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["status"], "queued");
        let reserved = state
            .budget_limits
            .models
            .get_model_usage("gpt-4o-mini")
            .unwrap();
        assert!(reserved.current_spend > 0.0);
        assert_eq!(reserved.request_count, 0);
        // Closing the creating HTTP service does not stop the background settlement.
        drop(app);
        let second = litellm_rs::server::HttpServer::new(state.config().as_ref())
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(second.state().clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let req = test::TestRequest::post()
            .uri("/v1/responses/resp_native/cancel")
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("bob"));
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::NOT_FOUND
        );
        for _ in 0..3 {
            let req = test::TestRequest::get()
                .uri("/v1/responses/resp_native")
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let value: Value = test::call_and_read_body_json(&app, req).await;
            assert_eq!(value["status"], "queued");
        }
        assert_eq!(
            state
                .budget_limits
                .models
                .get_model_usage("gpt-4o-mini")
                .unwrap()
                .request_count,
            0
        );
        if cancel {
            let req = test::TestRequest::post()
                .uri("/v1/responses/resp_native/cancel")
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::OK);
            let value: Value = test::read_body_json(response).await;
            assert_eq!(value["status"], "cancelled");
        } else {
            upstream.background_state.store(2, Ordering::SeqCst);
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if state
                    .budget_limits
                    .models
                    .get_model_usage("gpt-4o-mini")
                    .unwrap()
                    .request_count
                    == 1
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let settled = state
            .budget_limits
            .models
            .get_model_usage("gpt-4o-mini")
            .unwrap();
        assert!(settled.current_spend > 0.0 && settled.current_spend < reserved.current_spend);
        for _ in 0..3 {
            let req = test::TestRequest::get()
                .uri("/v1/responses/resp_native")
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let value: Value = test::call_and_read_body_json(&app, req).await;
            assert_eq!(
                value["status"],
                if cancel { "cancelled" } else { "completed" }
            );
        }
        assert_eq!(
            state
                .budget_limits
                .models
                .get_model_usage("gpt-4o-mini")
                .unwrap()
                .request_count,
            1
        );
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_background_store_false_keeps_only_temporary_handle_metadata() {
    use litellm_rs::core::types::context::RequestContext;
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["background"] = json!(true);
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let now = chrono::Utc::now().timestamp();
    let record = state
        .storage
        .database
        .owned_response("resp_native", "user:alice", now)
        .await
        .unwrap()
        .unwrap();
    assert!(record.background);
    assert!(record.expires_at <= now + 600);
    assert_eq!(
        serde_json::from_str::<Value>(&record.input_json).unwrap(),
        json!({"store":false,"stream":false})
    );
    assert!(!record.response_json.contains("Hello"));
    assert!(!record.response_json.contains("opaque"));
    upstream.background_state.store(2, Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let record = state
                .storage
                .database
                .owned_response("resp_native", "user:alice", now)
                .await
                .unwrap()
                .unwrap();
            if record.status == "completed" {
                assert!(!record.response_json.contains("opaque"));
                assert!(!record.response_json.contains("你好"));
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    handle.stop(false).await;
}

#[tokio::test]
async fn background_stream_reconnect_preserves_cursor_and_never_rebills() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    use litellm_rs::core::types::context::RequestContext;
    for store in [true, false] {
        let dir = tempfile::tempdir().unwrap();
        let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
            config.gateway.storage.database.enabled = true;
            config.gateway.storage.database.auto_migrate = true;
            config.gateway.storage.database.url = format!(
                "sqlite://{}?mode=rwc",
                dir.path().join("resume.db").display()
            );
        })
        .await;
        state.budget_limits.models.set_model_limit(
            "gpt-4o-mini",
            ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(true);
        body["background"] = json!(true);
        body["store"] = json!(store);
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(body)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("alice"));
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::OK);
        let wire = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
        assert!(wire.contains("id: 0\nevent: response.created"), "{wire}");
        assert_eq!(
            state
                .budget_limits
                .models
                .get_model_usage("gpt-4o-mini")
                .unwrap()
                .request_count,
            0
        );
        drop(app);
        let second = litellm_rs::server::HttpServer::new(state.config().as_ref())
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(second.state().clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let uri = "/v1/responses/resp_native?stream=true&starting_after=41";
        let req = test::TestRequest::get().uri(uri).to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id("bob"));
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::NOT_FOUND
        );
        upstream.background_state.store(2, Ordering::SeqCst);
        for _ in 0..3 {
            let req = test::TestRequest::get().uri(uri).to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("content-type").unwrap(),
                "text/event-stream"
            );
            let wire = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();
            assert!(wire.contains("id: 42\nevent: response.completed"), "{wire}");
            assert!(wire.contains("future_response_field"), "{wire}");
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                if state
                    .budget_limits
                    .models
                    .get_model_usage("gpt-4o-mini")
                    .unwrap()
                    .request_count
                    == 1
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        assert_eq!(
            upstream
                .lifecycle_calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| call.contains("stream=true&starting_after=41"))
                .count(),
            3
        );
        assert_eq!(
            state
                .budget_limits
                .models
                .get_model_usage("gpt-4o-mini")
                .unwrap()
                .request_count,
            1
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn compact_preserves_encrypted_items_and_settles_once_without_storage() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    state.budget_limits.models.set_model_limit(
        "gpt-4o-mini",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let body = json!({"model":"gpt-4o-mini","input":[{"type":"compaction","id":"cmp_old","encrypted_content":"opaque-old=="},{"role":"user","content":"Hello"}],"instructions":"Keep facts","future_field":{"preserve":true}});
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses/compact")
            .set_json(&body)
            .to_request(),
    )
    .await;
    let status = response.status();
    let value: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["object"], "response.compaction");
    assert_eq!(value["output"][0]["encrypted_content"], "opaque==");
    assert_eq!(value["output"][0]["future_compaction_field"], true);
    assert!(value.get("status").is_none());
    assert_eq!(upstream.seen.lock().unwrap().as_slice(), &[body]);
    assert!(upstream.lifecycle_calls.lock().unwrap().is_empty());
    let expected = 8.0 * 0.00000015 + 4.0 * 0.000000075 + 3.0 * 0.0000006;
    let spend = state
        .budget_limits
        .models
        .get_model_usage("gpt-4o-mini")
        .unwrap();
    assert!((spend.current_spend - expected).abs() < 1e-12, "{spend:?}");
    handle.stop(false).await;
}

#[tokio::test]
async fn compact_rejects_streaming_and_unenforceable_token_limits_before_dispatch() {
    use litellm_rs::core::types::context::RequestContext;
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for field in ["stream", "background", "store", "max_output_tokens"] {
        let mut body = json!({"model":"gpt-4o-mini","input":"Hello"});
        body[field] = json!(true);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses/compact")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{field}");
    }
    let req = test::TestRequest::post()
        .uri("/v1/responses/compact")
        .set_json(json!({"model":"gpt-4o-mini","input":"Hello"}))
        .to_request();
    let mut context = RequestContext::new();
    context.set_api_key_max_tokens_per_request(8);
    req.extensions_mut().insert(context);
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn compact_preserves_rate_limit_and_rejects_malformed_success() {
    for status in [StatusCode::OK, StatusCode::TOO_MANY_REQUESTS] {
        let (state, upstream, handle) = fixture(status, |_| {}).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses/compact")
                .set_json(
                    json!({"model":"gpt-4o-mini","input":"Hello","invalid_upstream_usage":true}),
                )
                .to_request(),
        )
        .await;
        assert_eq!(
            response.status(),
            if status == StatusCode::OK {
                StatusCode::BAD_GATEWAY
            } else {
                status
            }
        );
        if status == StatusCode::TOO_MANY_REQUESTS {
            assert_eq!(response.headers().get("retry-after").unwrap(), "9");
        }
        if status == StatusCode::OK {
            assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        } else {
            // Pre-response HTTP rate limits keep the router's configured retries.
            assert!(!upstream.seen.lock().unwrap().is_empty());
        }
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn compact_previous_response_is_owner_bound_and_does_not_create_a_handle() {
    use litellm_rs::core::types::context::RequestContext;
    let dir = tempfile::tempdir().unwrap();
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.storage.database.enabled = true;
        config.gateway.storage.database.auto_migrate = true;
        config.gateway.storage.database.url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("compact.db").display()
        );
    })
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["store"] = json!(true);
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _: Value = test::read_body_json(response).await;
    for (owner, expected) in [("bob", StatusCode::NOT_FOUND), ("alice", StatusCode::OK)] {
        let req = test::TestRequest::post().uri("/v1/responses/compact").set_json(json!({"model":"gpt-4o-mini","previous_response_id":"resp_native","input":"Keep going"})).to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_user_id(owner));
        let response = test::call_service(&app, req).await;
        let status = response.status();
        let value: Value = test::read_body_json(response).await;
        assert_eq!(status, expected, "{owner}: {value}");
    }
    let now = chrono::Utc::now().timestamp();
    assert!(
        state
            .storage
            .database
            .owned_response("resp_compacted", "user:alice", now)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        state
            .storage
            .database
            .owned_response("resp_native", "user:alice", now)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(upstream.seen.lock().unwrap().len(), 2);
    handle.stop(false).await;
}
