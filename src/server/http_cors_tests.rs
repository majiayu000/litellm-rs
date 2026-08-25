fn cors_auth_test_config() -> Config {
    let mut config = valid_http_test_config();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.rate_limit.enabled = true;
    config.gateway.server.cors.enabled = true;
    config.gateway.server.cors.allowed_origins = vec!["https://app.example".to_string()];
    config.gateway.monitoring.metrics.enabled = false;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config
}

fn disabled_cors_auth_test_config() -> Config {
    let mut config = cors_auth_test_config();
    config.gateway.server.cors.enabled = false;
    config
}

#[tokio::test]
async fn app_factory_cors_preflight_runs_before_auth() {
    let server = match HttpServer::new(&cors_auth_test_config()).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let req = actix_test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/v1/chat/completions")
        .insert_header((header::ORIGIN, "https://app.example"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .insert_header((
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            "authorization,content-type",
        ))
        .to_request();
    let resp = actix_test::call_service(&app, req).await;

    assert!(
        resp.status().is_success(),
        "preflight should be handled by CORS, got {}",
        resp.status()
    );
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("https://app.example")
    );
    assert!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .is_some(),
        "preflight response should include allowed methods"
    );
    assert!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_HEADERS)
            .is_some(),
        "preflight response should include allowed headers"
    );
}

#[tokio::test]
async fn app_factory_unauthenticated_post_still_401_with_cors_headers() {
    let server = match HttpServer::new(&cors_auth_test_config()).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let req = actix_test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header((header::ORIGIN, "https://app.example"))
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload("{}")
        .to_request();
    let resp = actix_test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|value| value.to_str().ok()),
        Some("https://app.example")
    );
}

#[tokio::test]
async fn app_factory_non_preflight_options_still_requires_auth() {
    let server = match HttpServer::new(&cors_auth_test_config()).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let missing_requested_method = actix_test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/v1/chat/completions")
        .insert_header((header::ORIGIN, "https://app.example"))
        .to_request();
    let resp = actix_test::call_service(&app, missing_requested_method).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let missing_origin = actix_test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/v1/chat/completions")
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .to_request();
    let resp = actix_test::call_service(&app, missing_origin).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn app_factory_disabled_cors_preflight_still_requires_auth() {
    let server = match HttpServer::new(&disabled_cors_auth_test_config()).await {
        Ok(server) => server,
        Err(error) => panic!("server startup failed: {error}"),
    };

    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let req = actix_test::TestRequest::default()
        .method(actix_web::http::Method::OPTIONS)
        .uri("/v1/chat/completions")
        .insert_header((header::ORIGIN, "https://app.example"))
        .insert_header((header::ACCESS_CONTROL_REQUEST_METHOD, "POST"))
        .to_request();
    let resp = actix_test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none()
    );
}
