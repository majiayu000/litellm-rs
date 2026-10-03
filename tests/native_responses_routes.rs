#![cfg(all(feature = "gateway", feature = "storage"))]

#[path = "common/providers.rs"]
pub mod provider_fixtures;

use actix_web::{App, HttpResponse, HttpServer, http::StatusCode, test, web};
use litellm_rs::{Config, server::state::AppState};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Upstream {
    seen: Arc<Mutex<Vec<Value>>>,
    status: StatusCode,
    count_seen: Arc<Mutex<Vec<Value>>>,
    count_output: Arc<Mutex<Value>>,
    output: Arc<Mutex<Value>>,
}

async fn upstream(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.seen.lock().unwrap().push(body.clone());
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "9"))
            .json(json!({"error":{"message":"capacity unavailable"}}));
    }
    let output = data.output.lock().unwrap().clone();
    if body["stream"] == true {
        if output["test_midstream_error"] == true {
            return HttpResponse::Ok().insert_header(("content-type", "text/event-stream"))
                .body("event: error\ndata: {\"type\":\"error\",\"message\":\"provider interrupted\"}\n\n");
        }
        let delta = json!({"type":"response.output_text.delta","output_index":0,"delta":"你好","future_event_field":{"native":true}});
        let completed = json!({"type":format!("response.{}", output["status"].as_str().unwrap_or("completed")), "response":output});
        return HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).body(format!("event: response.output_text.delta\ndata: {delta}\n\nevent: response.completed\ndata: {completed}\n\n"));
    }
    HttpResponse::Ok().json(&output)
}

async fn count_upstream(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.count_seen.lock().unwrap().push(body.into_inner());
    HttpResponse::build(data.status)
        .insert_header(("retry-after", "9"))
        .json(data.count_output.lock().unwrap().clone())
}

async fn fixture(
    status: StatusCode,
    mutate: impl FnOnce(&mut Config),
) -> (AppState, Upstream, actix_web::dev::ServerHandle) {
    let upstream_state = Upstream {
        seen: Arc::default(),
        count_seen: Arc::default(),
        count_output: Arc::new(Mutex::new(
            json!({"object":"response.input_tokens","input_tokens":12000}),
        )),
        status,
        output: Arc::new(Mutex::new(
            json!({"id":"resp_native","object":"response","status":"completed","model":"gpt-4o-mini","output":[{"type":"reasoning","encrypted_content":"opaque"},{"type":"message","content":[{"type":"output_text","text":"你好"}]}],"usage":{"input_tokens":12,"output_tokens":3,"total_tokens":15,"input_tokens_details":{"cached_tokens":4},"output_tokens_details":{"reasoning_tokens":1}},"future_response_field":{"preserved":true}}),
        )),
    };
    let data = upstream_state.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .route("/v1/responses", web::post().to(upstream))
            .route("/v1/responses/input_tokens", web::post().to(count_upstream))
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
    json!({"model":"gpt-4o-mini","input":"Hello","stream":stream,"store":false,"service_tier":"default","tools":[{"type":"function","name":"weather","parameters":{"type":"object","properties":{}}},{"type":"custom","name":"calculator"}],"reasoning":{"effort":"low"},"include":["reasoning.encrypted_content"],"future_request_field":{"preserved":true},"max_output_tokens":16})
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
            assert_eq!(value, *upstream.output.lock().unwrap());
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
async fn native_unpriced_hosted_tools_and_mutable_inputs_never_reach_upstream() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut cases = Vec::new();
    for kind in [
        "web_search",
        "web_search_preview",
        "file_search",
        "code_interpreter",
        "image_generation",
        "shell",
        "mcp",
        "computer_use_preview",
        "future_hosted_tool",
    ] {
        let mut body = request(false);
        body["tools"] = json!([{"type":kind}]);
        cases.push(body);
    }
    for field in [
        "prompt",
        "conversation",
        "container",
        "context_management",
        "service_tier",
    ] {
        let mut body = request(false);
        body[field] = json!({"id":"opaque"});
        cases.push(body);
    }
    for part in [
        json!({"type":"input_image","image_url":"https://example.test/image.png"}),
        json!({"type":"input_file","file_url":"https://example.test/paper.pdf"}),
        json!({"type":"input_audio","data":"opaque"}),
    ] {
        let mut body = request(false);
        body["input"] = json!([{"role":"user","content":[part]}]);
        cases.push(body);
    }
    for body in cases {
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(&body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
    }
    assert!(upstream.seen.lock().unwrap().is_empty());
    assert!(upstream.count_seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_image_and_pdf_count_processed_input_and_preserve_generation_wire() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    for (index, part) in [json!({"type":"input_image","image_url":"data:image/png;base64,aGVsbG8=","detail":"high"}),
        json!({"type":"input_image","file_id":"file-image"}),
        json!({"type":"input_file","file_data":"data:application/pdf;base64,aGVsbG8=","filename":"paper.pdf"}),
        json!({"type":"input_file","file_id":"file-pdf"})].into_iter().enumerate() {
        let mut body = request(index % 2 == 0); body["input"] = json!([{"role":"user","content":[part]}]);
        let response = test::call_service(&app, test::TestRequest::post().uri("/v1/responses").set_json(&body).to_request()).await;
        let status = response.status(); let bytes = test::read_body(response).await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&bytes));
        assert_eq!(upstream.seen.lock().unwrap()[index], body);
        let count = &upstream.count_seen.lock().unwrap()[index];
        assert_eq!(count["input"], body["input"]); assert_eq!(count["tools"], body["tools"]);
        assert!(count.get("stream").is_none()); assert!(count.get("max_output_tokens").is_none());
    }
    handle.stop(false).await;
}

