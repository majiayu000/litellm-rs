#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "gemini_sdk_routes/support.rs"]
mod support;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::support::{
        BrokenGeminiStreamServer, DelayedGeminiStreamServer, MockGeminiServer,
        api_key_with_invalid_runtime_permissions, api_key_with_max_tokens_per_request,
        build_auth_required_state, build_test_state, gemini_body,
        gemini_body_without_generation_config, gemini_provider, gemini_upstream_error_body,
    };
    use actix_web::{App, HttpMessage, dev::Service};
    use actix_web::{http::StatusCode, test, web};
    use litellm_rs::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::models::ApiKey;
    use litellm_rs::core::net::ProviderEndpointAccess;
    use serde_json::{Value, json};
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn gemini_sdk_routes_without_provider_fail_closed() {
        let state = build_test_state(Vec::new()).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let body: Value = test::read_body_json(response).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("Gemini SDK route provider")
        );
    }

    #[tokio::test]
    async fn gemini_sdk_route_proxies_native_body_and_records_spend() {
        let mock = MockGeminiServer::launch().await;
        let state = build_test_state(vec![
            gemini_provider(
                "googleai",
                "http://127.0.0.1:9",
                vec!["other-model".to_string()],
            ),
            gemini_provider(
                "gemini",
                &mock.base_url,
                vec!["gemini-3.1-flash-lite".to_string()],
            ),
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["candidates"][0]["content"]["parts"][0]["text"], "ok");

        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].path_and_query,
            "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=test-api-key-12345678901234567890"
        );
        assert_eq!(requests[0].headers["x-base-header"], "base-value");
        assert_eq!(requests[0].headers["x-custom-header"], "custom-value");
        let upstream_body: Value =
            serde_json::from_slice(&requests[0].body).expect("body should be json");
        assert_eq!(
            upstream_body["contents"][0]["parts"][0]["text"],
            "hello from the Gemini SDK"
        );

        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider spend should be recorded");
        assert!(provider_usage.current_spend > 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model spend should be recorded");
        assert!(model_usage.current_spend > 0.0);

        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_stream_route_uses_sse_alt_query() {
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/models/gemini-3.1-flash-lite:streamGenerateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("text/event-stream")
        );
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("\"usageMetadata\""));
        let requests = mock.requests();
        assert_eq!(
            requests[0].path_and_query,
            "/v1/models/gemini-3.1-flash-lite:streamGenerateContent?alt=sse&key=test-api-key-12345678901234567890"
        );
        let provider_usage = budget_limits
            .providers
            .get_provider_usage("gemini")
            .expect("provider stream spend should be recorded");
        assert!(provider_usage.current_spend > 0.0);
        let model_usage = budget_limits
            .models
            .get_model_usage("gemini-3.1-flash-lite")
            .expect("model stream spend should be recorded");
        assert!(model_usage.current_spend > 0.0);

        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_prefixed_sdk_route_is_supported() {
        let mock = MockGeminiServer::launch().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/gemini/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(
            requests[0].path_and_query,
            "/v1beta/models/gemini-3.1-flash-lite:generateContent?key=test-api-key-12345678901234567890"
        );

        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_unauthenticated_when_auth_enabled() {
        let mock = MockGeminiServer::launch().await;
        let state = build_auth_required_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_applies_api_key_token_limit_when_native_cap_is_omitted() {
        let mock = MockGeminiServer::launch().await;
        let state = build_auth_required_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        let api_key = api_key_with_max_tokens_per_request(7);
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body_without_generation_config())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        let upstream_body: Value =
            serde_json::from_slice(&requests[0].body).expect("body should be json");
        assert_eq!(
            upstream_body["generationConfig"]["maxOutputTokens"],
            json!(7)
        );
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_renders_invalid_api_key_policy_as_openai_error() {
        let mock = MockGeminiServer::launch().await;
        let state = build_auth_required_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        let api_key = api_key_with_invalid_runtime_permissions();
        let app = test::init_service(
            App::new()
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<ApiKey>(api_key.clone());
                    srv.call(req)
                })
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body: Value = test::read_body_json(response).await;
        assert_eq!(body["error"]["type"], "permission_error");
        assert_eq!(body["error"]["code"], "permission_denied");
        assert!(
            body["error"]["message"]
                .as_str()
                .expect("error message")
                .contains("API key runtime policy is invalid")
        );
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_unsafe_model_segment_before_upstream() {
        let mock = MockGeminiServer::launch().await;
        let state =
            build_test_state(vec![gemini_provider("gemini", &mock.base_url, Vec::new())]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/..%2Fgemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_rejects_exhausted_provider_budget_before_upstream() {
        let mock = MockGeminiServer::launch().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(0.01, ResetPeriod::Monthly),
        );
        state
            .budget_limits
            .record_spend("gemini", "gemini-3.1-flash-lite", 0.01);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_reserves_budget_before_upstream() {
        let mock = MockGeminiServer::launch().await;
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            &mock.base_url,
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        state.budget_limits.providers.set_provider_limit(
            "gemini",
            ProviderLimitConfig::new(0.000001, ResetPeriod::Monthly),
        );
        state.budget_limits.models.set_model_limit(
            "gemini-3.1-flash-lite",
            ModelLimitConfig::new(0.000001, ResetPeriod::Monthly),
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
                .uri("/v1beta/models/gemini-3.1-flash-lite:generateContent")
                .set_json(gemini_body())
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        assert!(mock.requests().is_empty());
        mock.shutdown().await;
    }

    #[tokio::test]
    async fn gemini_sdk_route_network_error_does_not_leak_provider_key() {
        let state = build_test_state(vec![gemini_provider(
            "gemini",
            "http://127.0.0.1:9",
            vec!["gemini-3.1-flash-lite".to_string()],
        )])
        .await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
    async fn gemini_sdk_stream_route_releases_budget_on_midstream_read_error() {
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
        let budget_limits = state.budget_limits.clone();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let stream_body = test::read_body(response).await;
        let stream_text = String::from_utf8(stream_body.to_vec()).expect("stream should be utf8");
        assert!(stream_text.contains("partial"));
        assert!(stream_text.contains("Gemini upstream stream error"));
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

        broken.shutdown().await;
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
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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

    #[tokio::test]
    async fn public_only_gemini_route_rejects_loopback_before_connect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let mut provider = gemini_provider(
            "gemini",
            &format!("http://{address}"),
            vec!["gemini-3.1-flash-lite".to_string()],
        );
        provider.endpoint_access = ProviderEndpointAccess::PublicOnly;
        let state = build_test_state(vec![provider]).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
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
        let body: Value = test::read_body_json(response).await;
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("SSRF protection"))
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "public-only Gemini route must not connect to loopback listener"
        );
    }
}
