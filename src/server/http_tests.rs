//! Tests for `src/server/http.rs`.
//!
//! Extracted from the inline `mod tests` to keep the core file small.

use super::*;
use crate::config::Config;
use actix_web::{
    http::{StatusCode, header},
    test as actix_test,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct RunningProductionServer {
    address: std::net::SocketAddr,
    handle: actix_web::dev::ServerHandle,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl RunningProductionServer {
    async fn start(server_config: ServerConfig) -> Self {
        let mut config = Config::default();
        config.gateway.server = server_config;
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.monitoring.metrics.enabled = false;

        let gateway = HttpServer::new(&config)
            .await
            .expect("production server state should initialize");
        let settings = HttpServer::validated_listener_settings(gateway.config())
            .expect("listener settings should validate");
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("production-path test listener should bind");
        let (server, addresses) = HttpServer::build_actix_server(
            web::Data::new(gateway.state().clone()),
            &settings,
            ServerBind::Listener(listener),
        )
        .expect("production Actix builder should accept the test listener");
        let address = *addresses
            .first()
            .expect("production Actix builder should report its listener address");
        let handle = server.handle();
        let task = tokio::spawn(server);
        wait_until_production_server_is_ready(address).await;
        Self {
            address,
            handle,
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        self.handle.stop(true).await;
        let result = self
            .task
            .take()
            .expect("server task should be present")
            .await
            .expect("server task should join");
        result.expect("server should stop cleanly");
    }
}

async fn wait_until_production_server_is_ready(address: std::net::SocketAddr) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(mut stream) = tokio::net::TcpStream::connect(address).await
            && stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .is_ok()
        {
            let mut response = Vec::new();
            if let Ok(Ok(_)) =
                tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut response))
                    .await
                && String::from_utf8_lossy(&response).contains(" 200 ")
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
                return;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "production-path test server did not become ready at {address}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

impl Drop for RunningProductionServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn open_incomplete_request(address: std::net::SocketAddr) -> tokio::net::TcpStream {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("test client should connect");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n")
        .await
        .expect("test client should write partial request head");
    stream
}

async fn open_keep_alive_request(address: std::net::SocketAddr) -> tokio::net::TcpStream {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("keep-alive client should connect");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("keep-alive client should write a complete request");
    let mut response = vec![0_u8; 2048];
    let bytes_read = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("keep-alive health request should receive a response")
        .expect("keep-alive health response should be readable");
    assert!(
        String::from_utf8_lossy(&response[..bytes_read]).contains(" 200 "),
        "keep-alive health request should succeed"
    );
    stream
}

#[test]
fn listener_settings_preserve_uncapped_workers_and_head_timeout() {
    let config = ServerConfig {
        workers: Some(4),
        max_connections: None,
        timeout: 17,
        ..ServerConfig::default()
    };

    let settings = HttpServer::validated_listener_settings(&config)
        .expect("valid listener settings should be derived");
    assert_eq!(settings.configured_workers, 4);
    assert_eq!(settings.effective_workers, 4);
    assert_eq!(
        settings.first_request_head_timeout,
        std::time::Duration::from_secs(17)
    );
    assert_eq!(settings.max_connections_per_worker, None);
}

#[test]
fn listener_settings_keep_safe_per_worker_limit_when_workers_exceed_capacity() {
    let config = ServerConfig {
        workers: Some(4),
        max_connections: Some(2),
        ..ServerConfig::default()
    };

    let settings = HttpServer::validated_listener_settings(&config)
        .expect("valid listener settings should be derived");
    assert_eq!(settings.configured_workers, 4);
    assert_eq!(settings.effective_workers, 1);
    assert_eq!(settings.max_connections_per_worker, Some(2));
}

#[test]
fn listener_settings_round_down_without_exceeding_server_wide_cap() {
    let config = ServerConfig {
        workers: Some(4),
        max_connections: Some(10),
        ..ServerConfig::default()
    };

    let settings = HttpServer::validated_listener_settings(&config)
        .expect("valid listener settings should be derived");
    let per_worker = settings
        .max_connections_per_worker
        .expect("configured cap should produce a per-worker cap");
    assert_eq!(settings.configured_workers, 4);
    assert_eq!(settings.effective_workers, 4);
    assert_eq!(per_worker, 2);
    assert_eq!(per_worker * settings.effective_workers, 8);
    assert!(per_worker * settings.effective_workers <= config.max_connections.unwrap_or_default());
}

#[test]
fn listener_settings_reject_invalid_custom_server_configs() {
    let invalid_configs = [
        ServerConfig {
            workers: Some(0),
            ..ServerConfig::default()
        },
        ServerConfig {
            max_connections: Some(0),
            ..ServerConfig::default()
        },
        ServerConfig {
            max_connections: Some(1),
            ..ServerConfig::default()
        },
        ServerConfig {
            timeout: 0,
            ..ServerConfig::default()
        },
        ServerConfig {
            max_body_size: 0,
            ..ServerConfig::default()
        },
    ];

    for config in invalid_configs {
        let error = HttpServer::validated_listener_settings(&config)
            .expect_err("invalid custom config must fail before listener construction");
        assert!(matches!(error, GatewayError::Config(_)));
    }

    let unsafe_connection_error = HttpServer::validated_listener_settings(&ServerConfig {
        max_connections: Some(1),
        ..ServerConfig::default()
    })
    .expect_err("an Actix per-worker limit below 2 must fail before listener construction");
    assert!(
        unsafe_connection_error
            .to_string()
            .contains("must be at least 2")
    );
}

#[tokio::test]
async fn production_builder_applies_first_request_head_timeout() {
    let running = RunningProductionServer::start(ServerConfig {
        workers: Some(1),
        timeout: 1,
        ..ServerConfig::default()
    })
    .await;
    let mut stream = open_incomplete_request(running.address).await;
    let mut response = vec![0_u8; 1024];
    let bytes_read = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut response))
        .await
        .expect("production first-request-head timeout should fire")
        .expect("timed-out connection should remain readable long enough for its response");
    let response = String::from_utf8_lossy(&response[..bytes_read]);
    assert!(
        response.contains(" 408 "),
        "expected a 408 from the production Actix builder, got: {response:?}"
    );
    running.stop().await;
}

