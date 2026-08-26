use std::time::Duration;

use actix_web::{http::StatusCode, test as actix_test, web};

use super::*;
use crate::core::traits::integration::{LlmEndEvent, LlmStartEvent};

#[tokio::test]
async fn metrics_endpoint_exposes_live_callback_sample_from_same_runtime() {
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
    config
        .gateway
        .providers
        .push(crate::config::models::provider::ProviderConfig {
            name: "metrics-test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-metrics-test".to_string(),
            ..Default::default()
        });
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;

    let server = HttpServer::new(&config)
        .await
        .unwrap_or_else(|error| panic!("server startup failed: {error}"));
    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let health_req = actix_test::TestRequest::get().uri("/health").to_request();
    let health_resp = actix_test::call_service(&app, health_req).await;
    assert_eq!(health_resp.status(), StatusCode::OK);
    drop(actix_test::read_body(health_resp).await);

    let start = LlmStartEvent::new("live-request", "gpt-live").provider("openai-live");
    let metrics = server
        .state()
        .callbacks
        .begin_llm_metrics(&start)
        .expect("live Prometheus metrics lifecycle should start");
    metrics.emit_end(&LlmEndEvent::new("live-request", "gpt-live").provider("openai-live"));

    let expected = "litellm_requests_total{model=\"gpt-live\",provider=\"openai-live\"} 1";
    let body = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let request = actix_test::TestRequest::get().uri("/metrics").to_request();
            let response = actix_test::call_service(&app, request).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = String::from_utf8(actix_test::read_body(response).await.to_vec())
                .expect("metrics response should be UTF-8");
            if body.contains(expected) {
                break body;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("live callback metric should become scrape-visible");

    assert!(body.contains("gateway_http_requests_total 1"));
    assert!(body.contains("gateway_http_responses_total{class=\"2xx\"} 1"));
    assert!(body.contains(
        "gateway_unpriced_events_total{provider=\"metrics-http-provider\",model_bucket=\"other\",policy=\"reject\",outcome=\"reject_preflight\"} 1"
    ));
    assert!(!body.contains("tenant-http-private-model"));
}
