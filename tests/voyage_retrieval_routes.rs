#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[allow(dead_code)]
#[path = "common/providers.rs"]
mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::{mock_provider_config, route_policy_bootstrap_providers};
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use bytes::Bytes;
    use litellm_rs::Config;
    use litellm_rs::config::models::provider::ProviderConfig;
    use litellm_rs::core::budget::{ProviderLimitConfig, ResetPeriod};
    use litellm_rs::core::net::ProviderEndpointAccess;
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Clone, Debug)]
    struct CapturedRequest {
        path: String,
        authorization: Option<String>,
        body: Value,
    }

    #[derive(Clone)]
    struct MockState {
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
    }

    struct MockVoyageServer {
        base_url: String,
        requests: Arc<Mutex<Vec<CapturedRequest>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockVoyageServer {
        async fn start_voyage_mock() -> Self {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let state = MockState {
                requests: Arc::clone(&requests),
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
            let address = listener.local_addr().expect("mock server address");
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .route("/v1/embeddings", web::post().to(mock_voyage_embeddings))
                    .route("/v1/rerank", web::post().to(mock_voyage_rerank))
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            for _ in 0..20 {
                if tokio::net::TcpStream::connect(address).await.is_ok() {
                    return Self {
                        base_url: format!("http://{address}/v1"),
                        requests,
                        handle,
                        task,
                    };
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            panic!("mock server did not accept connections at {address}");
        }

        fn requests(&self) -> Vec<CapturedRequest> {
            self.requests.lock().expect("captured requests").clone()
        }

        async fn shutdown_voyage_mock(self) {
            self.handle.stop(false).await;
            self.task
                .await
                .expect("mock server task")
                .expect("mock server result");
        }
    }

    fn capture_voyage_request(state: &MockState, request: &HttpRequest, body: &Bytes) -> Value {
        let body: Value = serde_json::from_slice(body).expect("request body should be JSON");
        state
            .requests
            .lock()
            .expect("captured requests")
            .push(CapturedRequest {
                path: request.path().to_string(),
                authorization: request
                    .headers()
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                body: body.clone(),
            });
        body
    }

    async fn mock_voyage_embeddings(
        state: web::Data<MockState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        let body = capture_voyage_request(&state, &request, &body);
        if body["input"] == json!(["force-error"]) {
            return HttpResponse::BadRequest().json(json!({"detail": "invalid voyage input"}));
        }
        HttpResponse::Ok().json(json!({
            "object": "list",
            "data": [
                {"object": "embedding", "index": 0, "embedding": [0.1, 0.2]},
                {"object": "embedding", "index": 1, "embedding": [0.3, 0.4]}
            ],
            "model": body["model"],
            "usage": {"total_tokens": 7}
        }))
    }

    async fn mock_voyage_rerank(
        state: web::Data<MockState>,
        request: HttpRequest,
        body: Bytes,
    ) -> HttpResponse {
        let body = capture_voyage_request(&state, &request, &body);
        if body["query"] == "force-error" {
            return HttpResponse::TooManyRequests().json(json!({"detail": "voyage rate limit"}));
        }
        if body["query"] == "malformed-response" {
            return HttpResponse::Ok()
                .content_type("application/json")
                .body("{");
        }
        HttpResponse::Ok().json(json!({
            "results": [
                {"index": 1, "relevance_score": 0.9},
                {"index": 0, "relevance_score": 0.2}
            ],
            "usage": {"total_tokens": 11}
        }))
    }

    fn voyage_provider(base_url: &str) -> ProviderConfig {
        let mut provider = mock_provider_config(
            "voyage-native",
            "voyage",
            "voyage-secret",
            base_url,
            vec!["voyage-4".to_string(), "rerank-2.5".to_string()],
        );
        provider.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        provider
    }

    async fn build_state(provider: ProviderConfig) -> litellm_rs::server::state::AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.providers = route_policy_bootstrap_providers(&[provider]);
        GatewayHttpServer::new(&config)
            .await
            .expect("Voyage gateway should initialize")
            .state()
            .clone()
    }

    #[tokio::test]
    async fn voyage_embeddings_preserve_native_contract() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(json!({
                    "model": "voyage-4",
                    "input": ["query", "document"],
                    "input_type": "document",
                    "dimensions": 512,
                    "truncation": false
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let response_body: Value = test::read_body_json(response).await;
        assert_eq!(response_body["model"], "voyage-4");
        assert_eq!(response_body["data"][1]["index"], 1);
        assert_eq!(response_body["usage"]["total_tokens"], 7);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/embeddings");
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer voyage-secret")
        );
        assert_eq!(requests[0].body["model"], "voyage-4");
        assert_eq!(requests[0].body["input"], json!(["query", "document"]));
        assert_eq!(requests[0].body["input_type"], "document");
        assert_eq!(requests[0].body["output_dimension"], 512);
        assert_eq!(requests[0].body["truncation"], false);
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_rerank_preserves_indexes_top_k_and_usage() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "best document",
                    "documents": ["first", "second"],
                    "top_n": 2,
                    "return_documents": true,
                    "truncation": false
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let response_body: Value = test::read_body_json(response).await;
        assert_eq!(response_body["model"], "rerank-2.5");
        assert_eq!(response_body["results"][0]["index"], 1);
        assert_eq!(response_body["results"][0]["document"], "second");
        assert_eq!(response_body["usage"]["total_tokens"], 11);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/rerank");
        assert_eq!(requests[0].body["model"], "rerank-2.5");
        assert_eq!(requests[0].body["top_k"], 2);
        assert_eq!(requests[0].body["truncation"], false);
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_rerank_defaults_to_returning_documents() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "best document",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let response_body: Value = test::read_body_json(response).await;
        assert_eq!(response_body["results"][0]["document"], "second");
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["return_documents"], true);
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_rerank_uses_configured_alias_and_records_canonical_spend() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let mut provider = voyage_provider("");
        provider.name = "custom-voyage".to_string();
        provider.base_url = None;
        provider
            .settings
            .insert("api_base".to_string(), json!(mock.base_url));
        let state = build_state(provider).await;
        state.budget_limits.providers.set_provider_limit(
            "voyage",
            ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
        );
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "best document",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let requests = mock.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].path, "/v1/rerank");
        let usage = state
            .budget_limits
            .providers
            .get_provider_usage("voyage")
            .expect("canonical Voyage budget should be metered");
        assert!((usage.current_spend - 11.0 * 5e-8).abs() < f64::EPSILON);
        assert!(
            state
                .budget_limits
                .providers
                .get_provider_usage("custom-voyage")
                .is_none(),
            "deployment name must not replace canonical pricing identity"
        );
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_base_url_precedes_api_base_alias() {
        let primary = MockVoyageServer::start_voyage_mock().await;
        let alias = MockVoyageServer::start_voyage_mock().await;
        let mut provider = voyage_provider(&primary.base_url);
        provider
            .settings
            .insert("api_base".to_string(), json!(alias.base_url.clone()));
        let state = build_state(provider).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "best document",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(primary.requests().len(), 1);
        assert!(alias.requests().is_empty());
        primary.shutdown_voyage_mock().await;
        alias.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_retrieval_rejects_wrong_qualifier_and_unknown_model_without_io() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        for body in [
            json!({"model": "cohere/rerank-2.5", "query": "q", "documents": ["d"]}),
            json!({"model": "voyage-unknown", "query": "q", "documents": ["d"]}),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/rerank")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        assert!(mock.requests().is_empty());
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_rerank_rejects_unpriced_exact_model_before_upstream_io() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let mut provider = voyage_provider(&mock.base_url);
        provider.models.push("rerank-1".to_string());
        let state = build_state(provider).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-1",
                    "query": "best document",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response_body: Value = test::read_body_json(response).await;
        assert!(response_body.to_string().contains("pricing unavailable"));
        assert!(mock.requests().is_empty());
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_rerank_rejects_invalid_request_before_upstream_io() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        for body in [
            json!({"model": "rerank-2.5", "query": "", "documents": ["document"]}),
            json!({"model": "rerank-2.5", "query": "query", "documents": []}),
            json!({"model": "rerank-2.5", "query": "query", "documents": ["document"], "top_n": 0}),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/v1/rerank")
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
        assert!(mock.requests().is_empty());
        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_upstream_errors_keep_status_and_message() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let embedding_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(json!({"model": "voyage-4", "input": ["force-error"]}))
                .to_request(),
        )
        .await;
        assert_eq!(embedding_response.status(), StatusCode::BAD_REQUEST);
        let embedding_error: Value = test::read_body_json(embedding_response).await;
        assert!(embedding_error.to_string().contains("invalid voyage input"));

        let rerank_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "force-error",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;
        assert_eq!(rerank_response.status(), StatusCode::TOO_MANY_REQUESTS);
        let rerank_error: Value = test::read_body_json(rerank_response).await;
        assert!(rerank_error.to_string().contains("voyage rate limit"));

        let malformed_response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/rerank")
                .set_json(json!({
                    "model": "rerank-2.5",
                    "query": "malformed-response",
                    "documents": ["first", "second"]
                }))
                .to_request(),
        )
        .await;
        assert!(malformed_response.status().is_server_error());

        mock.shutdown_voyage_mock().await;
    }

    #[tokio::test]
    async fn voyage_invalid_input_type_fails_before_upstream_io() {
        let mock = MockVoyageServer::start_voyage_mock().await;
        let state = build_state(voyage_provider(&mock.base_url)).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;

        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/v1/embeddings")
                .set_json(json!({
                    "model": "voyage-4",
                    "input": ["query", "document"],
                    "input_type": "search"
                }))
                .to_request(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response_body: Value = test::read_body_json(response).await;
        assert!(response_body.to_string().contains("input_type"));
        assert!(mock.requests().is_empty());
        mock.shutdown_voyage_mock().await;
    }
}