#[tokio::test]
async fn production_builder_enforces_server_wide_connection_cap_across_workers() {
    let running = RunningProductionServer::start(ServerConfig {
        workers: Some(2),
        max_connections: Some(4),
        timeout: 10,
        ..ServerConfig::default()
    })
    .await;

    let mut occupied = Vec::new();
    for _ in 0..4 {
        occupied.push(open_keep_alive_request(running.address).await);
    }
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut queued = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("queued client should reach the TCP backlog");
    queued
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("queued client should write a complete request");
    let mut response = vec![0_u8; 2048];
    assert!(
        tokio::time::timeout(Duration::from_secs(1), queued.read(&mut response))
            .await
            .is_err(),
        "a fifth request must not be served while the server-wide cap of four is occupied"
    );

    drop(occupied);
    let bytes_read = tokio::time::timeout(Duration::from_secs(3), queued.read(&mut response))
        .await
        .expect("queued request should resume after capacity is released")
        .expect("queued request should receive a response");
    let response = String::from_utf8_lossy(&response[..bytes_read]);
    assert!(
        response.contains(" 200 "),
        "expected queued health request to succeed, got: {response:?}"
    );
    running.stop().await;
}

#[tokio::test]
async fn new_rejects_invalid_server_config_before_initializing_dependencies() {
    let mut config = Config::default();
    config.gateway.server.workers = Some(0);

    let error = match HttpServer::new(&config).await {
        Ok(_) => panic!("invalid server config must fail at the start of construction"),
        Err(error) => error,
    };
    match error {
        GatewayError::Config(message) => assert!(message.contains("Worker count")),
        other => panic!("expected config error, got: {other:?}"),
    }
}

#[test]
fn build_cors_rejects_wildcard_with_credentials() {
    let cors_config = CorsConfig {
        allowed_origins: vec!["*".to_string()],
        allow_credentials: true,
        ..Default::default()
    };

    let error = match HttpServer::build_cors(&cors_config) {
        Ok(_) => panic!("invalid CORS configuration should be rejected"),
        Err(error) => error,
    };

    match error {
        GatewayError::Config(message) => {
            assert!(message.contains("Invalid CORS configuration"));
            assert!(message.contains("credentials"));
        }
        other => panic!("expected config error, got: {other:?}"),
    }
}

#[test]
fn app_factory_cors_builder_falls_back_without_panicking() {
    let cors_config = CorsConfig {
        allowed_origins: vec!["*".to_string()],
        allow_credentials: true,
        ..Default::default()
    };

    let _cors = HttpServer::build_cors_for_app_factory(&cors_config);
}

