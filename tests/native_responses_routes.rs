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
    output: Arc<Mutex<Value>>,
    count_seen: Arc<Mutex<Vec<Value>>>,
    count_output: Arc<Mutex<Value>>,
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
        let mut value = data.output.lock().unwrap().clone();
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
        if data.output.lock().unwrap()["test_pause_before_output"] == true {
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(futures::stream::pending::<
                    Result<bytes::Bytes, std::io::Error>,
                >());
        }
        if data.output.lock().unwrap()["test_stall_stream"] == true {
            use futures::StreamExt;
            let created = json!({"type":"response.created", "response":{"id":"resp_native","object":"response","status":"in_progress","output":[]}});
            let mut wire = format!("event: response.created\ndata: {created}\n\n");
            if data.output.lock().unwrap()["test_output_before_stall"] == true {
                // Cross the default 256-character output inspection window
                // before stalling so the enabled guard can release safe output.
                let delta = json!({"type":"response.output_text.delta","output_index":0,"delta":"generated".repeat(32)});
                wire.push_str(&format!(
                    "event: response.output_text.delta\ndata: {delta}\n\n"
                ));
            }
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(
                    futures::stream::once(async move {
                        Ok::<_, std::io::Error>(bytes::Bytes::from(wire))
                    })
                    .chain(futures::stream::pending()),
                );
        }
        if data.output.lock().unwrap()["test_midstream_error"] == true {
            return HttpResponse::Ok().insert_header(("content-type", "text/event-stream"))
                .body("event: error\ndata: {\"type\":\"error\",\"message\":\"provider interrupted\"}\n\n");
        }
        let created = json!({"type":"response.created", "response":{"id":data.output.lock().unwrap()["id"],"object":"response","status":"in_progress","output":[]}});
        let delta = json!({"type":"response.output_text.delta","output_index":0,"delta":"你好","future_event_field":{"native":true}});
        if data.output.lock().unwrap()["test_pause_after_output"] == true {
            use futures::StreamExt;
            // This stalled fixture must pass a real default guard inspection.
            // Ordinary completed responses keep their original short delta.
            let mut delta = delta;
            delta["delta"] = json!("你好".repeat(130));
            let prefix = bytes::Bytes::from(format!(
                "event: response.created\ndata: {created}\n\nevent: response.output_text.delta\ndata: {delta}\n\n"
            ));
            let output = data.output.lock().unwrap().clone();
            let stream = futures::stream::once(async move { Ok::<_, std::io::Error>(prefix) })
                .chain(futures::stream::once(async move {
                    if output["test_complete_after_pause"] != true {
                        std::future::pending::<()>().await;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    let completed = json!({"type":"response.completed", "response":output});
                    Ok(bytes::Bytes::from(format!(
                        "event: response.completed\ndata: {completed}\n\n"
                    )))
                }));
            return HttpResponse::Ok()
                .insert_header(("content-type", "text/event-stream"))
                .streaming(stream);
        }
        let output = data.output.lock().unwrap().clone();
        let completed = json!({"type":format!("response.{}", output["status"].as_str().unwrap_or("completed")), "response":output});
        return HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).body(format!("event: response.created\ndata: {created}\n\nevent: response.output_text.delta\ndata: {delta}\n\nevent: response.completed\ndata: {completed}\n\n"));
    }
    HttpResponse::Ok().json(data.output.lock().unwrap().clone())
}

async fn upstream_compact(data: web::Data<Upstream>, body: web::Json<Value>) -> HttpResponse {
    data.seen.lock().unwrap().push(body.clone());
    if data.status != StatusCode::OK {
        return HttpResponse::build(data.status)
            .insert_header(("retry-after", "9"))
            .json(json!({"error":{"message":"capacity unavailable"}}));
    }
    let mut value = data.output.lock().unwrap().clone();
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
        let event = json!({"type":"response.completed", "sequence_number":42, "response":data.output.lock().unwrap().clone()});
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
    let mut value = data.output.lock().unwrap().clone();
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
        lifecycle_calls: Arc::default(),
        background_state: Arc::default(),
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
            .route("/v1/responses/compact", web::post().to(upstream_compact))
            .route("/v1/responses/input_tokens", web::post().to(count_upstream))
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
    json!({"model":"gpt-4o-mini","input":"Hello","stream":stream,"store":false,"service_tier":"default","tools":[{"type":"function","name":"weather","parameters":{"type":"object","properties":{}}},{"type":"custom","name":"calculator"}],"reasoning":{"effort":"low"},"include":["reasoning.encrypted_content"],"future_request_field":{"preserved":true},"max_output_tokens":16})
}

#[tokio::test]
async fn native_foreground_unknown_usage_retains_tpm_and_records_success_once() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    for (usage, limit, blocked) in [
        (None, 128, true),
        (Some(0), 128, false),
        (Some(15), 128, false),
        (Some(15), 100, true),
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
            config.gateway.providers[0].tpm = limit;
            config.gateway.providers[0].rpm = 100;
            config.gateway.providers[0].max_concurrent_requests = 100;
            config.gateway.router.load_balancer.health_check_enabled = false;
        })
        .await;
        match usage {
            None => upstream.output.lock().unwrap()["usage"] = Value::Null,
            Some(0) => {
                upstream.output.lock().unwrap()["usage"] =
                    json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
            }
            Some(15) => {}
            _ => unreachable!(),
        }
        let router = state.unified_router();
        let ids = router.get_deployments_for_model("gpt-4o-mini");
        let deployment = router.get_deployment(&ids[0]).unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let second = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            % 60;
        // Quota rejection follows the production retry delays. Keep the
        // request and its follow-up within the same admission minute.
        if second >= 50 {
            tokio::time::sleep(Duration::from_secs(61 - second)).await;
        }
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(json!({"model":"gpt-4o-mini","input":"Hello","stream":false,"store":false,"max_output_tokens":64}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["status"], "completed");
        assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(
            deployment.state.tpm_current.load(Ordering::Relaxed),
            usage.unwrap_or(0)
        );
        assert!(deployment.is_healthy());
        let response = tokio::time::timeout(
            Duration::from_secs(if blocked { 1 } else { 5 }),
            test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(json!({"model":"gpt-4o-mini","input":"Hello","stream":false,"store":false,"max_output_tokens":64}))
                    .to_request(),
            ),
        )
        .await;
        if blocked {
            // Quota denial waits for the existing 60s selection retry. Cancel
            // before the window rolls over and prove no second dispatch occurs.
            assert!(response.is_err(), "usage={usage:?}, limit={limit}");
        } else {
            let response = response.expect("known usage must leave the next request admissible");
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "usage={usage:?}, limit={limit}"
            );
            test::read_body(response).await;
        }
        assert_eq!(
            upstream.seen.lock().unwrap().len(),
            if blocked { 1 } else { 2 }
        );
        assert_eq!(
            deployment.state.success_requests.load(Ordering::Relaxed),
            if blocked { 1 } else { 2 }
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_foreground_failed_usage_preserves_terminal_failure_accounting() {
    for usage in [None, Some(0), Some(15)] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
            config.gateway.providers[0].tpm = 128;
            config.gateway.providers[0].rpm = 100;
            config.gateway.router.load_balancer.health_check_enabled = false;
            config.gateway.router.circuit_breaker.failure_threshold = 1;
            config.gateway.router.circuit_breaker.min_requests = 1;
        })
        .await;
        {
            let mut value = upstream.output.lock().unwrap();
            value["status"] = json!("failed");
            value["error"] = json!({"code":"server_error","message":"generation failed"});
            match usage {
                None => value["usage"] = Value::Null,
                Some(0) => {
                    value["usage"] = json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
                }
                Some(15) => {}
                _ => unreachable!(),
            }
        }
        let router = state.unified_router();
        let ids = router.get_deployments_for_model("gpt-4o-mini");
        let deployment = router.get_deployment(&ids[0]).unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(json!({"model":"gpt-4o-mini","input":"Hello","stream":false,"store":false,"max_output_tokens":64}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let value: Value = test::read_body_json(response).await;
        assert_eq!(value["status"], "failed");
        assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(
            deployment.state.fails_this_minute.load(Ordering::Relaxed),
            1
        );
        assert!(deployment.state.cooldown_until.load(Ordering::Relaxed) > 0);
        assert_eq!(
            deployment.state.tpm_current.load(Ordering::Relaxed),
            usage.unwrap_or(0)
        );
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        handle.stop(false).await;
    }
}

fn cache_write_pricing(state: &AppState, output_bound: Value) {
    state.pricing.add_custom_model(
        "gpt-4o-mini".into(),
        serde_json::from_value(json!({
            "litellm_provider":"openai", "mode":"chat", "max_output_tokens":output_bound,
            "input_cost_per_token":0.000001, "output_cost_per_token":0.00002,
            "cache_read_input_token_cost":0.0000001, "cache_creation_input_token_cost":0.00000125
        }))
        .unwrap(),
    );
}

