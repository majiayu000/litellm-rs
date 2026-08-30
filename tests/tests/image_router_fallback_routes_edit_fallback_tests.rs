use super::*;
#[cfg(feature = "providers-extended")]
use litellm_rs::core::providers::Provider;
#[cfg(feature = "providers-extended")]
use litellm_rs::core::types::model::ProviderCapability;

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn image_edit_transport_uses_the_same_selected_deployment() {
    let proxy = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "stability-native",
            "stability",
            "http://127.0.0.1:1",
            vec!["inpaint".to_string()],
        ),
        openai_image_provider_with_mapping(
            "openai-proxy",
            &proxy.base_url,
            "inpaint",
            "gpt-image-1-mini",
        ),
    ])
    .await;
    add_raw_image_alias_pricing(&state, "inpaint", "gpt-image-1-mini");

    let first = state
        .unified_router
        .select_deployment_lease_for_capability("inpaint", &ProviderCapability::ImageEdit)
        .expect("first image edit deployment should be selectable");
    assert!(matches!(
        first.deployment().provider,
        Provider::Stability(_)
    ));
    drop(first);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-selected-transport-boundary";
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(boundary, "inpaint", 1))
            .to_request(),
    )
    .await;

    let status = resp.status();
    let response_body = test::read_body(resp).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected gateway response: {}",
        String::from_utf8_lossy(&response_body)
    );
    assert_eq!(proxy.paths(), vec!["/v1/images/edits".to_string()]);
    proxy.stop().await;
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn native_image_edit_precedes_wildcard_proxy_router_key() {
    let native = MockImageServer::start().await;
    let proxy = MockImageServer::start().await;
    let native_base = native.base_url.trim_end_matches("/v1");
    let state = build_route_policy_test_state_with_pricing(
        vec![
            image_provider(
                "stability-native",
                "stability",
                native_base,
                vec!["inpaint".to_string()],
            ),
            image_provider(
                "wild-proxy",
                "openai_compatible",
                &proxy.base_url,
                Vec::new(),
            ),
        ],
        Some(HashMap::from([(
            "inpaint".to_string(),
            flat_image_model_info_for_provider("stability", 0.06),
        )])),
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-native-before-wildcard";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(boundary, "inpaint", 1))
            .to_request(),
    )
    .await;

    let status = resp.status();
    let response_body = test::read_body(resp).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected gateway response: {}",
        String::from_utf8_lossy(&response_body)
    );
    assert_eq!(
        native.paths(),
        vec!["/v2beta/stable-image/edit/inpaint".to_string()]
    );
    assert!(
        proxy.paths().is_empty(),
        "wildcard proxy must remain unused"
    );
    native.stop().await;
    proxy.stop().await;
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn native_image_edit_rejects_explicit_quality_before_upstream_io() {
    let state = build_route_policy_test_state_with_pricing(
        vec![image_provider(
            "stability-native",
            "stability",
            "http://127.0.0.1:1",
            vec!["inpaint".to_string()],
        )],
        Some(HashMap::from([(
            "inpaint".to_string(),
            flat_image_model_info_for_provider("stability", 0.06),
        )])),
    )
    .await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    for quality in ["high", "hd"] {
        let boundary = format!("litellm-rs-native-quality-{quality}");
        let mut body = Vec::new();
        add_text_field(&mut body, &boundary, "model", "inpaint");
        add_text_field(&mut body, &boundary, "prompt", "make it lighter");
        add_text_field(&mut body, &boundary, "quality", quality);
        add_file_field(&mut body, &boundary, "image", "input.png", b"png-bytes");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let resp = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/images/edits")
                .insert_header((
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(body)
                .to_request(),
        )
        .await;
        let status = resp.status();
        let response_body = test::read_body(resp).await;

        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "quality={quality} reached upstream I/O: {}",
            String::from_utf8_lossy(&response_body)
        );
    }
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn accepted_bfl_image_jobs_settle_configured_provider_budget() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for task in ["edit", "generation"] {
            let (mut socket, _) = listener.accept().await.expect("submit should arrive");
            use tokio::io::AsyncReadExt as _;
            let mut request = vec![0_u8; 8192];
            let _ = socket
                .read(&mut request)
                .await
                .expect("submit request should read");
            let body =
                format!(r#"{{"id":"{task}","polling_url":"http://127.0.0.1:1/poll/{task}"}}"#);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            use tokio::io::AsyncWriteExt as _;
            socket
                .write_all(response.as_bytes())
                .await
                .expect("submit response should write");
        }
    });
    let state = build_route_policy_test_state_with_pricing(
        vec![image_provider(
            "bfl-primary",
            "black_forest_labs",
            &format!("http://{address}"),
            vec!["flux-kontext-pro".to_string(), "flux-pro-1.1".to_string()],
        )],
        Some(HashMap::from([
            (
                "flux-kontext-pro".to_string(),
                flat_image_model_info_for_provider("black_forest_labs", 0.06),
            ),
            (
                "flux-pro-1.1".to_string(),
                flat_image_model_info_for_provider("black_forest_labs", 0.06),
            ),
        ])),
    )
    .await;
    state.budget_limits.providers.set_provider_limit(
        "bfl-primary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-bfl-accepted-budget";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(
                boundary,
                "flux-kontext-pro",
                1,
            ))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    let generation_resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/generations")
            .set_json(json!({
                "model": "flux-pro-1.1",
                "prompt": "a glass city",
                "n": 1,
                "response_format": "url"
            }))
            .to_request(),
    )
    .await;
    assert_eq!(generation_resp.status(), StatusCode::BAD_GATEWAY);
    server.await.expect("mock server should finish");
    let configured_spend = budget_limits
        .providers
        .get_provider_usage("bfl-primary")
        .map(|usage| usage.current_spend)
        .unwrap_or_default();
    assert!((configured_spend - 0.12).abs() < f64::EPSILON);
    assert!(
        budget_limits
            .providers
            .get_provider_usage("black_forest_labs")
            .is_none(),
        "canonical provider identity must not receive configured deployment spend"
    );
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn accepted_stability_body_failure_settles_configured_provider_budget() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock Stability listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should arrive");
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let mut request = vec![0_u8; 8192];
        let _ = socket
            .read(&mut request)
            .await
            .expect("request should read");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 100\r\nConnection: close\r\n\r\ntruncated",
            )
            .await
            .expect("truncated response should write");
    });
    let mut provider = image_provider(
        "stability-primary",
        "stability",
        &format!("http://{address}"),
        vec!["stable-image-core".to_string()],
    );
    provider.max_retries = 0;
    let state = build_route_policy_test_state_with_pricing(
        vec![provider],
        Some(HashMap::from([(
            "stable-image-core".to_string(),
            flat_image_model_info_for_provider("stability", 0.06),
        )])),
    )
    .await;
    state.budget_limits.providers.set_provider_limit(
        "stability-primary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/generations")
            .set_json(json!({
                "model": "stable-image-core",
                "prompt": "a glass city",
                "n": 1,
                "response_format": "png"
            }))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    server.await.expect("mock server should finish");
    let configured_spend = budget_limits
        .providers
        .get_provider_usage("stability-primary")
        .map(|usage| usage.current_spend)
        .unwrap_or_default();
    assert!((configured_spend - 0.06).abs() < f64::EPSILON);
}

