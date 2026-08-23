use super::*;
use actix_web::body::MessageBody;
use litellm_rs::core::budget::{BudgetConfig, BudgetScope};
use litellm_rs::core::types::context::RequestContext;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

async fn route_key_budget_context(
    state: &litellm_rs::server::state::AppState,
) -> (BudgetScope, RequestContext) {
    let scope = BudgetScope::ApiKey(format!("gemini-route-{}", Uuid::new_v4()));
    let budget = state
        .budget_manager
        .create_budget(
            scope.clone(),
            BudgetConfig::new("Gemini route key budget", 100.0),
        )
        .await
        .expect("API key budget");
    let budget_id = Uuid::parse_str(&budget.id).expect("budget id should be a UUID");
    (scope, RequestContext::new().with_api_key_budget(budget_id))
}

#[tokio::test]
async fn gemini_sdk_route_executes_selected_runtime_provider_snapshot() {
    let selected = MockGeminiServer::launch().await;
    let backup = MockGeminiServer::launch().await;
    let replacement = MockGeminiServer::launch().await;
    let configured = |name: &str, base_url: &str, api_key: &str, header_value: &str| {
        let mut provider = gemini_provider(name, base_url, Vec::new());
        provider.api_key = api_key.to_string();
        provider
            .settings
            .insert("provider_name".to_string(), json!("gemini"));
        provider.settings.insert(
            "custom_headers".to_string(),
            json!({"X-Custom-Header": header_value}),
        );
        provider
    };
    let state = build_test_state(vec![
        configured(
            "zz-primary",
            &selected.base_url,
            "selected-runtime-key",
            "selected-runtime-header",
        ),
        configured("aa-backup", &backup.base_url, "backup-key", "backup-header"),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let mut replaced_config = state.config().as_ref().clone();
    replaced_config.gateway.providers = vec![configured(
        "zz-primary",
        &replacement.base_url,
        "replacement-key",
        "replacement-header",
    )];
    state.config.store(replaced_config);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let stream_response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(stream_response.status(), StatusCode::OK);
    let stream_body = test::read_body(stream_response).await;
    assert!(String::from_utf8_lossy(&stream_body).contains("usageMetadata"));
    let requests = selected.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path_and_query,
        "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=selected-runtime-key"
    );
    assert_eq!(
        requests[1].path_and_query,
        "/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent?alt=sse&key=selected-runtime-key"
    );
    assert_eq!(
        requests[0].headers["x-custom-header"],
        "selected-runtime-header"
    );
    assert!(backup.requests().is_empty());
    assert!(replacement.requests().is_empty());
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("selected runtime provider budget should exist");
    assert!(provider_usage.current_spend > 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("requested model budget should exist");
    assert!(model_usage.current_spend > 0.0);
    selected.shutdown().await;
    backup.shutdown().await;
    replacement.shutdown().await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("delayed upstream should bind");
    let address = listener.local_addr().expect("delayed upstream address");
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should connect");
        let mut request = [0_u8; 4096];
        socket
            .read_exact(&mut request[..1])
            .await
            .expect("request should read");
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}")
            .await
            .expect("response should write");
    });
    let mut selected = gemini_provider(
        "runtime-explicit",
        &format!("http://{address}"),
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    selected
        .settings
        .insert("provider_name".to_string(), json!("gemini"));
    selected.timeout = 2;
    let mut replacement = selected.clone();
    let state = build_test_state(vec![selected]).await;
    replacement.base_url = Some("http://127.0.0.1:9".to_string());
    replacement.timeout = 1;
    let mut replaced_config = state.config().as_ref().clone();
    replaced_config.gateway.providers = vec![replacement];
    state.config.store(replaced_config);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let started = Instant::now();
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(started.elapsed() >= Duration::from_millis(1_100));
    upstream.await.expect("delayed upstream should finish");
}