#[tokio::test]
async fn compact_reserves_model_output_and_cache_writes_before_generation() {
    use litellm_rs::core::{
        budget::{BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod},
        types::context::RequestContext,
    };
    // 0.02 catches the old 100-output-token allowance; 0.024 fits the model
    // output bound plus ordinary input, but not the provider's cache-write rate.
    for scope in ["provider", "model", "key"] {
        for limit in [0.02, 0.024] {
            let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
            cache_write_pricing(&state, json!(500));
            let mut context = RequestContext::new();
            match scope {
                "provider" => state.budget_limits.providers.set_provider_limit(
                    "native-test",
                    ProviderLimitConfig::new(limit, ResetPeriod::Monthly),
                ),
                "model" => state.budget_limits.models.set_model_limit(
                    "gpt-4o-mini",
                    ModelLimitConfig::new(limit, ResetPeriod::Monthly),
                ),
                _ => {
                    let budget = state
                        .budget_manager
                        .create_budget(
                            BudgetScope::ApiKey("compact-test".into()),
                            BudgetConfig::new("compact bound", limit),
                        )
                        .await
                        .unwrap();
                    context = context.with_api_key_budget(budget.id.parse().unwrap());
                }
            }
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let req = test::TestRequest::post().uri("/v1/responses/compact").set_json(json!({"model":"gpt-4o-mini","input":[{"type":"compaction","encrypted_content":"opaque=="}]})).to_request();
            req.extensions_mut().insert(context);
            let response = test::call_service(&app, req).await;
            assert_eq!(
                response.status(),
                StatusCode::PAYMENT_REQUIRED,
                "{scope}/{limit}"
            );
            assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
            assert!(upstream.seen.lock().unwrap().is_empty());
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn compact_settles_cache_write_usage_in_provider_model_and_key_budgets() {
    use litellm_rs::core::{
        budget::{BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod},
        keys::CreateKeyConfig,
        types::context::RequestContext,
    };
    let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
    cache_write_pricing(&state, json!(500));
    upstream.output.lock().unwrap()["usage"]["input_tokens_details"]["cache_write_tokens"] =
        json!(5);
    state.budget_limits.providers.set_provider_limit(
        "native-test",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gpt-4o-mini",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let scope = BudgetScope::ApiKey("compact-settlement".into());
    let budget = state
        .budget_manager
        .create_budget(scope.clone(), BudgetConfig::new("compact settlement", 1.0))
        .await
        .unwrap();
    let (key_id, _) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "compact-cache-write".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let req = test::TestRequest::post()
        .uri("/v1/responses/compact")
        .set_json(json!({"model":"gpt-4o-mini","input":"Hello"}))
        .to_request();
    req.extensions_mut().insert(
        RequestContext::new()
            .with_api_key(key_id)
            .with_api_key_budget(budget.id.parse().unwrap()),
    );
    let response = test::call_service(&app, req).await;
    let status = response.status();
    let value: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(
        value["usage"]["input_tokens_details"]["cache_write_tokens"],
        5
    );
    let expected = 3.0 * 0.000001 + 4.0 * 0.0000001 + 5.0 * 0.00000125 + 3.0 * 0.00002;
    for actual in [
        state
            .budget_limits
            .providers
            .get_provider_usage("native-test")
            .unwrap()
            .current_spend,
        state
            .budget_limits
            .models
            .get_model_usage("gpt-4o-mini")
            .unwrap()
            .current_spend,
        state.budget_manager.get_current_spend(&scope),
        state
            .key_manager
            .get_usage_stats(key_id)
            .await
            .unwrap()
            .total_cost,
    ] {
        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn compact_rejects_unknown_output_bounds_and_invalid_cache_write_usage() {
    for (bound, written, status) in [
        (Value::Null, json!(5), StatusCode::BAD_REQUEST),
        (json!(500), json!(9), StatusCode::BAD_GATEWAY),
        (json!(500), json!(-1), StatusCode::BAD_GATEWAY),
        (json!(500), json!(1.5), StatusCode::BAD_GATEWAY),
        (json!(500), json!(u64::MAX), StatusCode::BAD_GATEWAY),
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
        cache_write_pricing(&state, bound);
        upstream.output.lock().unwrap()["usage"]["input_tokens_details"]["cache_write_tokens"] =
            written;
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
                .set_json(json!({"model":"gpt-4o-mini","input":"Hello"}))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), status);
        assert_eq!(
            upstream.seen.lock().unwrap().len(),
            usize::from(status == StatusCode::BAD_GATEWAY)
        );
        handle.stop(false).await;
    }
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn compact_skips_higher_priority_non_openai_responses_deployments() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.router.strategy =
            litellm_rs::core::router::config::RoutingStrategy::PriorityBased;
        config.gateway.providers[0].priority = 1;
    })
    .await;
    let router = state.unified_router();
    let provider =
        litellm_rs::core::providers::github_copilot::GitHubCopilotProvider::new(Default::default())
            .await
            .unwrap();
    router.add_deployment(litellm_rs::core::router::deployment::Deployment::new(
        "copilot-test".into(),
        litellm_rs::core::providers::Provider::GitHubCopilot(provider),
        "gpt-4o-mini".into(),
        "gpt-4o-mini".into(),
    ));
    let copilot = router.get_deployment("copilot-test").unwrap();
    assert!(
        copilot
            .provider
            .capabilities()
            .contains(&litellm_rs::core::types::model::ProviderCapability::Responses)
    );
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
            .set_json(json!({"model":"gpt-4o-mini","input":"Hello"}))
            .to_request(),
    )
    .await;
    let status = response.status();
    let value: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    assert_eq!(copilot.state.total_requests.load(Ordering::Relaxed), 0);
    handle.stop(false).await;
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn file_search_skips_higher_priority_non_openai_responses_deployments() {
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.router.strategy =
            litellm_rs::core::router::config::RoutingStrategy::PriorityBased;
        config.gateway.providers[0].priority = 1;
    })
    .await;
    upstream.output.lock().unwrap()["output"] = json!([]);
    let router = state.unified_router();
    let provider =
        litellm_rs::core::providers::github_copilot::GitHubCopilotProvider::new(Default::default())
            .await
            .unwrap();
    router.add_deployment(litellm_rs::core::router::deployment::Deployment::new(
        "copilot-test".into(),
        litellm_rs::core::providers::Provider::GitHubCopilot(provider),
        "gpt-4o-mini".into(),
        "gpt-4o-mini".into(),
    ));
    let copilot = router.get_deployment("copilot-test").unwrap();
    assert!(
        copilot
            .provider
            .capabilities()
            .contains(&litellm_rs::core::types::model::ProviderCapability::Responses)
    );
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
            .set_json(file_search_request(false))
            .to_request(),
    )
    .await;
    let status = response.status();
    let value: Value = test::read_body_json(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
    assert_eq!(copilot.state.total_requests.load(Ordering::Relaxed), 0);
    handle.stop(false).await;
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
async fn native_stream_client_disconnect_preserves_tpm_and_rpm_admission() {
    use litellm_rs::core::router::DeploymentConfig;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for quota in ["tpm", "rpm"] {
        let (state, upstream, upstream_handle) = fixture(StatusCode::OK, |config| {
            config.gateway.router.load_balancer.health_check_enabled = false;
        })
        .await;
        upstream.output.lock().unwrap()["test_pause_after_output"] = json!(true);
        let router = state.unified_router();
        let ids = router.get_deployments_for_model("gpt-4o-mini");
        assert_eq!(ids.len(), 1);
        let deployment = router.get_deployment(&ids[0]).unwrap();
        // Bootstrap keeps valid provider limits. Express each independent quota
        // on the existing deployment; None disables the other policy.
        router.add_deployment(deployment.as_ref().clone().with_config(DeploymentConfig {
            rpm_limit: (quota == "rpm").then_some(1),
            tpm_limit: (quota == "tpm").then_some(1000),
            max_parallel_requests: Some(1),
            ..deployment.config.clone()
        }));
        let deployment = router.get_deployment(&ids[0]).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let gateway = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes)
        })
        .workers(1)
        .listen(listener)
        .unwrap()
        .run();
        let gateway_handle = gateway.handle();
        tokio::spawn(gateway);
        // These are real sockets and the production quota uses wall time.
        // Start away from the minute boundary rather than faking that clock.
        let second = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            % 60;
        if second >= 38 {
            tokio::time::sleep(Duration::from_secs(61 - second)).await;
        }
        let mut body = request(true);
        // Keep the safe output inside its requested bound in both quota cases.
        // One TPM estimate fits; two 700-token output bounds do not.
        body["max_output_tokens"] = json!(700);
        let payload = serde_json::to_vec(&body).unwrap();
        let mut connection = tokio::net::TcpStream::connect(address).await.unwrap();
        let headers = format!(
            "POST /v1/responses HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        );
        connection.write_all(headers.as_bytes()).await.unwrap();
        connection.write_all(&payload).await.unwrap();
        let mut wire = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !String::from_utf8_lossy(&wire).contains("你好") {
                let mut chunk = [0_u8; 4096];
                let read = connection.read(&mut chunk).await.unwrap();
                assert_ne!(read, 0, "upstream stays open after its output delta");
                wire.extend_from_slice(&chunk[..read]);
            }
        })
        .await
        .expect("the client must receive actual native output before disconnecting");
        let wire = String::from_utf8_lossy(&wire);
        assert!(wire.starts_with("HTTP/1.1 200"), "{wire}");
        assert!(wire.contains("response.created"));
        assert!(wire.contains("response.output_text.delta"));
        assert!(!wire.contains("response.completed"));
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        // FIN is a valid HTTP half-close and Actix keeps a pending response
        // alive. Reset this owned socket to exercise an actual lost client.
        socket2::SockRef::from(&connection)
            .set_linger(Some(Duration::ZERO))
            .unwrap();
        drop(connection);

        tokio::time::timeout(Duration::from_secs(5), async {
            while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the actual HTTP disconnect must release the selected lease");
        assert_eq!(
            deployment.state.rpm_current.load(Ordering::Relaxed),
            1,
            "{quota}"
        );
        assert_eq!(
            deployment.state.tpm_current.load(Ordering::Relaxed),
            0,
            "{quota}"
        );
        assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
        assert!(deployment.is_healthy());

        if quota == "tpm" {
            // The HTTP route supplies the same positive request estimate. Its
            // quota retry waits 60s; bound this probe before the minute rolls.
            let client = reqwest::Client::new();
            assert!(
                tokio::time::timeout(
                    Duration::from_secs(1),
                    client
                        .post(format!("http://{address}/v1/responses"))
                        .json(&body)
                        .send(),
                )
                .await
                .is_err(),
                "retained estimate must deny another positive native request"
            );
        } else {
            // RPM denial does not depend on an input/output estimate.
            assert!(
                router
                    .select_deployment_lease_async("gpt-4o-mini")
                    .await
                    .is_err(),
                "{quota}: a partial native response must consume the current-minute quota"
            );
        }
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        gateway_handle.stop(false).await;
        upstream_handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_stream_half_closed_writer_reads_heartbeat_and_completion() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (state, upstream, upstream_handle) = fixture(StatusCode::OK, |_| {}).await;
    upstream.output.lock().unwrap()["test_pause_after_output"] = json!(true);
    upstream.output.lock().unwrap()["test_complete_after_pause"] = json!(true);
    let router = state.unified_router();
    let deployment = router.get_deployment("native-test-gpt-4o-mini").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let gateway = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes)
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let gateway_handle = gateway.handle();
    tokio::spawn(gateway);
    let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
    // The shared stalled fixture crosses the default output guard window.
    // Keep its safe output within the same bound as the full-disconnect case.
    let mut body = request(true);
    body["max_output_tokens"] = json!(700);
    let body = body.to_string();
    let wire = format!(
        "POST /v1/responses HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    client.write_all(wire.as_bytes()).await.unwrap();
    // Normal HTTP half-close: stop sending, but keep reading the full response.
    client.shutdown().await.unwrap();
    let mut received = Vec::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(6),
        client.read_to_end(&mut received),
    )
    .await
    .unwrap()
    .unwrap();
    let wire = String::from_utf8(received).unwrap();
    assert!(wire.starts_with("HTTP/1.1 200"), "{wire}");
    assert!(wire.contains("你好"), "{wire}");
    assert!(wire.contains(": keep-alive"), "{wire}");
    assert!(wire.contains("response.completed"), "{wire}");
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 15);
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    gateway_handle.stop(false).await;
    upstream_handle.stop(false).await;
}