#[tokio::test]
async fn new_rejects_invalid_cors_config_before_startup() {
    let mut config = Config::default();
    config.gateway.server.cors.allowed_origins = vec!["*".to_string()];
    config.gateway.server.cors.allow_credentials = true;

    let error = match HttpServer::new(&config).await {
        Ok(_) => panic!("server startup should reject invalid CORS configuration"),
        Err(error) => error,
    };

    match error {
        GatewayError::Config(message) => {
            assert!(message.contains("Invalid CORS configuration"));
            assert!(message.contains("credentials"));
        }
        other => panic!("expected config error, got: {other:?}"),
    }
}

/// Build a config whose only optional dependency that is *configured* is
/// the pricing source, which points at a non-existent file so the initial
/// load fails deterministically.
fn config_with_broken_pricing(allow_degraded: bool) -> Config {
    let mut config = Config::default();
    // Disable enterprise/storage subsystems that would require real I/O.
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source =
        Some("/nonexistent/path/that/cannot/be/loaded.json".to_string());
    config.gateway.pricing.allow_degraded = allow_degraded;
    config
}

#[tokio::test]
async fn new_fails_when_pricing_source_broken_and_not_allowed_to_degrade() {
    let config = config_with_broken_pricing(false);
    let result = HttpServer::new(&config).await;
    assert!(
        result.is_err(),
        "pricing source load failure with allow_degraded=false must fail startup"
    );
}

#[tokio::test]
async fn new_succeeds_when_pricing_source_broken_but_allowed_to_degrade() {
    let config = config_with_broken_pricing(true);
    let result = HttpServer::new(&config).await;
    assert!(
        result.is_ok(),
        "pricing source load failure with allow_degraded=true must keep startup running, \
         got: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn new_wires_enabled_cache_config() {
    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.cache.enabled = true;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("enabled cache should wire runtime cache: {error}"),
    };

    assert!(server.state().response_cache.is_some());
}

#[tokio::test]
async fn new_wires_configured_callback_backend() {
    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.monitoring.callbacks.backends = vec![
        crate::config::models::monitoring::CallbackBackendConfig::OpenTelemetry(
            crate::core::integrations::OpenTelemetryConfig::default(),
        ),
    ];

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("configured callbacks should wire at startup: {error}"),
    };

    assert!(server.state().callbacks.is_enabled());
    assert_eq!(
        server.state().callbacks.registered_integrations().await,
        vec!["opentelemetry"]
    );
}