#[tokio::test]
async fn native_processed_token_count_enforces_budget_before_generation() {
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    state.budget_limits.providers.set_provider_limit(
        "native-test",
        ProviderLimitConfig::new(0.001, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["input"] =
        json!([{"role":"user","content":[{"type":"input_file","file_id":"large-pdf"}]}]);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(body)
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn native_bad_or_failed_input_counts_never_execute_generation() {
    for status in [
        StatusCode::OK,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        let (state, upstream, handle) = fixture(status, |_| {}).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        for (index, count) in [
            json!({}),
            json!({"object":"response.input_tokens","input_tokens":-1}),
            json!({"object":"response.input_tokens","input_tokens":1.5}),
            json!({"object":"response.input_tokens","input_tokens":u64::MAX}),
        ]
        .into_iter()
        .enumerate()
        {
            if status != StatusCode::OK && index > 0 {
                break;
            }
            *upstream.count_output.lock().unwrap() = count;
            let mut body = request(false);
            body["input"] =
                json!([{"role":"user","content":[{"type":"input_file","file_id":"file-pdf"}]}]);
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert!(!response.status().is_success());
            if status == StatusCode::TOO_MANY_REQUESTS {
                assert_eq!(response.status(), status);
                assert_eq!(response.headers().get("retry-after").unwrap(), "9");
            }
        }
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_counted_input_settles_terminal_usage_and_retains_unknown_budget() {
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    for (streaming, status, missing) in [
        (false, "completed", false),
        (true, "completed", false),
        (false, "incomplete", false),
        (true, "incomplete", false),
        (false, "failed", false),
        (true, "failed", false),
        (false, "completed", true),
        (true, "completed", true),
        (true, "error", true),
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
        state.budget_limits.providers.set_provider_limit(
            "native-test",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        let limits = state.budget_limits.clone();
        {
            let mut output = upstream.output.lock().unwrap();
            output["status"] = json!(status);
            if missing {
                output["usage"] = Value::Null;
            }
            if status == "error" {
                output["test_midstream_error"] = json!(true);
            }
        }
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(streaming);
        body["input"] =
            json!([{"role":"user","content":[{"type":"input_file","file_id":"file-pdf"}]}]);
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = test::read_body(response).await;
        if status == "error" {
            assert!(String::from_utf8_lossy(&bytes).contains("provider interrupted"));
        }
        let spend = limits
            .providers
            .get_provider_usage("native-test")
            .unwrap()
            .current_spend;
        // GPT-4o-mini: 8 uncached + 4 cached input tokens and 3 output tokens.
        let expected = if missing {
            12000.0 * 0.00000015 + 16.0 * 0.0000006
        } else {
            8.0 * 0.00000015 + 4.0 * 0.000000075 + 3.0 * 0.0000006
        };
        assert!(
            (spend - expected).abs() < 0.000000001,
            "stream={streaming} status={status} missing={missing} spend={spend} expected={expected}"
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_missing_usage_never_records_reserved_cost_as_actual_key_usage() {
    use actix_web::HttpMessage;
    use litellm_rs::core::{
        budget::{ProviderLimitConfig, ResetPeriod},
        keys::CreateKeyConfig,
        types::context::RequestContext,
    };
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    state.budget_limits.providers.set_provider_limit(
        "native-test",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let keys = state.key_manager.clone();
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "unknown-responses-usage".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    upstream.output.lock().unwrap()["usage"] = Value::Null;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(request(false))
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_api_key(key_id));
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let usage = keys.get_usage_stats(key_id).await.unwrap();
    assert_eq!(usage.total_requests, 1);
    assert_eq!(usage.unpriced_requests, 1);
    assert_eq!(usage.total_tokens, 0);
    assert_eq!(usage.total_cost, 0.0);
    handle.stop(false).await;
}
