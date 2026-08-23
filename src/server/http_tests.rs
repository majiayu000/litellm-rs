//! Tests for `src/server/http.rs`.
//!
//! Extracted from the inline `mod tests` to keep the core file small.

use super::*;
use crate::config::Config;
use actix_web::{
    http::{StatusCode, header},
    test as actix_test,
};

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