/// In-memory budget snapshots load succeeds (returns empty) on sqlite, so
/// we can't trigger a real "load failed" path from `Config::default()`
/// alone without a mock. The disabled-DB branch is covered here: when the
/// database is disabled we use the in-memory sqlite backend which always
/// returns an empty snapshot set, exercising the "Ok(snapshots)" arm.
#[tokio::test]
async fn new_succeeds_with_in_memory_budgets_when_db_disabled() {
    let mut config = Config::default();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    // Disable pricing so we don't conflate with the broken-pricing tests.
    config.gateway.pricing.source = None;

    let result = HttpServer::new(&config).await;
    assert!(
        result.is_ok(),
        "disabled DB must keep startup running with in-memory budgets, got: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn app_factory_metrics_endpoint_includes_recorded_http_requests() {
    let _metrics_guard = MetricsMiddleware::test_lock().await;
    MetricsMiddleware::reset_for_tests();
    crate::server::middleware::reset_unpriced_metrics_for_tests();
    crate::server::middleware::record_unpriced_event(
        "metrics-http-provider",
        "tenant-http-private-model",
        "reject",
        "reject_preflight",
    );

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let health_req = actix_test::TestRequest::get().uri("/health").to_request();
    let health_resp = actix_test::call_service(&app, health_req).await;
    assert_eq!(health_resp.status(), StatusCode::OK);
    drop(actix_test::read_body(health_resp).await);

    let metrics_req = actix_test::TestRequest::get().uri("/metrics").to_request();
    let metrics_resp = actix_test::call_service(&app, metrics_req).await;
    assert_eq!(metrics_resp.status(), StatusCode::OK);

    let body = actix_test::read_body(metrics_resp).await;
    let body = match std::str::from_utf8(&body) {
        Ok(body) => body,
        Err(error) => panic!("metrics response was not utf-8: {error}"),
    };

    assert!(body.contains("gateway_http_requests_total 1"));
    assert!(body.contains("gateway_http_responses_total{class=\"2xx\"} 1"));
    assert!(body.contains(
        "gateway_unpriced_events_total{provider=\"metrics-http-provider\",model_bucket=\"other\",policy=\"reject\",outcome=\"reject_preflight\"} 1"
    ));
    assert!(!body.contains("tenant-http-private-model"));

    let rendered_after_scrape = MetricsMiddleware::render_prometheus();
    assert!(rendered_after_scrape.contains("gateway_http_requests_total 1"));
}

#[tokio::test]
async fn app_factory_metrics_records_auth_rejections_before_handler() {
    let _metrics_guard = MetricsMiddleware::test_lock().await;
    MetricsMiddleware::reset_for_tests();

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.rate_limit.enabled = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let models_req = actix_test::TestRequest::get()
        .uri("/v1/models")
        .to_request();
    match actix_test::try_call_service(&app, models_req).await {
        Ok(response) => {
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            drop(actix_test::read_body(response).await);
        }
        Err(error) => assert_eq!(
            error.as_response_error().status_code(),
            StatusCode::UNAUTHORIZED
        ),
    }

    let body = MetricsMiddleware::render_prometheus();
    assert!(body.contains("gateway_http_requests_total 1"));
    assert!(body.contains("gateway_http_request_errors_total 1"));
    assert!(body.contains("gateway_http_responses_total{class=\"4xx\"} 1"));
}

include!("http_cors_tests.rs");

#[tokio::test]
async fn app_factory_does_not_collect_http_metrics_when_metrics_disabled() {
    let _metrics_guard = MetricsMiddleware::test_lock().await;
    MetricsMiddleware::reset_for_tests();

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.monitoring.metrics.enabled = false;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let health_req = actix_test::TestRequest::get().uri("/health").to_request();
    let health_resp = actix_test::call_service(&app, health_req).await;
    assert_eq!(health_resp.status(), StatusCode::OK);
    drop(actix_test::read_body(health_resp).await);

    let body = MetricsMiddleware::render_prometheus();
    assert!(body.contains("gateway_http_requests_total 0"));
    assert!(body.contains("gateway_http_responses_total{class=\"2xx\"} 0"));
}

#[tokio::test]
async fn app_factory_mounts_explicit_cache_admin_surface() {
    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.monitoring.metrics.enabled = false;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let req = actix_test::TestRequest::get()
        .uri("/admin/cache/status")
        .to_request();
    let resp = actix_test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

    let body: serde_json::Value = actix_test::read_body_json(resp).await;
    assert_eq!(body["status"], "unsupported");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not wired")
    );
}

#[tokio::test]
async fn app_factory_enforces_configured_max_body_size_on_json_bodies() {
    // Serialize against tests that assert on the process-wide metric
    // counters; these requests pass through the same middleware stack.
    let _metrics_guard = MetricsMiddleware::test_lock().await;

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    // Tiny limit so the test payload clearly exceeds it.
    config.gateway.server.max_body_size = 1024;
    // Keep parallel tests from racing on the global metrics counters.
    config.gateway.monitoring.metrics.enabled = false;

    let server = match HttpServer::new(&config).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };
    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    // Oversized body must be rejected by the JSON extractor itself.
    let oversized = format!(
        r#"{{"model":"test-model","messages":[{{"role":"user","content":"{}"}}]}}"#,
        "x".repeat(4096)
    );
    let request = actix_test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(oversized)
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(response).await;
    let body = String::from_utf8_lossy(&body);
    assert!(
        body.contains("Invalid JSON request body"),
        "expected extractor-level rejection, got: {body}"
    );

    // A body under the configured limit must pass extraction.
    let small = r#"{"model":"test-model","messages":[{"role":"user","content":"hi"}]}"#;
    let request = actix_test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(small)
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    let body = actix_test::read_body(response).await;
    let body = String::from_utf8_lossy(&body);
    assert!(
        !body.contains("Invalid JSON request body"),
        "small body should pass extraction, got: {body}"
    );
}

#[tokio::test]
async fn app_factory_enforces_max_body_size_on_non_ai_json_routes() {
    let _metrics_guard = MetricsMiddleware::test_lock().await;

    let mut config = Config::default();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.server.max_body_size = 1024;
    config.gateway.monitoring.metrics.enabled = false;

    let server = HttpServer::new(&config)
        .await
        .expect("server should initialize for app-level body-limit test");
    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let oversized = serde_json::json!({
        "username": "body-limit-user",
        "password": "x".repeat(4096),
    });
    let request = actix_test::TestRequest::post()
        .uri("/auth/login")
        .set_json(oversized)
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let request = actix_test::TestRequest::post()
        .uri("/auth/login")
        .set_json(serde_json::json!({
            "username": "body-limit-user",
            "password": "invalid-password",
        }))
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    assert_ne!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
