#[cfg(all(test, feature = "gateway", feature = "storage"))]
#[path = "common/providers.rs"]
pub mod provider_fixtures;

#[cfg(all(test, feature = "gateway", feature = "storage"))]
mod tests {
    use super::provider_fixtures::mock_provider_config;
    use actix_web::{App, HttpRequest, HttpResponse, HttpServer, http::StatusCode, test, web};
    use litellm_rs::Config;
    use litellm_rs::core::budget::{
        BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod,
    };
    use litellm_rs::core::models::user::types::{User, UserRole, UserStatus};
    use litellm_rs::core::models::{ApiKey, Metadata, UsageStats};
    use litellm_rs::server::HttpServer as GatewayHttpServer;
    use litellm_rs::server::middleware::AuthMiddleware;
    use litellm_rs::server::state::AppState;
    use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use uuid::Uuid;

    const CHAT_MODEL: &str = "gpt-4o";
    const EMBEDDING_MODEL: &str = "text-embedding-3-small";
    const IMAGE_MODEL: &str = "gpt-image-1-mini";

    #[derive(Clone, Copy)]
    enum Endpoint {
        Chat,
        Embeddings,
        Images,
    }

    impl Endpoint {
        const ALL: [Endpoint; 3] = [Endpoint::Chat, Endpoint::Embeddings, Endpoint::Images];

        fn path(self) -> &'static str {
            match self {
                Endpoint::Chat => "/v1/chat/completions",
                Endpoint::Embeddings => "/v1/embeddings",
                Endpoint::Images => "/v1/images/generations",
            }
        }

