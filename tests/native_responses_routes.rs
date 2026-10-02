#![cfg(all(feature = "gateway", feature = "storage"))]

#[path = "common/providers.rs"]
pub mod provider_fixtures;

use actix_web::{
    App, HttpMessage, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web,
};
use litellm_rs::{Config, server::state::AppState};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Upstream {
    seen: Arc<Mutex<Vec<Value>>>,
    status: StatusCode,
    output: Value,
    lifecycle_calls: Arc<Mutex<Vec<String>>>,
}

async fn upstream(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.seen.lock().unwrap().push(body.clone());
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "9"))
            .json(json!({"error":{"message":"capacity unavailable"}}));
    }
    if body["stream"] == true {
        let created = json!({"type":"response.created", "response":{"id":data.output["id"],"object":"response","status":"in_progress","output":[]}});
        let delta = json!({"type":"response.output_text.delta","output_index":0,"delta":"你好","future_event_field":{"native":true}});
        let completed = json!({"type":"response.completed", "response":data.output});
        return HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).body(format!("event: response.created\ndata: {created}\n\nevent: response.output_text.delta\ndata: {delta}\n\nevent: response.completed\ndata: {completed}\n\n"));
    }
    HttpResponse::Ok().json(&data.output)
}

async fn upstream_lifecycle(data: web::Data<Upstream>, req: HttpRequest) -> HttpResponse {
    data.lifecycle_calls
        .lock()
        .unwrap()
        .push(format!("{} {}", req.method(), req.uri()));
    if req.path().ends_with("/input_items") {
        return HttpResponse::Ok().json(json!({"object":"list", "data":[{"id":"item_native", "type":"message", "role":"user", "content":[{"type":"input_text","text":"Hello"}], "future_input_field":true}], "has_more":false}));
    }
    if req.method() == actix_web::http::Method::DELETE {
        return HttpResponse::Ok()
            .json(json!({"id":"resp_native", "object":"response.deleted", "deleted":true}));
    }
    HttpResponse::Ok().json(&data.output)
}

async fn fixture(
    status: StatusCode,
    mutate: impl FnOnce(&mut Config),
) -> (AppState, Upstream, actix_web::dev::ServerHandle) {
    let upstream_state = Upstream {
        seen: Arc::default(),
        status,
        lifecycle_calls: Arc::default(),
        output: json!({"id":"resp_native","object":"response","status":"completed","model":"gpt-4o-mini","output":[{"type":"reasoning","encrypted_content":"opaque"},{"type":"message","content":[{"type":"output_text","text":"你好"}]}],"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":1}},"future_response_field":{"preserved":true}}),
    };
    let data = upstream_state.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .route("/v1/responses", web::post().to(upstream))
            .route("/v1/responses/{id}", web::get().to(upstream_lifecycle))
            .route("/v1/responses/{id}", web::delete().to(upstream_lifecycle))
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
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(request(false))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
        let second = litellm_rs::server::HttpServer::new(&state.config())
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
