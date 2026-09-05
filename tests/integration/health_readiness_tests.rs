//! Cross-module readiness coverage using a real loopback health probe.

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use crate::common::providers::mock_provider_config;
    use actix_web::http::StatusCode;
    use actix_web::{App, HttpResponse, HttpServer, test, web};
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderHealthCheckConfig;
    use litellm_rs::core::providers::model_identity::MODEL_IDENTITY_MAPPINGS_KEY;
    use litellm_rs::core::router::HealthStatus;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::routes;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn successful_local_probe_makes_public_readiness_green() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("probe server should bind to loopback");
        let address = listener
            .local_addr()
            .expect("probe server should expose its address");
        let probe_server = HttpServer::new(|| {
            App::new().route(
                "/health",
                web::get().to(|| async { HttpResponse::NoContent().finish() }),
            )
        })
        .listen(listener)
        .expect("probe server should listen")
        .run();
        let probe_handle = probe_server.handle();
        let probe_task = tokio::spawn(probe_server);

        let mut config = Config::default();
        config.gateway.auth.enable_jwt = true;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.enterprise.audit_logging = true;
        let mut pricing_file =
            NamedTempFile::new().expect("readiness pricing fixture should be created");
        serde_json::to_writer(
            pricing_file.as_file_mut(),
            &json!({
                "readiness-model": {
                    "max_tokens": 4096,
                    "max_input_tokens": 4096,
                    "max_output_tokens": 1024,
                    "input_cost_per_token": 0.00001,
                    "output_cost_per_token": 0.00002,
                    "litellm_provider": "openai",
                    "mode": "chat"
                }
            }),
        )
        .expect("readiness pricing fixture should serialize");
        pricing_file
            .as_file()
            .sync_all()
            .expect("readiness pricing fixture should be flushed");
        config.gateway.pricing.source = Some(pricing_file.path().to_string_lossy().into_owned());
        let mut provider = mock_provider_config(
            "probe-primary",
            "openai",
            "sk-test",
            &format!("http://{address}"),
            vec!["readiness-model".to_string()],
        );
        provider.health_check = ProviderHealthCheckConfig {
            interval: 60,
            failure_threshold: 1,
            recovery_timeout: 1,
            endpoint: Some("health".to_string()),
            expected_codes: vec![204],
        };
        provider.settings.insert(
            MODEL_IDENTITY_MAPPINGS_KEY.to_string(),
            json!({
                "readiness-model": {
                    "capability_catalog_model": "gpt-4",
                    "pricing_model": "readiness-model"
                }
            }),
        );
        config.gateway.providers = vec![provider];

        let gateway = GatewayHttpServer::new(&config)
            .await
            .expect("gateway should start with the local probe");
        let state = gateway.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("in-memory migrations should succeed");
        let router = Arc::clone(&state.unified_router());
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let published = router
                    .get_deployment("probe-primary-readiness-model")
                    .is_some_and(|deployment| {
                        deployment.state.probe_health_status() == HealthStatus::Healthy
                    });
                if published {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("successful local probe should publish health");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .wrap(AuthMiddleware)
                .configure(routes::health::configure_routes),
        )
        .await;
        let response = test::call_service(
            &app,
            test::TestRequest::get().uri("/health/ready").to_request(),
        )
        .await;

        probe_handle.stop(true).await;
        probe_task
            .await
            .expect("probe task should finish cleanly")
            .expect("probe server should stop cleanly");
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["data"]["ready"], true);
        assert_eq!(body["data"]["reason"], "ok");
        assert!(body["data"].get("storage").is_none());
        assert!(body["data"].get("providers").is_none());
    }
}