        fn request_body(self) -> Value {
            match self {
                Endpoint::Chat => json!({
                    "model": CHAT_MODEL,
                    "messages": [{ "role": "user", "content": "hello" }],
                    "max_tokens": 16
                }),
                Endpoint::Embeddings => json!({
                    "model": EMBEDDING_MODEL,
                    "input": "hello"
                }),
                Endpoint::Images => json!({
                    "model": IMAGE_MODEL,
                    "prompt": "make a small icon",
                    "size": "1024x1024"
                }),
            }
        }
    }

    #[derive(Clone, Copy)]
    enum MockMode {
        Success,
        Failure,
    }

    #[derive(Clone)]
    struct MockState {
        mode: MockMode,
        paths: Arc<Mutex<Vec<String>>>,
    }

    struct MockOpenAiServer {
        base_url: String,
        paths: Arc<Mutex<Vec<String>>>,
        handle: actix_web::dev::ServerHandle,
        task: tokio::task::JoinHandle<std::io::Result<()>>,
    }

    impl MockOpenAiServer {
        async fn start(mode: MockMode) -> Self {
            let paths = Arc::new(Mutex::new(Vec::new()));
            let state = MockState {
                mode,
                paths: Arc::clone(&paths),
            };
            let listener =
                std::net::TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
            let address = listener.local_addr().expect("mock server address");
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .route("/v1/chat/completions", web::post().to(mock_chat))
                    .route("/v1/embeddings", web::post().to(mock_embeddings))
                    .route("/v1/images/generations", web::post().to(mock_images))
            })
            .listen(listener)
            .expect("mock server should listen")
            .run();
            let handle = server.handle();
            let task = tokio::spawn(server);
            wait_for_server(address).await;

            Self {
                base_url: format!("http://{address}/v1"),
                paths,
                handle,
                task,
            }
        }

        fn paths(&self) -> Vec<String> {
            self.paths.lock().unwrap().clone()
        }

        async fn stop(self) {
            self.handle.stop(true).await;
            let result = self.task.await.expect("mock server task should join");
            if let Err(error) = result {
                panic!("mock server should stop cleanly: {error}");
            }
        }
    }

    async fn wait_for_server(address: std::net::SocketAddr) {
        for _ in 0..20 {
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("mock server did not accept connections at {address}");
    }

    async fn mock_chat(state: web::Data<MockState>, request: HttpRequest) -> HttpResponse {
        record_path(&state, &request);
        match state.mode {
            MockMode::Success => HttpResponse::Ok().json(json!({
                "id": "chatcmpl-key-budget",
                "object": "chat.completion",
                "created": 1_707_000_000_i64,
                "model": CHAT_MODEL,
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "ok" },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 4,
                    "completion_tokens": 3,
                    "total_tokens": 7
                }
            })),
            MockMode::Failure => upstream_failure(),
        }
    }

    async fn mock_embeddings(state: web::Data<MockState>, request: HttpRequest) -> HttpResponse {
        record_path(&state, &request);
        match state.mode {
            MockMode::Success => HttpResponse::Ok().json(json!({
                "object": "list",
                "data": [{
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2]
                }],
                "model": EMBEDDING_MODEL,
                "usage": {
                    "prompt_tokens": 2,
                    "completion_tokens": 0,
                    "total_tokens": 2
                }
            })),
            MockMode::Failure => upstream_failure(),
        }
    }

    async fn mock_images(state: web::Data<MockState>, request: HttpRequest) -> HttpResponse {
        record_path(&state, &request);
        match state.mode {
            MockMode::Success => HttpResponse::Ok().json(json!({
                "created": 1_707_000_001_i64,
                "data": [{ "url": "https://images.example.test/key-budget.png" }]
            })),
            MockMode::Failure => upstream_failure(),
        }
    }

    fn record_path(state: &MockState, request: &HttpRequest) {
        state.paths.lock().unwrap().push(request.path().to_string());
    }

    fn upstream_failure() -> HttpResponse {
        HttpResponse::BadRequest().json(json!({
            "error": {
                "type": "invalid_request_error",
                "code": "mock_upstream_failure",
                "message": "mock upstream failure"
            }
        }))
    }

    async fn build_state(base_url: &str) -> AppState {
        let mut config = Config::default();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = false;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
        config.gateway.providers = vec![mock_provider_config(
            "openai",
            "openai",
            "sk-test",
            base_url,
            vec![
                CHAT_MODEL.to_string(),
                EMBEDDING_MODEL.to_string(),
                IMAGE_MODEL.to_string(),
            ],
        )];

        let server = GatewayHttpServer::new(&config)
            .await
            .expect("gateway server should initialize");
        let state = server.state().clone();
        state
            .storage
            .migrate()
            .await
            .expect("in-memory storage should migrate");
        configure_provider_and_model_budgets(&state);
        state
    }

    fn configure_provider_and_model_budgets(state: &AppState) {
        state.budget_limits.providers.set_provider_limit(
            "openai",
            ProviderLimitConfig::new(100.0, ResetPeriod::Monthly),
        );
        for model in [CHAT_MODEL, EMBEDDING_MODEL, IMAGE_MODEL] {
            state
                .budget_limits
                .models
                .set_model_limit(model, ModelLimitConfig::new(100.0, ResetPeriod::Monthly));
        }
    }

    async fn create_api_key_budget(state: &AppState, max_budget: f64) -> (BudgetScope, Uuid) {
        let scope = BudgetScope::ApiKey(format!("key-budget-{}", Uuid::new_v4()));
        let budget = state
            .budget_manager
            .create_budget(
                scope.clone(),
                BudgetConfig::new("route key budget", max_budget),
            )
            .await
            .expect("API key budget should be created");
        let budget_id = Uuid::parse_str(&budget.id).expect("budget id should be a UUID");
        (scope, budget_id)
    }

    async fn seed_api_key(state: &AppState, budget_id: Uuid) -> String {
        let mut user = User::new(
            "budget-route-user".to_string(),
            format!("budget-route-{}@example.test", Uuid::new_v4()),
            "hashed-password".to_string(),
        );
        user.role = UserRole::User;
        user.status = UserStatus::Active;
        let user = state
            .storage
            .db()
            .create_user(&user)
            .await
            .expect("test user should be inserted");

        let raw_api_key = format!("gw-key-budget-{}", Uuid::new_v4());
        let mut metadata = Metadata::new();
        metadata.set_extra(
            "__core_keys",
            json!({
                "budget_id": budget_id.to_string()
            }),
        );
        let api_key = ApiKey {
            metadata,
            name: "key-budget-route-test".to_string(),
            key_hash: hash_api_key(&raw_api_key, None),
            key_prefix: extract_api_key_prefix(&raw_api_key),
            user_id: Some(user.id()),
            team_id: None,
            permissions: vec!["use:api".to_string()],
            rate_limits: None,
            expires_at: None,
            is_active: true,
            last_used_at: None,
            usage_stats: UsageStats::default(),
        };

        state
            .storage
            .db()
            .create_api_key(&api_key)
            .await
            .expect("test API key should be inserted");
        raw_api_key
    }

    fn build_test_app(
        state: AppState,
    ) -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        let budget_limits = web::Data::new(Arc::clone(&state.budget_limits));

        App::new()
            .app_data(web::Data::new(state))
            .app_data(budget_limits)
            .wrap(AuthMiddleware)
            .configure(litellm_rs::server::routes::ai::configure_routes)
    }

    #[tokio::test]
    async fn api_key_budget_rejects_exhausted_budget_before_upstream() {
        let mock = MockOpenAiServer::start(MockMode::Success).await;
        let state = build_state(&mock.base_url).await;
        let (scope, budget_id) = create_api_key_budget(&state, 1.0).await;
        state
            .budget_manager
            .record_spend(&scope, 1.0)
            .await
            .expect("pre-test spend should be recorded");
        let budget_manager = state.budget_manager.clone();
        let raw_api_key = seed_api_key(&state, budget_id).await;
        let app = test::init_service(build_test_app(state)).await;

        for endpoint in Endpoint::ALL {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(endpoint.path())
                    .insert_header(("x-api-key", raw_api_key.as_str()))
                    .set_json(endpoint.request_body())
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        }

        assert!(
            mock.paths().is_empty(),
            "exhausted API key budget must reject every route before upstream"
        );
        assert_eq!(budget_manager.get_current_spend(&scope), 1.0);
        mock.stop().await;
    }

    #[tokio::test]
    async fn api_key_budget_records_successful_chat_embeddings_and_image_spend() {
        let mock = MockOpenAiServer::start(MockMode::Success).await;
        let state = build_state(&mock.base_url).await;
        let (scope, budget_id) = create_api_key_budget(&state, 100.0).await;
        let budget_manager = state.budget_manager.clone();
        let raw_api_key = seed_api_key(&state, budget_id).await;
        let app = test::init_service(build_test_app(state)).await;

        for endpoint in Endpoint::ALL {
            let before = budget_manager.get_current_spend(&scope);
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(endpoint.path())
                    .insert_header(("x-api-key", raw_api_key.as_str()))
                    .set_json(endpoint.request_body())
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let after = budget_manager.get_current_spend(&scope);
            assert!(
                after > before,
                "{} should settle API key budget spend after success",
                endpoint.path()
            );
        }

        assert_eq!(
            mock.paths(),
            vec![
                "/v1/chat/completions".to_string(),
                "/v1/embeddings".to_string(),
                "/v1/images/generations".to_string(),
            ]
        );
        mock.stop().await;
    }

    #[tokio::test]
    async fn api_key_budget_refunds_failed_upstream_calls() {
        let mock = MockOpenAiServer::start(MockMode::Failure).await;
        let state = build_state(&mock.base_url).await;
        let (scope, budget_id) = create_api_key_budget(&state, 100.0).await;
        let budget_manager = state.budget_manager.clone();
        let raw_api_key = seed_api_key(&state, budget_id).await;
        let app = test::init_service(build_test_app(state)).await;

        for endpoint in Endpoint::ALL {
            let response = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri(endpoint.path())
                    .insert_header(("x-api-key", raw_api_key.as_str()))
                    .set_json(endpoint.request_body())
                    .to_request(),
            )
            .await;
            assert!(
                !response.status().is_success(),
                "{} should surface upstream failure",
                endpoint.path()
            );
            assert_eq!(
                budget_manager.get_current_spend(&scope),
                0.0,
                "{} should release API key budget reservation after provider failure",
                endpoint.path()
            );
        }

        assert_eq!(
            mock.paths(),
            vec![
                "/v1/chat/completions".to_string(),
                "/v1/embeddings".to_string(),
                "/v1/images/generations".to_string(),
            ]
        );
        mock.stop().await;
    }
}
