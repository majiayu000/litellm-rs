#[tokio::test]
async fn app_factory_serves_public_stable_inference_openapi_contract() {
    const SENTINEL_API_KEY: &str = "sk-must-not-appear-in-openapi";
    const SENTINEL_PROVIDER_NAME: &str = "runtime-provider-must-not-appear";

    let mut config = valid_http_test_config();
    config.gateway.providers[0].api_key = SENTINEL_API_KEY.to_string();
    config.gateway.providers[0].name = SENTINEL_PROVIDER_NAME.to_string();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
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

    let request = actix_test::TestRequest::get()
        .uri("/openapi.json")
        .to_request();
    let response = actix_test::call_service(&app, request).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static("application/json"))
    );
    let body = actix_test::read_body(response).await;
    let contract: serde_json::Value =
        serde_json::from_slice(&body).expect("OpenAPI response must be valid JSON");

    assert_eq!(contract["openapi"], "3.2.0");
    assert!(contract["paths"]["/v1/chat/completions"]["post"].is_object());
    assert!(contract["paths"]["/v1/responses/{response_id}"]["get"].is_object());
    assert!(contract["paths"]["/v1/models"]["get"].is_object());
    assert!(contract["paths"].get("/admin").is_none());
    let body = String::from_utf8_lossy(&body);
    assert!(!body.contains(SENTINEL_API_KEY));
    assert!(!body.contains(SENTINEL_PROVIDER_NAME));
}