#[tokio::test]
async fn native_stream_heartbeat_keeps_upstream_idle_deadline_and_usage() {
    let (state, upstream, upstream_handle) = fixture(StatusCode::OK, |config| {
        config.gateway.server.stream_idle_timeout = 2;
    })
    .await;
    upstream.output.lock().unwrap()["test_pause_before_output"] = json!(true);
    let router = state.unified_router();
    let deployment = router.get_deployment("native-test-gpt-4o-mini").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let gateway = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(litellm_rs::server::routes::ai::configure_routes)
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let gateway_handle = gateway.handle();
    tokio::spawn(gateway);
    let wire = tokio::time::timeout(std::time::Duration::from_secs(4), async {
        reqwest::Client::new()
            .post(format!("http://{address}/v1/responses"))
            .json(&request(true))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    })
    .await
    .expect("downstream comments must not restart the two-second upstream idle deadline");
    assert!(wire.contains(": keep-alive"), "{wire}");
    assert!(wire.contains("Responses stream idle timeout"), "{wire}");
    assert!(!wire.contains("response.output_text.delta"), "{wire}");
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(upstream.seen.lock().unwrap().len(), 1);
    gateway_handle.stop(false).await;
    upstream_handle.stop(false).await;
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
        assert_eq!(value, *upstream.output.lock().unwrap());
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
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        // Retained context must reach the monetary budget boundary, not fail
        // this fixture's unrelated deployment TPM admission first.
        config.gateway.providers[0].tpm = 1_000_000;
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
    use litellm_rs::core::types::context::RequestContext;
    for (status, known_usage) in [
        ("completed", true),
        ("cancelled", true),
        ("failed", true),
        ("failed", false),
    ] {
        let cancel = status == "cancelled";
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
        {
            let mut output = upstream.output.lock().unwrap();
            output["status"] = json!(status);
            if !known_usage {
                output["usage"] = Value::Null;
            }
        }
        let router = state.unified_router();
        let id = router
            .get_deployments_for_model("gpt-4o-mini")
            .pop()
            .unwrap();
        let deployment = router.get_deployment(&id).unwrap();
        let initial_health = deployment.state.health.load(Ordering::Relaxed);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = request(false);
        body["background"] = json!(true);
        body["store"] = json!(true);
        body["input"] =
            json!([{"role":"user","content":[{"type":"input_file","file_id":"file-pdf"}]}]);
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
        assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
        assert!((settlement_row(&state).await.reserved - 0.0018096).abs() < 1e-10);
        assert!(!settlement_row(&state).await.complete);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
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
        assert_eq!(u8::from(settlement_row(&state).await.complete), 0);
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
                if u8::from(settlement_row(&state).await.complete) == 1
                    && deployment.state.active_requests.load(Ordering::Relaxed) == 0
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let settled = settlement_row(&state).await;
        assert_eq!(
            settled.outcome,
            if known_usage {
                "actual"
            } else {
                "reserved_unknown"
            }
        );
        assert!(settled.cost.unwrap() > 0.0);
        assert_eq!(settled.tokens, if known_usage { 15 } else { 0 });
        if !known_usage {
            assert_eq!(settled.cost, Some(settled.reserved));
        }
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(
            deployment.state.tpm_current.load(Ordering::Relaxed),
            u64::from(known_usage) * 15
        );
        assert_eq!(
            deployment.state.success_requests.load(Ordering::Relaxed),
            u64::from(status == "completed")
        );
        assert_eq!(
            deployment.state.fail_requests.load(Ordering::Relaxed),
            u64::from(status == "failed")
        );
        assert_eq!(
            deployment.state.total_requests.load(Ordering::Relaxed),
            u64::from(status != "cancelled")
        );
        if cancel {
            assert_eq!(
                deployment.state.health.load(Ordering::Relaxed),
                initial_health
            );
        }
        let counts = (
            deployment.state.rpm_current.load(Ordering::Relaxed),
            deployment.state.tpm_current.load(Ordering::Relaxed),
            deployment.state.total_requests.load(Ordering::Relaxed),
            deployment.state.success_requests.load(Ordering::Relaxed),
            deployment.state.fail_requests.load(Ordering::Relaxed),
        );
        for _ in 0..3 {
            let req = test::TestRequest::get()
                .uri("/v1/responses/resp_native")
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let value: Value = test::call_and_read_body_json(&app, req).await;
            assert_eq!(value["status"], status);
        }
        assert_eq!(u8::from(settlement_row(&state).await.complete), 1);
        assert_eq!(upstream.seen.lock().unwrap().len(), 1);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(
            counts,
            (
                deployment.state.rpm_current.load(Ordering::Relaxed),
                deployment.state.tpm_current.load(Ordering::Relaxed),
                deployment.state.total_requests.load(Ordering::Relaxed),
                deployment.state.success_requests.load(Ordering::Relaxed),
                deployment.state.fail_requests.load(Ordering::Relaxed),
            ),
            "lifecycle reads must not duplicate usage or upstream health outcomes"
        );
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_background_failed_poll_records_failure_and_keeps_usage_once() {
    use litellm_rs::core::types::context::RequestContext;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    for shared in [false, true] {
        let redis_url = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                continue;
            };
            Some(url)
        } else {
            None
        };
        for usage in [None, Some(0), Some(15)] {
            let observed_usage = usage;
            let dir = tempfile::tempdir().unwrap();
            let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
                config.gateway.storage.database.enabled = true;
                config.gateway.storage.database.auto_migrate = true;
                config.gateway.storage.database.url = format!(
                    "sqlite://{}?mode=rwc",
                    dir.path().join("failed-background.db").display()
                );
                if let Some(url) = &redis_url {
                    config.gateway.storage.redis.enabled = true;
                    config.gateway.storage.redis.allow_degraded = false;
                    config.gateway.storage.redis.url = url.clone();
                }
                // Shared keys are isolated. Input plus 64 output tokens fits
                // once; unknown usage must retain that complete estimate.
                config.gateway.providers[0].name =
                    format!("native-failed-{}", uuid::Uuid::new_v4());
                config.gateway.providers[0].rpm = 100;
                config.gateway.providers[0].tpm = 128;
                config.gateway.providers[0].max_concurrent_requests = 100;
                config.gateway.router.load_balancer.health_check_enabled = false;
            })
            .await;
            {
                let mut output = upstream.output.lock().unwrap();
                output["status"] = json!("failed");
                output["error"] = json!({"code":"server_error","message":"generation failed"});
                match usage {
                    None => output["usage"] = Value::Null,
                    Some(0) => {
                        output["usage"] =
                            json!({"input_tokens":0,"output_tokens":0,"total_tokens":0});
                    }
                    Some(15) => {}
                    _ => unreachable!(),
                }
            }
            let router = state.unified_router();
            let ids = router.get_deployments_for_model("gpt-4o-mini");
            assert_eq!(ids.len(), 1);
            let deployment = router.get_deployment(&ids[0]).unwrap();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut redis = match &redis_url {
                Some(url) => Some(
                    redis::Client::open(url.as_str())
                        .unwrap()
                        .get_multiplexed_async_connection()
                        .await
                        .unwrap(),
                ),
                None => None,
            };
            // The production quota uses wall time, and polling starts after a
            // real one-second delay. Keep each observation in one minute.
            let second = if let Some(conn) = &mut redis {
                let (seconds, _micros): (u64, u64) =
                    redis::cmd("TIME").query_async(conn).await.unwrap();
                seconds % 60
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    % 60
            };
            if second >= 40 {
                tokio::time::sleep(Duration::from_secs(61 - second)).await;
            }
            let body = json!({
                "model":"gpt-4o-mini", "input":"Hello", "stream":false,
                "background":true, "store":true, "max_output_tokens":64
            });
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(&body)
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::OK);
            let queued: Value = test::read_body_json(response).await;
            assert_eq!(queued["status"], "queued");
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 0);

            let (admission_key, circuit_key, reserved_tokens) = if let Some(conn) = &mut redis {
                // Gateway keys append the captured resource identity. Discover
                // only this UUID deployment's owned key instead of duplicating
                // the private identity digest algorithm in an integration test.
                let pattern = format!("litellm-rs:admission:v1:{}:*", deployment.id);
                let mut cursor = 0_u64;
                let mut keys = Vec::new();
                loop {
                    let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(conn)
                        .await
                        .unwrap();
                    keys.extend(batch);
                    cursor = next;
                    if cursor == 0 {
                        break;
                    }
                }
                keys.sort();
                keys.dedup();
                assert_eq!(keys.len(), 1, "exactly one owned gateway reservation");
                let admission_key = keys.pop().unwrap();
                let shared_id = admission_key
                    .strip_prefix("litellm-rs:admission:v1:")
                    .unwrap();
                let circuit_key = format!("litellm-rs:circuit:v1:{shared_id}");
                let reservation: (i64, i64, i64) = redis::cmd("HMGET")
                    .arg(&admission_key)
                    .arg(&["p", "r", "t"])
                    .query_async(conn)
                    .await
                    .unwrap();
                assert_eq!((reservation.0, reservation.1), (1, 1));
                assert!(reservation.2 > 0);
                (admission_key, circuit_key, reservation.2)
            } else {
                (String::new(), String::new(), 0)
            };
            // POST returned a genuine queued object. Only a later HTTP GET
            // reveals the failed terminal value with its known/unknown usage.
            upstream.background_state.store(2, Ordering::SeqCst);
            tokio::time::timeout(Duration::from_secs(10), async {
                while deployment.state.fail_requests.load(Ordering::Relaxed) != 1
                    || deployment.state.active_requests.load(Ordering::Relaxed) != 0
                {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("a failed background generation must finish as failure, never success");
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
            assert_eq!(
                deployment
                    .state
                    .consecutive_successes
                    .load(Ordering::Relaxed),
                0
            );
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(
                deployment.state.tpm_current.load(Ordering::Relaxed),
                usage.unwrap_or(0)
            );
            assert!(
                upstream
                    .lifecycle_calls
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|call| { call == "GET /v1/responses/resp_native" })
            );
            let settled = settlement_row(&state).await;
            assert!(settled.complete);
            assert_eq!(settled.tokens, i64::try_from(usage.unwrap_or(0)).unwrap());
            assert_eq!(
                settled.outcome,
                if observed_usage.is_some() {
                    "actual"
                } else {
                    "reserved_unknown"
                }
            );
            if observed_usage.is_none() {
                assert_eq!(settled.cost, Some(settled.reserved));
                assert!(deployment.is_healthy());
                let req = test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(&body)
                    .to_request();
                req.extensions_mut()
                    .insert(RequestContext::new().with_user_id("alice"));
                let next =
                    tokio::time::timeout(Duration::from_secs(1), test::call_service(&app, req))
                        .await;
                assert!(
                    next.is_err(),
                    "unknown usage must retain the complete request estimate"
                );
            }
            if usage == Some(0) {
                assert_eq!(settled.cost, Some(0.0));
            } else {
                assert!(settled.cost.unwrap() > 0.0);
            }

            if let Some(conn) = &mut redis {
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let counts: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = redis::cmd("HMGET")
                            .arg(&circuit_key)
                            .arg(&["tot", "fail", "r", "consec"])
                            .query_async(conn)
                            .await
                            .unwrap();
                        let admission: (Option<i64>, Option<i64>, Option<i64>) = redis::cmd("HMGET")
                            .arg(&admission_key)
                            .arg(&["p", "r", "t"])
                            .query_async(conn)
                            .await
                            .unwrap();
                        let expected_tokens = observed_usage.map_or(reserved_tokens, |tokens| tokens as i64);
                        if counts == (Some(1), Some(1), Some(0), Some(0))
                            && admission == (Some(0), Some(1), Some(expected_tokens))
                        {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("shared circuit must record one failure, with retained or actual admission usage");
                let fields: Vec<String> = redis::cmd("HKEYS")
                    .arg(&admission_key)
                    .query_async(conn)
                    .await
                    .unwrap();
                assert!(!fields.iter().any(|field| field.starts_with("l:")));
            }

            for _ in 0..2 {
                let req = test::TestRequest::get()
                    .uri("/v1/responses/resp_native")
                    .to_request();
                req.extensions_mut()
                    .insert(RequestContext::new().with_user_id("alice"));
                let value: Value = test::call_and_read_body_json(&app, req).await;
                assert_eq!(value["status"], "failed");
            }
            assert_eq!(upstream.seen.lock().unwrap().len(), 1);
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
            if let Some(conn) = &mut redis {
                let counts: (i64, i64, i64) = redis::cmd("HMGET")
                    .arg(&circuit_key)
                    .arg(&["tot", "fail", "r"])
                    .query_async(conn)
                    .await
                    .unwrap();
                assert_eq!(counts, (1, 1, 0));
                let _: i64 = redis::cmd("DEL")
                    .arg(&[admission_key, circuit_key])
                    .query_async(conn)
                    .await
                    .unwrap();
            }
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn native_background_unknown_usage_retains_admission_and_local_errors_stay_neutral() {
    use litellm_rs::core::types::context::RequestContext;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    for shared in [false, true] {
        let redis_url = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                continue;
            };
            Some(url)
        } else {
            None
        };
        for (status, known_zero) in [
            ("completed", false),
            ("incomplete", false),
            ("cancelled", false),
            ("invalid_status", false),
            ("missing_id", false),
            ("completed", true),
        ] {
            let completed = status == "completed";
            let dir = tempfile::tempdir().unwrap();
            let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
                config.gateway.storage.database.enabled = true;
                config.gateway.storage.database.auto_migrate = true;
                config.gateway.storage.database.url = format!(
                    "sqlite://{}?mode=rwc",
                    dir.path().join("background-usage.db").display()
                );
                if let Some(url) = &redis_url {
                    config.gateway.storage.redis.enabled = true;
                    config.gateway.storage.redis.allow_degraded = false;
                    config.gateway.storage.redis.url = url.clone();
                }
                config.gateway.providers[0].name =
                    format!("native-unknown-{}", uuid::Uuid::new_v4());
                // Input plus 64 output tokens fits only once in 128 TPM.
                // RPM remains nonbinding, exposing incorrect TPM refunds.
                config.gateway.providers[0].rpm = 100;
                config.gateway.providers[0].tpm = 128;
                config.gateway.providers[0].max_concurrent_requests = 100;
                config.gateway.router.load_balancer.health_check_enabled = false;
            })
            .await;
            {
                let mut output = upstream.output.lock().unwrap();
                output["status"] = json!(status);
                output["usage"] = if known_zero {
                    json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
                } else {
                    Value::Null
                };
                if status == "missing_id" {
                    output["id"] = json!("");
                }
            }
            let router = state.unified_router();
            let ids = router.get_deployments_for_model("gpt-4o-mini");
            assert_eq!(ids.len(), 1);
            let deployment = router.get_deployment(&ids[0]).unwrap();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut redis = match &redis_url {
                Some(url) => Some(
                    redis::Client::open(url.as_str())
                        .unwrap()
                        .get_multiplexed_async_connection()
                        .await
                        .unwrap(),
                ),
                None => None,
            };
            let second = if let Some(conn) = &mut redis {
                let (seconds, _micros): (u64, u64) =
                    redis::cmd("TIME").query_async(conn).await.unwrap();
                seconds % 60
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    % 60
            };
            if second >= 40 {
                tokio::time::sleep(Duration::from_secs(61 - second)).await;
            }
            let body = json!({
                "model":"gpt-4o-mini", "input":"Hello", "stream":false,
                "background":true, "store":true, "max_output_tokens":64
            });
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(&body)
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let response = test::call_service(&app, req).await;
            if status == "missing_id" {
                assert!(response.status().is_server_error());
                let _ = test::read_body(response).await;
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                let queued: Value = test::read_body_json(response).await;
                assert_eq!(queued["status"], "queued");
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
                // A real later upstream GET supplies the terminal value or
                // malformed status; POST itself only accepted the work.
                upstream.background_state.store(2, Ordering::SeqCst);
            }
            tokio::time::timeout(Duration::from_secs(10), async {
                while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("background completion must release its exact lease");
            let successes = u64::from(completed);
            assert_eq!(
                deployment.state.total_requests.load(Ordering::Relaxed),
                successes,
                "{status}"
            );
            assert_eq!(
                deployment.state.success_requests.load(Ordering::Relaxed),
                successes,
                "{status}"
            );
            assert_eq!(
                deployment.state.fail_requests.load(Ordering::Relaxed),
                0,
                "{status}"
            );
            assert_eq!(
                deployment
                    .state
                    .consecutive_successes
                    .load(Ordering::Relaxed),
                u32::from(completed),
                "{status}"
            );
            assert_eq!(
                deployment.state.rpm_current.load(Ordering::Relaxed),
                1,
                "{status}"
            );
            assert_eq!(
                deployment.state.tpm_current.load(Ordering::Relaxed),
                0,
                "{status}"
            );
            assert!(deployment.is_healthy());
            assert_eq!(upstream.seen.lock().unwrap().len(), 1);
            let settled = settlement_row(&state).await;
            if status == "missing_id" {
                assert!(
                    !settled.complete,
                    "the durable recovery owner must retain its obligation"
                );
                assert_eq!(settled.outcome, "pending");
                assert!(settled.response_id.is_none());
                assert!(settled.cost.is_none());
                assert!(upstream.lifecycle_calls.lock().unwrap().is_empty());
            } else {
                assert!(settled.complete);
                // Explicit validated zero is an actual observation; absent
                // usage keeps the durable monetary reservation conservative.
                assert_eq!(
                    settled.outcome,
                    if known_zero {
                        "actual"
                    } else {
                        "reserved_unknown"
                    }
                );
                assert_eq!(
                    settled.cost,
                    Some(if known_zero { 0.0 } else { settled.reserved })
                );
                assert_eq!(settled.tokens, 0);
                assert!(
                    upstream
                        .lifecycle_calls
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|call| call == "GET /v1/responses/resp_native")
                );
            }

            let mut owned_keys = Vec::new();
            if let Some(conn) = &mut redis {
                let pattern = format!("litellm-rs:admission:v1:{}:*", deployment.id);
                let mut cursor = 0_u64;
                loop {
                    let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(conn)
                        .await
                        .unwrap();
                    owned_keys.extend(batch);
                    cursor = next;
                    if cursor == 0 {
                        break;
                    }
                }
                owned_keys.sort();
                owned_keys.dedup();
                assert_eq!(owned_keys.len(), 1);
                let admission_key = owned_keys[0].clone();
                let shared_id = admission_key
                    .strip_prefix("litellm-rs:admission:v1:")
                    .unwrap();
                let circuit_key = format!("litellm-rs:circuit:v1:{shared_id}");
                let admission = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let counts: (i64, i64, i64) = redis::cmd("HMGET")
                            .arg(&admission_key)
                            .arg(&["p", "r", "t"])
                            .query_async(conn)
                            .await
                            .unwrap();
                        if counts.0 == 0 {
                            break counts;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("shared admission cleanup must complete");
                assert_eq!((admission.0, admission.1), (0, 1), "{status}");
                if known_zero {
                    assert_eq!(admission.2, 0, "validated zero must refund the estimate");
                } else {
                    assert!(
                        (65..=128).contains(&admission.2),
                        "{status}: retained request estimate"
                    );
                }
                let counts: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) =
                    redis::cmd("HMGET")
                        .arg(&circuit_key)
                        .arg(&["tot", "fail", "r", "consec"])
                        .query_async(conn)
                        .await
                        .unwrap();
                let expected = i64::from(completed);
                assert_eq!(
                    (
                        counts.0.unwrap_or(0),
                        counts.1.unwrap_or(0),
                        counts.2.unwrap_or(0),
                        counts.3.unwrap_or(0)
                    ),
                    (expected, 0, expected, expected),
                    "{status}"
                );
                owned_keys.push(circuit_key);
            }
            // Missing usage retains the request estimate, including local
            // background errors. Validated zero leaves the next call admissible.
            let mut next_body = body.clone();
            next_body["background"] = json!(false);
            next_body["store"] = json!(false);
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(&next_body)
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_user_id("alice"));
            let next = tokio::time::timeout(
                Duration::from_secs(if known_zero { 5 } else { 1 }),
                test::call_service(&app, req),
            )
            .await;
            if known_zero {
                let next = next.expect("known zero leaves the next request admissible");
                assert_eq!(next.status(), StatusCode::OK);
                let _ = test::read_body(next).await;
            } else {
                assert!(
                    next.is_err(),
                    "{status}, zero={known_zero}, shared={shared}"
                );
            }
            assert_eq!(
                upstream.seen.lock().unwrap().len(),
                if known_zero { 2 } else { 1 }
            );
            if let Some(conn) = &mut redis {
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let pending: i64 = redis::cmd("HGET")
                            .arg(&owned_keys[0])
                            .arg("p")
                            .query_async(conn)
                            .await
                            .unwrap();
                        if pending == 0 {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("the background lease must release before cleanup");
                let _: i64 = redis::cmd("DEL")
                    .arg(&owned_keys)
                    .query_async(conn)
                    .await
                    .unwrap();
            }
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn native_background_store_false_keeps_only_temporary_handle_metadata() {
    use litellm_rs::core::types::context::RequestContext;
    let dir = tempfile::tempdir().unwrap();
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.storage.database.enabled = true;
        config.gateway.storage.database.auto_migrate = true;
        config.gateway.storage.database.url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("temporary.db").display()
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
        let router = state.unified_router();
        let id = router
            .get_deployments_for_model("gpt-4o-mini")
            .pop()
            .unwrap();
        let deployment = router.get_deployment(&id).unwrap();
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
        assert_eq!(u8::from(settlement_row(&state).await.complete), 0);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
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
                if u8::from(settlement_row(&state).await.complete) == 1
                    && deployment.state.active_requests.load(Ordering::Relaxed) == 0
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
        assert_eq!(u8::from(settlement_row(&state).await.complete), 1);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 15);
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
    let body = json!({"model":"gpt-4o-mini","service_tier":"default","input":[{"type":"compaction","id":"cmp_old","encrypted_content":"opaque-old=="},{"role":"user","content":"Hello"}],"instructions":"Keep facts","future_field":{"preserve":true}});
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
    assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
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

async fn settlement_row(
    state: &AppState,
) -> litellm_rs::storage::database::entities::response_settlement::Model {
    use sea_orm::EntityTrait;
    litellm_rs::storage::database::entities::response_settlement::Entity::find()
        .one(state.storage.database.connection())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn background_rejects_process_local_budgets_before_dispatch() {
    use litellm_rs::core::budget::{ModelLimitConfig, ResetPeriod};
    use litellm_rs::core::types::context::RequestContext;
    let dir = tempfile::tempdir().unwrap();
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.storage.database.enabled = true;
        config.gateway.storage.database.auto_migrate = true;
        config.gateway.storage.database.url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("limits.db").display()
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
    body["background"] = json!(true);
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut().insert(
        RequestContext::new()
            .with_user_id("alice")
            .with_api_key_budget(uuid::Uuid::new_v4()),
    );
    let response = test::call_service(&app, req).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    state.budget_limits.models.set_model_limit(
        "gpt-4o-mini",
        ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let req = test::TestRequest::post()
        .uri("/v1/responses")
        .set_json(&body)
        .to_request();
    req.extensions_mut()
        .insert(RequestContext::new().with_user_id("alice"));
    let response = test::call_service(&app, req).await;
    assert!(!response.status().is_success());
    let value: Value = test::read_body_json(response).await;
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Redis")
    );
    assert!(upstream.seen.lock().unwrap().is_empty());
    handle.stop(false).await;
}

#[tokio::test]
async fn publicly_created_chat_key_can_use_native_responses_and_usage_is_durable() {
    use litellm_rs::core::models::user::types::{User, UserStatus};
    use litellm_rs::server::middleware::AuthMiddleware;
    let dir = tempfile::tempdir().unwrap();
    let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
        config.gateway.storage.database.enabled = true;
        config.gateway.storage.database.auto_migrate = true;
        config.gateway.storage.database.url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("key-usage.db").display()
        );
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = false;
    })
    .await;
    let mut owner = User::new(
        "response-owner".into(),
        "response-owner@example.com".into(),
        "unused-test-hash".into(),
    );
    owner.status = UserStatus::Active;
    state.storage.database.create_user(&owner).await.unwrap();
    // The public creation API validates api.chat; no hand-inserted api.responses permission.
    let (key, token) = state
        .auth
        .api_key()
        .create_key(
            Some(owner.metadata.id),
            None,
            "responses".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state.clone()))
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let mut body = request(false);
    body["background"] = json!(true);
    body["store"] = json!(true);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses")
            .insert_header(("authorization", format!("ApiKey {token}")))
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
    upstream.background_state.store(2, Ordering::SeqCst);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !settlement_row(&state).await.complete {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let before = state
        .storage
        .database
        .find_api_key_by_id(key.metadata.id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(before.total_requests, 1);
    assert_eq!(before.total_tokens, 15);
    assert!(before.total_cost > 0.0);
    for _ in 0..2 {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/resp_native")
                .insert_header(("authorization", format!("ApiKey {token}")))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
    }
    let after = state
        .storage
        .database
        .find_api_key_by_id(key.metadata.id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(after.total_requests, before.total_requests);
    assert_eq!(after.total_cost, before.total_cost);
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/responses/compact")
            .insert_header(("authorization", format!("ApiKey {token}")))
            .set_json(json!({"model":"gpt-4o-mini","input":"Compact this conversation"}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let _ = test::read_body(response).await;
    let usage = state
        .storage
        .database
        .find_api_key_by_id(key.metadata.id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(usage.total_requests, 2);
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
async fn native_unknown_usage_retains_admission_but_known_zero_does_not() {
    use actix_web::body::MessageBody;
    use litellm_rs::core::router::DeploymentConfig;
    use std::time::Duration;

    let mut backends = vec![None];
    if let Ok(url) = std::env::var("REDIS_URL") {
        backends.push(Some(url));
    }
    for redis_url in backends {
        for (streaming, terminal, output, known_zero) in [
            (true, false, true, false),
            (true, false, false, false),
            (true, true, true, false),
            (true, true, true, true),
            (false, true, true, false),
            (false, true, true, true),
        ] {
            let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
                config.gateway.providers[0].name =
                    format!("native-admission-{}", uuid::Uuid::new_v4());
                if let Some(url) = &redis_url {
                    config.gateway.storage.redis.enabled = true;
                    config.gateway.storage.redis.url = url.clone();
                }
            })
            .await;
            let router = state.unified_router();
            let id = router
                .get_deployments_for_model("gpt-4o-mini")
                .pop()
                .unwrap();
            let deployment = router.get_deployment(&id).unwrap();
            router.add_deployment(deployment.as_ref().clone().with_config(DeploymentConfig {
                tpm_limit: Some(1000),
                ..deployment.config.clone()
            }));
            let deployment = router.get_deployment(&id).unwrap();
            let initial_health = deployment.state.health.load(Ordering::Relaxed);
            {
                let mut response = upstream.output.lock().unwrap();
                response["usage"] = if known_zero {
                    json!({"input_tokens":0,"output_tokens":0,"total_tokens":0})
                } else {
                    Value::Null
                };
                response["test_stall_stream"] = json!(!terminal);
                response["test_output_before_stall"] = json!(output);
            }
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            // One positive input/output estimate fits 1000; two 700-output
            // reservations cannot fit. This tests retained quota through the route.
            let body = json!({"model":"gpt-4o-mini","stream":streaming,"store":false,"input":"x","max_output_tokens":700});
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(&body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            if terminal {
                let _ = test::read_body(response).await;
            } else {
                let mut response_body = Box::pin(response.into_body());
                tokio::time::timeout(Duration::from_secs(5), async {
                    let mut wire = String::new();
                    loop {
                        let chunk =
                            futures::future::poll_fn(|cx| response_body.as_mut().poll_next(cx))
                                .await
                                .expect("stream must expose creation/output before cancellation")
                                .unwrap();
                        wire.push_str(&String::from_utf8_lossy(&chunk));
                        if wire.contains(if output {
                            "generated"
                        } else {
                            "response.created"
                        }) {
                            break;
                        }
                    }
                })
                .await
                .expect("native stream must expose the expected prefix promptly");
                drop(response_body);
            }
            tokio::time::timeout(Duration::from_secs(5), async {
                while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("native completion must release the original lease");
            // Acceptance is known even when only response.created reached the client.
            // Retain its estimate without inventing observed tokens or health.
            let retained = !known_zero;
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
            assert_eq!(
                deployment.state.success_requests.load(Ordering::Relaxed),
                u64::from(terminal)
            );
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
            if !terminal {
                assert_eq!(
                    deployment.state.health.load(Ordering::Relaxed),
                    initial_health
                );
            }
            if let Some(redis_url) = &redis_url {
                let mut conn = redis::Client::open(redis_url.as_str())
                    .unwrap()
                    .get_multiplexed_async_connection()
                    .await
                    .unwrap();
                // The gateway binds admission to its captured resource identity.
                // This fixture has a unique provider; inspect that exact namespace.
                let mut cursor = 0_u64;
                let mut keys = Vec::<String>::new();
                loop {
                    let (next, found): (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(format!("litellm-rs:admission:v1:{id}:*"))
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    keys.extend(found);
                    cursor = next;
                    if cursor == 0 {
                        break;
                    }
                }
                keys.sort();
                keys.dedup();
                assert_eq!(keys.len(), 1, "one captured admission identity per fixture");
                let key = &keys[0];
                tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let counts: Vec<Option<i64>> = redis::cmd("HMGET")
                            .arg(key).arg(&["p", "r", "t"])
                            .query_async(&mut conn).await.unwrap();
                        if counts[0] == Some(0) {
                            assert_eq!(counts[1], Some(1));
                            let tokens = counts[2].unwrap();
                            assert!(if retained { tokens > 700 && tokens < 1000 } else { tokens == 0 });
                            let fields: Vec<String> = redis::cmd("HKEYS").arg(key).query_async(&mut conn).await.unwrap();
                            assert!(!fields.iter().any(|field| field.starts_with("l:") || field.starts_with("p:")));
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }).await.expect("shared admission must retain/refund the original hold");
            }
            // The existing selection retry policy waits 60s after a quota
            // denial. Bound/cancel this waiter before the minute can roll over.
            let next = tokio::time::timeout(
                Duration::from_secs(if retained { 1 } else { 5 }),
                test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/responses")
                        .set_json(&body)
                        .to_request(),
                ),
            )
            .await;
            if retained {
                assert!(
                    next.is_err(),
                    "retained estimate must deny the next admission"
                );
            } else {
                let next = next.expect("known-zero request must remain admissible");
                assert_eq!(next.status(), StatusCode::OK);
                drop(next);
            }
            assert_eq!(
                upstream.seen.lock().unwrap().len(),
                if retained { 1 } else { 2 }
            );
            tokio::time::timeout(Duration::from_secs(5), async {
                while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("follow-up must leave no active lease");
            handle.stop(false).await;
        }
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

fn file_search_request(streaming: bool) -> Value {
    let mut body = request(streaming);
    body["tools"] = json!([{"type":"file_search", "vector_store_ids":["vs_existing"],
        "max_num_results":2, "filters":{"type":"eq","key":"department","value":"support"}}]);
    body["max_tool_calls"] = json!(2);
    body
}

#[tokio::test]
async fn native_file_search_without_output_limit_reserves_catalog_bound() {
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    for streaming in [false, true] {
        for budget in [0.5, 4.0] {
            let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
            state.pricing.add_custom_model(
                "gpt-4o-mini".into(),
                serde_json::from_value(json!({
                    "litellm_provider":"openai", "mode":"chat",
                    "max_input_tokens":128000, "max_output_tokens":32000,
                    "input_cost_per_token":0.0, "output_cost_per_token":0.0001
                }))
                .unwrap(),
            );
            state.budget_limits.providers.set_provider_limit(
                "native-test",
                ProviderLimitConfig::new(budget, ResetPeriod::Monthly),
            );
            {
                let mut output = upstream.output.lock().unwrap();
                output["output"] = json!([]);
                output["usage"] =
                    json!({"input_tokens":12,"output_tokens":20000,"total_tokens":20012});
            }
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let mut body = file_search_request(streaming);
            body.as_object_mut().unwrap().remove("max_output_tokens");
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/responses")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            if budget < 3.205 {
                assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
                assert!(upstream.seen.lock().unwrap().is_empty());
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                let _ = test::read_body(response).await;
                assert_eq!(upstream.seen.lock().unwrap().len(), 1);
                assert!(
                    (state
                        .budget_limits
                        .providers
                        .get_provider_usage("native-test")
                        .unwrap()
                        .current_spend
                        - 2.0)
                        .abs()
                        < 1e-10
                );
            }
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn native_file_search_preserves_wire_and_settles_observed_calls_once() {
    use litellm_rs::core::{
        budget::{BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod},
        keys::CreateKeyConfig,
        types::context::RequestContext,
    };
    for streaming in [false, true] {
        for calls in 0..=2 {
            let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
            state.budget_limits.providers.set_provider_limit(
                "native-test",
                ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
            );
            state.budget_limits.models.set_model_limit(
                "gpt-4o-mini",
                ModelLimitConfig::new(1.0, ResetPeriod::Monthly),
            );
            let budget = state
                .budget_manager
                .create_budget(
                    BudgetScope::ApiKey("file-search".into()),
                    BudgetConfig::new("file search", 1.0),
                )
                .await
                .unwrap();
            let (key_id, _) = state
                .key_manager
                .generate_key(CreateKeyConfig {
                    name: "file-search".into(),
                    ..Default::default()
                })
                .await
                .unwrap();
            let output: Vec<Value> = (0..calls).map(|id| json!({"id":format!("fs_{id}"), "type":"file_search_call", "status":"completed", "queries":["support"], "results":null})).collect();
            upstream.output.lock().unwrap()["output"] = json!(output);
            let expected_response = upstream.output.lock().unwrap().clone();
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let body = file_search_request(streaming);
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(&body)
                .to_request();
            req.extensions_mut().insert(
                RequestContext::new()
                    .with_api_key(key_id)
                    .with_api_key_budget(budget.id.parse().unwrap()),
            );
            let response = test::call_service(&app, req).await;
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = test::read_body(response).await;
            if streaming {
                assert!(String::from_utf8_lossy(&bytes).contains(&expected_response.to_string()));
            } else {
                assert_eq!(
                    serde_json::from_slice::<Value>(&bytes).unwrap(),
                    expected_response
                );
            }
            assert_eq!(upstream.seen.lock().unwrap().as_slice(), &[body]);
            assert_eq!(upstream.count_seen.lock().unwrap().len(), 1);
            let expected =
                8.0 * 0.00000015 + 4.0 * 0.000000075 + 3.0 * 0.0000006 + f64::from(calls) * 0.0025;
            assert!(
                (state
                    .budget_limits
                    .providers
                    .get_provider_usage("native-test")
                    .unwrap()
                    .current_spend
                    - expected)
                    .abs()
                    < 1e-10
            );
            assert!(
                (state
                    .budget_limits
                    .models
                    .get_model_usage("gpt-4o-mini")
                    .unwrap()
                    .current_spend
                    - expected)
                    .abs()
                    < 1e-10
            );
            assert!(
                (state
                    .budget_manager
                    .get_budget(&BudgetScope::ApiKey("file-search".into()))
                    .unwrap()
                    .current_spend
                    - expected)
                    .abs()
                    < 1e-10
            );
            let usage = state.key_manager.get_usage_stats(key_id).await.unwrap();
            assert!((usage.total_cost - expected).abs() < 1e-10);
            assert_eq!(usage.total_tokens, 15);
            assert_eq!(usage.total_requests, 1);
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn native_file_search_reserves_retrieval_context_and_tool_fees_before_generation() {
    use litellm_rs::core::{
        budget::{BudgetConfig, BudgetScope, ProviderLimitConfig, ResetPeriod},
        types::context::RequestContext,
    };
    for key_budget in [false, true] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
        let context = if key_budget {
            let budget = state
                .budget_manager
                .create_budget(
                    BudgetScope::ApiKey("bounded-file-search".into()),
                    BudgetConfig::new("bounded file search", 0.03),
                )
                .await
                .unwrap();
            RequestContext::new().with_api_key_budget(budget.id.parse().unwrap())
        } else {
            state.budget_limits.providers.set_provider_limit(
                "native-test",
                ProviderLimitConfig::new(0.03, ResetPeriod::Monthly),
            );
            RequestContext::new()
        };
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(file_search_request(false))
            .to_request();
        req.extensions_mut().insert(context);
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_file_search_requires_a_bound_before_counting_or_generation() {
    for bound in [
        Value::Null,
        json!(0),
        json!(-1),
        json!(1.5),
        json!(4294967296_u64),
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = file_search_request(false);
        body["max_tool_calls"] = bound;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(upstream.count_seen.lock().unwrap().is_empty());
        assert!(upstream.seen.lock().unwrap().is_empty());
        handle.stop(false).await;
    }
}

#[tokio::test]
async fn native_file_search_unknown_calls_retain_reservation_and_unpriced_key_usage() {
    use litellm_rs::core::{
        budget::{ProviderLimitConfig, ResetPeriod},
        keys::CreateKeyConfig,
        types::context::RequestContext,
    };
    for streaming in [false, true] {
        for output in [
            Value::Null,
            json!([{"type":"file_search_call","id":"fs_1","status":"failed"}]),
            json!([{"type":"file_search_call","id":"fs_1","status":"completed"},{"type":"file_search_call","id":"fs_1","status":"completed"}]),
            json!([{"type":"file_search_call","id":"fs_1","status":"completed"},{"type":"file_search_call","id":"fs_2","status":"completed"},{"type":"file_search_call","id":"fs_3","status":"completed"}]),
        ] {
            let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
            state.budget_limits.providers.set_provider_limit(
                "native-test",
                ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
            );
            let (key_id, _) = state
                .key_manager
                .generate_key(CreateKeyConfig {
                    name: "unknown-file-search".into(),
                    ..Default::default()
                })
                .await
                .unwrap();
            upstream.output.lock().unwrap()["output"] = output;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let req = test::TestRequest::post()
                .uri("/v1/responses")
                .set_json(file_search_request(streaming))
                .to_request();
            req.extensions_mut()
                .insert(RequestContext::new().with_api_key(key_id));
            let facts = Arc::new(Mutex::new(
                litellm_rs::core::request_ledger::RequestLedgerFacts::default(),
            ));
            let response = litellm_rs::core::request_ledger::scope_facts(
                facts.clone(),
                test::call_service(&app, req),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let _ = test::read_body(response).await;
            let reserved =
                (12000.0 + 2.0 * 128000.0) * 0.00000015 + 16.0 * 0.0000006 + 2.0 * 0.0025;
            assert!(
                (state
                    .budget_limits
                    .providers
                    .get_provider_usage("native-test")
                    .unwrap()
                    .current_spend
                    - reserved)
                    .abs()
                    < 1e-10
            );
            let usage = state.key_manager.get_usage_stats(key_id).await.unwrap();
            assert_eq!(usage.unpriced_requests, 1);
            assert_eq!(usage.total_tokens, 15);
            assert_eq!(usage.total_cost, 0.0);
            let settled = litellm_rs::core::request_ledger::snapshot_facts(&facts);
            assert_eq!(settled.prompt_tokens, Some(12));
            assert_eq!(settled.completion_tokens, Some(3));
            assert_eq!(settled.total_tokens, Some(15));
            assert_eq!(settled.cost, None);
            handle.stop(false).await;
        }
    }
}

#[tokio::test]
async fn native_file_search_missing_price_or_context_never_generates() {
    for row in [
        json!({"litellm_provider":"openai","mode":"chat","input_cost_per_token":0.00000015,"output_cost_per_token":0.0000006}),
        json!({"litellm_provider":"openai","mode":"chat","max_input_tokens":128000,"max_output_tokens":16384}),
        json!({"litellm_provider":"openai","mode":"chat","max_input_tokens":128000,"input_cost_per_token":0.00000015,"output_cost_per_token":0.0000006}),
    ] {
        let (state, upstream, handle) = fixture(StatusCode::OK, |_| {}).await;
        state
            .pricing
            .add_custom_model("gpt-4o-mini".into(), serde_json::from_value(row).unwrap());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = file_search_request(false);
        body.as_object_mut().unwrap().remove("max_output_tokens");
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
}

#[tokio::test]
async fn native_foreground_completion_retains_unknown_usage_and_settles_known_usage_once() {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    for shared in [false, true] {
        let redis_url = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                eprintln!("Skipping native foreground shared cases: REDIS_URL is unset");
                continue;
            };
            Some(url)
        } else {
            None
        };
        for failed in [false, true] {
            for usage in [None, Some(0_u64), Some(15_u64)] {
                let observed_usage = usage;
                let retain_unknown = !failed && observed_usage.is_none();
                // Known positive usage settles the shared request reservation
                // even on provider failure. Local success RPM remains separate.
                let retain_request = !failed || observed_usage.is_some_and(|tokens| tokens > 0);
                let (state, upstream, handle) = fixture(StatusCode::OK, |config| {
                    if let Some(url) = &redis_url {
                        config.gateway.storage.redis.enabled = true;
                        config.gateway.storage.redis.allow_degraded = false;
                        config.gateway.storage.redis.url = url.clone();
                    }
                    config.gateway.providers[0].name =
                        format!("native-foreground-{}", uuid::Uuid::new_v4());
                    // The short input plus 64 output tokens fits once in 128 TPM.
                    // RPM, occupancy and the circuit cannot reject the second call.
                    config.gateway.providers[0].tpm = 128;
                    config.gateway.providers[0].rpm = 100;
                    config.gateway.providers[0].max_concurrent_requests = 100;
                    config.gateway.router.load_balancer.health_check_enabled = false;
                    config.gateway.router.circuit_breaker.failure_threshold = 100;
                    config.gateway.router.circuit_breaker.min_requests = 100;
                })
                .await;
                {
                    let mut output = upstream.output.lock().unwrap();
                    if failed {
                        output["status"] = json!("failed");
                        output["error"] =
                            json!({"code":"server_error","message":"generation failed"});
                    }
                    match usage {
                        None => output["usage"] = Value::Null,
                        Some(0) => {
                            output["usage"] =
                                json!({"input_tokens":0,"output_tokens":0,"total_tokens":0});
                        }
                        Some(15) => {}
                        _ => unreachable!(),
                    }
                }
                let router = state.unified_router();
                let ids = router.get_deployments_for_model("gpt-4o-mini");
                assert_eq!(ids.len(), 1);
                let deployment = router.get_deployment(&ids[0]).unwrap();
                let app = test::init_service(
                    App::new()
                        .app_data(web::Data::new(state))
                        .configure(litellm_rs::server::routes::ai::configure_routes),
                )
                .await;
                let mut redis = match &redis_url {
                    Some(url) => Some(
                        redis::Client::open(url.as_str())
                            .unwrap()
                            .get_multiplexed_async_connection()
                            .await
                            .unwrap(),
                    ),
                    None => None,
                };
                // Use Redis's clock for its fixed window, leaving ample time for
                // both HTTP requests and assertions before the minute rolls over.
                let second = if let Some(conn) = &mut redis {
                    let (seconds, _micros): (u64, u64) =
                        redis::cmd("TIME").query_async(conn).await.unwrap();
                    seconds % 60
                } else {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        % 60
                };
                if second >= 40 {
                    tokio::time::sleep(Duration::from_secs(61 - second)).await;
                }
                let body = json!({
                    "model":"gpt-4o-mini", "input":"Hello", "stream":false,
                    "store":false, "max_output_tokens":64
                });
                let first = test::call_service(
                    &app,
                    test::TestRequest::post()
                        .uri("/v1/responses")
                        .set_json(&body)
                        .to_request(),
                )
                .await;
                assert_eq!(first.status(), StatusCode::OK, "{failed}/{usage:?}");
                let value: Value = test::read_body_json(first).await;
                assert_eq!(value, *upstream.output.lock().unwrap());
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(
                    deployment.state.rpm_current.load(Ordering::Relaxed),
                    u64::from(!failed)
                );
                assert_eq!(
                    deployment.state.tpm_current.load(Ordering::Relaxed),
                    usage.unwrap_or(0),
                    "estimates must stay out of observed TPM"
                );
                assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
                assert_eq!(
                    deployment.state.success_requests.load(Ordering::Relaxed),
                    u64::from(!failed)
                );
                assert_eq!(
                    deployment.state.fail_requests.load(Ordering::Relaxed),
                    u64::from(failed)
                );
                assert_eq!(deployment.state.cooldown_until.load(Ordering::Relaxed), 0);
                assert_eq!(upstream.seen.lock().unwrap().len(), 1);

                let (admission_key, circuit_key, retained_tokens) = if let Some(conn) = &mut redis {
                    // Match only this fixture's UUID resource. Gateway identity is
                    // production-owned, so the test does not reconstruct its hash.
                    let pattern = format!("litellm-rs:admission:v1:{}:*", deployment.id);
                    let mut cursor = 0_u64;
                    let mut keys = Vec::new();
                    loop {
                        let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                            .arg(cursor)
                            .arg("MATCH")
                            .arg(&pattern)
                            .arg("COUNT")
                            .arg(100)
                            .query_async(conn)
                            .await
                            .unwrap();
                        keys.extend(batch);
                        cursor = next;
                        if cursor == 0 {
                            break;
                        }
                    }
                    keys.sort();
                    keys.dedup();
                    assert_eq!(keys.len(), 1, "exactly one owned foreground admission key");
                    let admission_key = keys.pop().unwrap();
                    let shared_id = admission_key
                        .strip_prefix("litellm-rs:admission:v1:")
                        .unwrap();
                    let circuit_key = format!("litellm-rs:circuit:v1:{shared_id}");
                    let admission: (i64, i64, i64) = redis::cmd("HMGET")
                        .arg(&admission_key)
                        .arg(&["p", "r", "t"])
                        .query_async(conn)
                        .await
                        .unwrap();
                    assert_eq!(
                        (admission.0, admission.1),
                        (0, i64::from(retain_request)),
                        "shared admission request ownership: failed={failed}, usage={usage:?}"
                    );
                    match observed_usage {
                        Some(tokens) => assert_eq!(admission.2, i64::try_from(tokens).unwrap()),
                        None if failed => assert_eq!(admission.2, 0),
                        None => assert!(
                            (65..=128).contains(&admission.2),
                            "accepted unknown usage retains input plus the requested output bound"
                        ),
                    }
                    let counts: (i64, i64) = redis::cmd("HMGET")
                        .arg(&circuit_key)
                        .arg(&["tot", "fail"])
                        .query_async(conn)
                        .await
                        .unwrap();
                    assert_eq!(counts, (1, i64::from(failed)));
                    (admission_key, circuit_key, admission.2)
                } else {
                    (String::new(), String::new(), 0)
                };

                let second = tokio::time::timeout(
                    Duration::from_secs(if retain_unknown { 1 } else { 5 }),
                    test::call_service(
                        &app,
                        test::TestRequest::post()
                            .uri("/v1/responses")
                            .set_json(&body)
                            .to_request(),
                    ),
                )
                .await;
                let completed = if retain_unknown {
                    assert!(second.is_err(), "unknown usage must retain TPM");
                    1
                } else {
                    let second =
                        second.expect("known usage must leave the next request admissible");
                    assert_eq!(second.status(), StatusCode::OK, "{failed}/{usage:?}");
                    let value: Value = test::read_body_json(second).await;
                    assert_eq!(value, *upstream.output.lock().unwrap());
                    2
                };
                assert_eq!(upstream.seen.lock().unwrap().len(), completed as usize);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(
                    deployment.state.rpm_current.load(Ordering::Relaxed),
                    if failed { 0 } else { completed }
                );
                assert_eq!(
                    deployment.state.total_requests.load(Ordering::Relaxed),
                    completed
                );
                assert_eq!(
                    deployment.state.tpm_current.load(Ordering::Relaxed),
                    usage.unwrap_or(0) * completed
                );
                assert_eq!(
                    deployment.state.success_requests.load(Ordering::Relaxed),
                    if failed { 0 } else { completed }
                );
                assert_eq!(
                    deployment.state.fail_requests.load(Ordering::Relaxed),
                    if failed { completed } else { 0 }
                );
                assert_eq!(deployment.state.cooldown_until.load(Ordering::Relaxed), 0);
                if let Some(conn) = &mut redis {
                    let counts: (i64, i64) = redis::cmd("HMGET")
                        .arg(&circuit_key)
                        .arg(&["tot", "fail"])
                        .query_async(conn)
                        .await
                        .unwrap();
                    let expected_requests = i64::try_from(completed).unwrap();
                    assert_eq!(
                        counts,
                        (
                            expected_requests,
                            if failed { expected_requests } else { 0 }
                        )
                    );
                    let admission: (i64, i64, i64) = redis::cmd("HMGET")
                        .arg(&admission_key)
                        .arg(&["p", "r", "t"])
                        .query_async(conn)
                        .await
                        .unwrap();
                    let expected_tokens = observed_usage.map_or(retained_tokens, |tokens| {
                        i64::try_from(tokens * completed).unwrap()
                    });
                    assert_eq!(
                        admission,
                        (
                            0,
                            i64::from(retain_request) * expected_requests,
                            expected_tokens
                        )
                    );
                    let fields: Vec<String> = redis::cmd("HKEYS")
                        .arg(&admission_key)
                        .query_async(conn)
                        .await
                        .unwrap();
                    assert!(!fields.iter().any(|field| field.starts_with("l:")));
                    let _: i64 = redis::cmd("DEL")
                        .arg(&[admission_key, circuit_key])
                        .query_async(conn)
                        .await
                        .unwrap();
                }
                handle.stop(false).await;
            }
        }
    }
}