#[tokio::test]
async fn image_edit_records_flat_output_image_spend_after_success() {
    let mock = MockImageServer::start().await;
    let state = build_route_policy_test_state_with_pricing(
        vec![openai_image_provider_with_mapping(
            "openai-primary",
            &mock.base_url,
            "flat-image-model",
            "flat-image-model",
        )],
        Some(HashMap::from([(
            "flat-image-model".to_string(),
            flat_image_model_info(0.06),
        )])),
    )
    .await;
    state.budget_limits.providers.set_provider_limit(
        "openai-primary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let budget_limits = state.budget_limits.clone();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-flat-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body_for_model(
                boundary,
                "flat-image-model",
                2,
            ))
            .to_request(),
    )
    .await;

    let status = resp.status();
    let response_body = test::read_body(resp).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected gateway response: {}",
        String::from_utf8_lossy(&response_body)
    );
    assert_eq!(mock.paths(), vec!["/v1/images/edits".to_string()]);
    let upstream_bodies = mock.bodies();
    let upstream_body = String::from_utf8_lossy(&upstream_bodies[0]);
    assert!(upstream_body.contains("name=\"model\"\r\n\r\nflat-image-model"));
    let spent = budget_limits
        .providers
        .get_provider_usage("openai-primary")
        .map(|usage| usage.current_spend)
        .unwrap_or_default();
    assert!((spent - 0.12).abs() < f64::EPSILON);
    mock.stop().await;
}

#[tokio::test]
async fn native_openai_image_edit_uses_selected_provider_config_after_budget_fallback() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "openai-primary",
            "openai",
            &exhausted.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
        image_provider(
            "openai-secondary",
            "openai",
            &fallback.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "openai-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("openai-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "openai-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let boundary = "litellm-rs-image-boundary";
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}

#[tokio::test]
async fn wildcard_openai_compatible_image_edit_tries_next_provider_name_key() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "wild-primary",
            "openai_compatible",
            &exhausted.base_url,
            Vec::new(),
        ),
        image_provider(
            "wild-secondary",
            "openai_compatible",
            &fallback.base_url,
            Vec::new(),
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "wild-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("wild-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "wild-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let boundary = "litellm-rs-image-boundary";
    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}

#[tokio::test]
async fn explicit_image_provider_falls_back_to_wildcard_provider() {
    let exhausted = MockImageServer::start().await;
    let fallback = MockImageServer::start().await;
    let state = build_route_policy_test_state(vec![
        image_provider(
            "explicit-primary",
            "openai_compatible",
            &exhausted.base_url,
            vec!["gpt-image-1-mini".to_string()],
        ),
        image_provider(
            "wild-secondary",
            "openai_compatible",
            &fallback.base_url,
            Vec::new(),
        ),
    ])
    .await;
    state.budget_limits.providers.set_provider_limit(
        "explicit-primary",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    state
        .budget_limits
        .providers
        .record_provider_spend("explicit-primary", 2.0);
    state.budget_limits.providers.set_provider_limit(
        "wild-secondary",
        ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
    );
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;
    let boundary = "litellm-rs-image-boundary";

    let resp = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/images/edits")
            .insert_header((
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            ))
            .set_payload(image_edit_multipart_body(boundary))
            .to_request(),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["data"][0]["url"],
        "https://images.example.test/edit.png"
    );
    assert!(exhausted.paths().is_empty());
    assert_eq!(fallback.paths(), vec!["/v1/images/edits".to_string()]);

    exhausted.stop().await;
    fallback.stop().await;
}
