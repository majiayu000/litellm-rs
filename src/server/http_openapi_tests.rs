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

    let admin_request = actix_test::TestRequest::get()
        .uri("/admin/openapi.json")
        .to_request();
    let admin_response = actix_test::call_service(&app, admin_request).await;
    assert_eq!(admin_response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn app_factory_serves_admin_openapi_without_merging_into_inference_contract() {
    const SENTINEL_API_KEY: &str = "sk-must-not-appear-in-admin-openapi";
    const SENTINEL_PROVIDER_NAME: &str = "runtime-provider-must-not-appear-admin";

    let mut config = valid_http_test_config();
    config.gateway.providers[0].api_key = SENTINEL_API_KEY.to_string();
    config.gateway.providers[0].name = SENTINEL_PROVIDER_NAME.to_string();
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

    let admin_request = actix_test::TestRequest::get()
        .uri("/admin/openapi.json")
        .to_request();
    let admin_response = actix_test::call_service(&app, admin_request).await;
    assert_eq!(admin_response.status(), StatusCode::OK);
    assert_eq!(
        admin_response.headers().get(header::CONTENT_TYPE),
        Some(&header::HeaderValue::from_static("application/json"))
    );
    let admin_body = actix_test::read_body(admin_response).await;
    let admin_contract: serde_json::Value =
        serde_json::from_slice(&admin_body).expect("admin OpenAPI must be valid JSON");
    assert_eq!(admin_contract["openapi"], "3.2.0");
    assert!(admin_contract["paths"]["/admin/routing/inventory"]["get"].is_object());
    assert!(admin_contract["paths"]["/admin/request-ledger"]["get"].is_object());
    assert!(admin_contract["paths"]["/v1/keys"]["post"].is_object());
    assert!(admin_contract["paths"]["/v1/teams"]["get"].is_object());
    assert!(admin_contract["paths"]["/v1/budget/summary"]["get"].is_object());
    assert!(admin_contract["paths"]["/auth/login"]["post"].is_object());
    assert!(admin_contract["paths"].get("/admin/dashboard").is_none());
    let admin_text = String::from_utf8_lossy(&admin_body);
    assert!(!admin_text.contains(SENTINEL_API_KEY));
    assert!(!admin_text.contains(SENTINEL_PROVIDER_NAME));

    let inference_request = actix_test::TestRequest::get()
        .uri("/openapi.json")
        .to_request();
    let inference_response = actix_test::call_service(&app, inference_request).await;
    assert_eq!(inference_response.status(), StatusCode::OK);
    let inference_body = actix_test::read_body(inference_response).await;
    let inference_contract: serde_json::Value =
        serde_json::from_slice(&inference_body).expect("inference OpenAPI must be valid JSON");
    assert!(inference_contract["paths"]["/v1/chat/completions"]["post"].is_object());
    assert!(inference_contract["paths"].get("/admin").is_none());
    assert!(inference_contract["paths"].get("/admin/openapi.json").is_none());
    assert!(inference_contract["paths"].get("/v1/keys").is_none());
}