#[tokio::test]
async fn gemini_sdk_route_accepts_only_closed_runtime_alias_set() {
    let mock = MockGeminiServer::launch().await;
    for (index, name) in ["gemini", "Google-AI", "google_ai_studio"]
        .into_iter()
        .enumerate()
    {
        let state = build_test_state(vec![gemini_provider(name, &mock.base_url, Vec::new())]).await;
        let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
            litellm_rs::server::routes::ai::configure_routes(
                cfg,
                litellm_rs::config::models::default_max_body_size(),
            )
        }))
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "alias {name}");
        assert_eq!(mock.requests().len(), index + 1, "alias {name}");
    }

    let rejected = ["openai", "my-gemini-proxy", "g.e.m.i.n.i"]
        .into_iter()
        .map(|name| gemini_provider(name, &mock.base_url, Vec::new()))
        .collect();
    let state = build_test_state(rejected).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(mock.requests().len(), 3);
    mock.shutdown().await;
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn gemini_sdk_route_accepts_native_gemini_runtime() {
    let mock = MockGeminiServer::launch().await;
    let mut provider = gemini_provider(
        "native-gemini",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    provider.provider_type = "gemini".to_string();
    let state = build_test_state(vec![provider]).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(mock.requests().len(), 1);

    let mut unrelated = gemini_provider(
        "native-registry",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    unrelated.provider_type = "gemini".to_string();
    let state = build_test_state(vec![unrelated]).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-unrelated-model:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(mock.requests().len(), 1);
    mock.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_route_source_has_one_runtime_sender_and_no_config_adapter() {
    let route = include_str!("../../src/server/routes/ai/gemini.rs");
    let provider = include_str!("../../src/server/routes/ai/gemini/provider.rs");
    let spend = include_str!("../../src/server/routes/ai/gemini/spend.rs");
    let source = format!("{route}\n{provider}\n{spend}");
    for forbidden in [
        "state.config().providers()",
        ".gateway.providers",
        ".providers()",
        "RouteHttpClient",
        "ProviderConfig",
        "create_provider(",
        "BaseHttpClient",
        "reqwest::Client",
        "Client::new(",
        "GeminiClient::new(",
        "OpenAILikeProvider::new(",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden Gemini route source: {forbidden}"
        );
    }
    let adapter = provider
        .split("pub(super) struct GeminiRouteProvider")
        .nth(1)
        .expect("route adapter declaration")
        .split("impl GeminiRouteProvider")
        .next()
        .expect("route adapter fields");
    for sensitive in ["api_key", "base_url", "headers", "timeout", "client"] {
        assert!(!adapter.contains(sensitive), "adapter retains {sensitive}");
    }
    assert_eq!(source.matches(".gemini_generate_content(").count(), 1);
    assert_eq!(
        source
            .matches("selected_provider.gemini_generate_content(native_request)")
            .count(),
        1
    );
    for alternate_sender in [
        "Provider::gemini_generate_content(",
        "GeminiProvider::gemini_generate_content(",
        "OpenAILikeProvider::gemini_generate_content(",
        ".execute_request(",
        ".execute_streaming_request(",
        ".post(",
        ".request(",
    ] {
        assert!(
            !source.contains(alternate_sender),
            "alternate sender: {alternate_sender}"
        );
    }
}

#[tokio::test]
async fn gemini_sdk_stream_client_cancel_is_health_neutral_and_settles_observed_spend() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("cancel upstream should bind");
    let address = listener.local_addr().expect("cancel upstream address");
    let (continue_tx, continue_rx) = tokio::sync::oneshot::channel();
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should connect");
        let mut request = [0_u8; 4096];
        socket
            .read_exact(&mut request[..1])
            .await
            .expect("request should read");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
            )
            .await
            .expect("headers should write");
        let first = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"observed\"}]}}],",
            "\"usageMetadata\":{\"promptTokenCount\":10,\"candidatesTokenCount\":5,",
            "\"totalTokenCount\":15}}\n\n"
        );
        socket
            .write_all(format!("{:x}\r\n{first}\r\n", first.len()).as_bytes())
            .await
            .expect("observed chunk should write");
        continue_rx
            .await
            .expect("client cancellation should release upstream");
        let second = "data: {\"candidates\":[]}\n\n";
        let _ = socket
            .write_all(format!("{:x}\r\n{second}\r\n0\r\n\r\n", second.len()).as_bytes())
            .await;
    });
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &format!("http://{address}"),
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let deployment = state
        .unified_router
        .get_deployment("gemini-gemini-3.1-flash-lite")
        .expect("runtime deployment");
    let successes = deployment.state.success_requests.load(Ordering::Relaxed);
    let failures = deployment.state.fail_requests.load(Ordering::Relaxed);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body();
    let observed =
        futures::future::poll_fn(|context| MessageBody::poll_next(Pin::new(&mut body), context))
            .await
            .expect("stream should yield")
            .expect("observed chunk should be valid");
    assert!(String::from_utf8_lossy(&observed).contains("usageMetadata"));
    drop(body);
    continue_tx
        .send(())
        .expect("cancelled client should release upstream");
    upstream.await.expect("cancel upstream should finish");
    tokio::time::timeout(Duration::from_secs(2), async {
        while deployment.state.active_requests.load(Ordering::Relaxed) != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled stream lease should release");
    assert_eq!(
        deployment.state.success_requests.load(Ordering::Relaxed),
        successes
    );
    assert_eq!(
        deployment.state.fail_requests.load(Ordering::Relaxed),
        failures
    );
    assert!(
        budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("observed provider spend")
            .current_spend
            > 0.0
    );
    assert!(
        budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("observed model spend")
            .current_spend
            > 0.0
    );
}

#[tokio::test]
async fn gemini_sdk_route_network_error_does_not_leak_provider_key() {
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        "http://127.0.0.1:9",
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body: Value = test::read_body_json(response).await;
    let message = body["error"]["message"].as_str().expect("error message");
    assert!(message.contains("Gemini upstream request failed"));
    assert!(!message.contains("test-api-key-12345678901234567890"));
}

#[tokio::test]
async fn gemini_sdk_route_redacts_key_from_upstream_error_body() {
    let mock = MockGeminiServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
            .set_json(gemini_upstream_error_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = test::read_body(response).await;
    let text = String::from_utf8(body.to_vec()).expect("body should be utf8");
    assert!(text.contains("upstream failed"));
    assert!(text.contains("key=[REDACTED]"));
    assert!(!text.contains("test-api-key-12345678901234567890"));

    mock.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_route_does_not_charge_upstream_error_body() {
    let mock = MockGeminiServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &mock.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_upstream_error_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let stream_body = test::read_body(response).await;
    let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
    assert!(stream_text.contains("upstream failed"));
    assert!(stream_text.contains("key=[REDACTED]"));
    assert!(!stream_text.contains("test-api-key-12345678901234567890"));
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("provider budget should exist");
    assert_eq!(provider_usage.current_spend, 0.0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("model budget should exist");
    assert_eq!(model_usage.current_spend, 0.0);

    mock.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_route_settles_reserved_spend_after_output_then_read_error() {
    let broken = BrokenGeminiStreamServer::launch().await;
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &broken.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let expected_reservation = state
        .pricing
        .estimate_loaded_completion_cost_for_provider("gemini", "gemini-3.1-flash-lite", 7, Some(8))
        .expect("Gemini pricing")
        .max_cost;
    let (key_scope, key_context) = route_key_budget_context(&state).await;
    let budget_manager = state.budget_manager.clone();
    let budget_limits = state.budget_limits.clone();
    let deployment = state
        .unified_router
        .get_deployment("gemini-gemini-3.1-flash-lite")
        .expect("runtime deployment");
    let successes = deployment.state.success_requests.load(Ordering::Relaxed);
    let failures = deployment.state.fail_requests.load(Ordering::Relaxed);
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let request = test::TestRequest::post()
        .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
        .set_json(gemini_body())
        .to_request();
    request.extensions_mut().insert(key_context);
    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let stream_body = test::read_body(response).await;
    let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
    assert!(stream_text.contains("partial"));
    assert!(stream_text.contains("Gemini upstream stream error"));
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("provider budget should exist");
    assert!((provider_usage.current_spend - expected_reservation).abs() < f64::EPSILON);
    assert_eq!(provider_usage.request_count, 1);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("model budget should exist");
    assert_eq!(model_usage.current_spend, provider_usage.current_spend);
    assert_eq!(model_usage.request_count, 1);
    assert!(
        (budget_manager.get_current_spend(&key_scope) - expected_reservation).abs() < f64::EPSILON
    );
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(
        deployment.state.success_requests.load(Ordering::Relaxed),
        successes
    );
    assert_eq!(
        deployment.state.fail_requests.load(Ordering::Relaxed),
        failures + 1
    );

    broken.shutdown().await;
}

#[tokio::test]
async fn gemini_sdk_stream_heartbeat_before_read_error_does_not_charge_reservation() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("empty broken stream should bind");
    let address = listener.local_addr().expect("broken stream address");
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should connect");
        let mut request = [0_u8; 4096];
        socket
            .read_exact(&mut request[..1])
            .await
            .expect("request should read");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 4096\r\nconnection: close\r\n\r\n: keepalive\n\n",
            )
            .await
            .expect("headers should write");
        socket.shutdown().await.expect("stream should close");
    });
    let state = build_test_state(vec![gemini_provider(
        "gemini",
        &format!("http://{address}"),
        vec!["gemini-3.1-flash-lite".to_string()],
    )])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gemini-3.1-flash-lite",
        ModelLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let (key_scope, key_context) = route_key_budget_context(&state).await;
    let budget_manager = state.budget_manager.clone();
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let request = test::TestRequest::post()
        .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
        .set_json(gemini_body())
        .to_request();
    request.extensions_mut().insert(key_context);
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(test::read_body(response).await.to_vec())
        .expect("stream body should be utf8");
    assert!(body.contains(": keepalive"));
    assert!(!body.contains("partial"));
    assert!(body.contains("Gemini upstream stream error"));
    let provider_usage = budget_limits
        .providers
        .get_provider_usage("gemini")
        .expect("provider budget");
    assert_eq!(provider_usage.current_spend, 0.0);
    assert_eq!(provider_usage.request_count, 0);
    let model_usage = budget_limits
        .models
        .get_model_usage("gemini-3.1-flash-lite")
        .expect("model budget");
    assert_eq!(model_usage.current_spend, 0.0);
    assert_eq!(model_usage.request_count, 0);
    assert_eq!(budget_manager.get_current_spend(&key_scope), 0.0);
    upstream.await.expect("broken upstream should finish");
}

#[tokio::test]
async fn gemini_sdk_stream_body_is_not_cut_off_by_ordinary_timeout() {
    let delayed = DelayedGeminiStreamServer::launch(Duration::from_millis(1_200)).await;
    let mut provider = gemini_provider(
        "gemini",
        &delayed.base_url,
        vec!["gemini-3.1-flash-lite".to_string()],
    );
    provider.timeout = 1;
    let state = build_test_state(vec![provider]).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;
    let started = Instant::now();

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1beta/models/gemini-3.1-flash-lite:streamGenerateContent")
            .set_json(gemini_body())
            .to_request(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = tokio::time::timeout(Duration::from_secs(3), test::read_body(response))
        .await
        .expect("delayed SSE body should complete")
        .to_vec();
    let body = String::from_utf8(body).expect("stream body should be utf8");
    assert!(body.contains("delayed"));
    assert!(started.elapsed() >= Duration::from_millis(1_100));
    delayed.shutdown().await;
}
