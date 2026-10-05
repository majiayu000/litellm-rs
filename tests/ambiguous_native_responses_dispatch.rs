#![cfg(all(feature = "gateway", feature = "sqlite"))]

#[path = "common/providers.rs"]
pub mod provider_fixtures;

use actix_web::{App, HttpMessage, http::StatusCode, test, web};
use litellm_rs::{
    Config,
    core::{
        budget::{BudgetConfig, BudgetScope, ModelLimitConfig, ProviderLimitConfig, ResetPeriod},
        keys::CreateKeyConfig,
        types::context::RequestContext,
    },
    server::{HttpServer, state::AppState},
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy)]
enum Reply {
    Disconnect,
    HeaderTimeout,
    Reject,
}

struct Upstream {
    base: String,
    posts: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Upstream {
    async fn start(reply: Reply) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        let posts = Arc::new(AtomicUsize::new(0));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let count = posts.clone();
        let seen = bodies.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut stream, _) = accepted.unwrap();
                        let count = count.clone();
                        let seen = seen.clone();
                        connections.spawn(async move {
                            let mut request = Vec::new();
                            let (header_end, length) = loop {
                                let mut chunk = [0; 4096];
                                let n = stream.read(&mut chunk).await.unwrap();
                                assert_ne!(n, 0, "client closed before its complete request");
                                request.extend_from_slice(&chunk[..n]);
                                assert!(request.len() < 64 * 1024);
                                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                                    let headers = std::str::from_utf8(&request[..end]).unwrap();
                                    assert!(headers.starts_with("POST /v1/responses"));
                                    let length = headers.lines().find_map(|line| {
                                        let (name, value) = line.split_once(':')?;
                                        name.eq_ignore_ascii_case("content-length")
                                            .then(|| value.trim().parse::<usize>().unwrap())
                                    }).expect("JSON request has content-length");
                                    break (end + 4, length);
                                }
                            };
                            while request.len() < header_end + length {
                                let mut chunk = [0; 4096];
                                let n = stream.read(&mut chunk).await.unwrap();
                                assert_ne!(n, 0);
                                request.extend_from_slice(&chunk[..n]);
                            }
                            seen.lock().unwrap().push(
                                serde_json::from_slice(&request[header_end..header_end + length]).unwrap()
                            );
                            // The complete billable POST was received, but there is no response ID.
                            count.fetch_add(1, Ordering::SeqCst);
                            match reply {
                                Reply::Disconnect => {}
                                Reply::HeaderTimeout => tokio::time::sleep(Duration::from_secs(60)).await,
                                Reply::Reject => {
                                    let body = r#"{"error":{"message":"invalid credentials"}}"#;
                                    let response = format!(
                                        "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                                        body.len()
                                    );
                                    stream.write_all(response.as_bytes()).await.unwrap();
                                }
                            }
                            stream.shutdown().await.unwrap();
                        });
                    }
                    completed = connections.join_next(), if !connections.is_empty() => {
                        completed.unwrap().unwrap();
                    }
                }
            }
        });
        Self {
            base,
            posts,
            bodies,
            task,
        }
    }
}

async fn gateway(upstream: &Upstream, dir: Option<&tempfile::TempDir>) -> HttpServer {
    let mut config = Config::default();
    config.gateway.storage.database.enabled = dir.is_some();
    config.gateway.storage.database.auto_migrate = true;
    if let Some(dir) = dir {
        config.gateway.storage.database.url = format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("dispatch.db").display()
        );
    }
    config.gateway.storage.redis.enabled = false;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.pricing.source = None;
    let mut provider = provider_fixtures::mock_provider_config(
        "native-test",
        "openai",
        "sk-test-not-a-real-key-12345678901234567890",
        &upstream.base,
        vec!["gpt-4o-mini".into()],
    );
    provider.timeout = 1;
    provider.retry.base_delay = 1;
    provider.retry.max_delay = 1;
    provider.retry.jitter = 0.0;
    config.gateway.providers = vec![provider];
    let server = HttpServer::new(&config).await.unwrap();
    server.state().pricing.add_custom_model(
        "gpt-4o-mini".into(),
        serde_json::from_value(json!({
            "litellm_provider":"openai", "mode":"chat", "max_output_tokens":1,
            "input_cost_per_token":0.0, "output_cost_per_token":1.0
        }))
        .unwrap(),
    );
    server
}

fn body(mode: &str) -> Value {
    if mode == "compact" {
        json!({"model":"gpt-4o-mini","input":"Hello"})
    } else {
        json!({"model":"gpt-4o-mini","input":"Hello","max_output_tokens":1,
            "store":false,"stream":mode == "stream"})
    }
}

async fn budget_context(state: &AppState) -> (RequestContext, uuid::Uuid, BudgetScope) {
    state.budget_limits.providers.set_provider_limit(
        "native-test",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    state.budget_limits.models.set_model_limit(
        "gpt-4o-mini",
        ModelLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let scope = BudgetScope::ApiKey("dispatch-test".into());
    let budget = state
        .budget_manager
        .create_budget(scope.clone(), BudgetConfig::new("dispatch-test", 10.0))
        .await
        .unwrap();
    let (key_id, _) = state
        .key_manager
        .generate_key(CreateKeyConfig {
            name: "dispatch-test".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    (
        RequestContext::new()
            .with_api_key(key_id)
            .with_api_key_budget(budget.id.parse().unwrap()),
        key_id,
        scope,
    )
}

#[tokio::test]
async fn foreground_ambiguous_dispatch_retains_unknown_budgets() {
    for reply in [Reply::Disconnect, Reply::HeaderTimeout] {
        for mode in ["unary", "stream", "compact"] {
            let upstream = Upstream::start(reply).await;
            let server = gateway(&upstream, None).await;
            let state = server.state();
            let (context, key_id, scope) = budget_context(state).await;
            let app = test::init_service(
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .configure(litellm_rs::server::routes::ai::configure_routes),
            )
            .await;
            let path = if mode == "compact" {
                "/v1/responses/compact"
            } else {
                "/v1/responses"
            };
            let req = test::TestRequest::post()
                .uri(path)
                .set_json(body(mode))
                .to_request();
            req.extensions_mut().insert(context);
            let response = test::call_service(&app, req).await;
            assert!(!response.status().is_success());
            let _ = test::read_body(response).await;
            for spend in [
                state
                    .budget_limits
                    .providers
                    .get_provider_usage("native-test")
                    .unwrap()
                    .current_spend,
                state
                    .budget_limits
                    .models
                    .get_model_usage("gpt-4o-mini")
                    .unwrap()
                    .current_spend,
                state.budget_manager.get_current_spend(&scope),
            ] {
                assert!(
                    (spend - 1.0).abs() < 1e-12,
                    "{mode} lost reserved upper bound: {spend}"
                );
            }
            let usage = state.key_manager.get_usage_stats(key_id).await.unwrap();
            assert_eq!(usage.total_requests, 1);
            assert_eq!(usage.unpriced_requests, 1);
            assert_eq!(usage.total_tokens, 0);
            assert_eq!(usage.total_cost, 0.0);
            assert_eq!(
                upstream.posts.load(Ordering::SeqCst),
                1,
                "{mode} replayed POST"
            );
        }
    }
}

#[tokio::test]
async fn foreground_ambiguous_dispatch_is_not_replayed() {
    for mode in ["unary", "stream", "compact"] {
        let upstream = Upstream::start(Reply::Disconnect).await;
        let server = gateway(&upstream, None).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(server.state().clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let path = if mode == "compact" {
            "/v1/responses/compact"
        } else {
            "/v1/responses"
        };
        let response = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(path)
                .set_json(body(mode))
                .to_request(),
        )
        .await;
        assert!(!response.status().is_success());
        let _ = test::read_body(response).await;
        assert_eq!(
            upstream.posts.load(Ordering::SeqCst),
            1,
            "{mode} replayed POST"
        );
    }
}

#[tokio::test]
async fn foreground_known_rejection_releases_budgets() {
    for mode in ["unary", "stream", "compact"] {
        let upstream = Upstream::start(Reply::Reject).await;
        let server = gateway(&upstream, None).await;
        let state = server.state();
        let (context, key_id, scope) = budget_context(state).await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let path = if mode == "compact" {
            "/v1/responses/compact"
        } else {
            "/v1/responses"
        };
        let req = test::TestRequest::post()
            .uri(path)
            .set_json(body(mode))
            .to_request();
        req.extensions_mut().insert(context);
        let response = test::call_service(&app, req).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let _ = test::read_body(response).await;
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
        assert_eq!(
            state
                .budget_limits
                .providers
                .get_provider_usage("native-test")
                .unwrap()
                .current_spend,
            0.0
        );
        assert_eq!(
            state
                .budget_limits
                .models
                .get_model_usage("gpt-4o-mini")
                .unwrap()
                .current_spend,
            0.0
        );
        assert_eq!(state.budget_manager.get_current_spend(&scope), 0.0);
        assert_eq!(
            state
                .key_manager
                .get_usage_stats(key_id)
                .await
                .unwrap()
                .unpriced_requests,
            0
        );
    }
}

#[tokio::test]
async fn background_ambiguous_dispatch_keeps_one_durable_unknown_obligation() {
    use litellm_rs::core::models::user::types::{User, UserStatus};
    use litellm_rs::storage::database::entities::response_settlement::Entity;
    use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};

    for mode in ["unary", "stream"] {
        let upstream = Upstream::start(Reply::Disconnect).await;
        let dir = tempfile::tempdir().unwrap();
        let server = gateway(&upstream, Some(&dir)).await;
        let state = server.state();
        let mut user = User::new(
            "dispatch-owner".into(),
            "dispatch@example.com".into(),
            "unused-hash".into(),
        );
        user.status = UserStatus::Active;
        state.storage.database.create_user(&user).await.unwrap();
        let (key, _) = state
            .auth
            .api_key()
            .create_key(
                Some(user.metadata.id),
                None,
                "dispatch".into(),
                vec!["api.chat".into()],
            )
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(litellm_rs::server::routes::ai::configure_routes),
        )
        .await;
        let mut body = body(mode);
        body["background"] = json!(true);
        let req = test::TestRequest::post()
            .uri("/v1/responses")
            .set_json(body)
            .to_request();
        req.extensions_mut()
            .insert(RequestContext::new().with_api_key(key.metadata.id));
        let response = test::call_service(&app, req).await;
        assert!(!response.status().is_success());
        let _ = test::read_body(response).await;
        assert_eq!(
            upstream.posts.load(Ordering::SeqCst),
            1,
            "{mode} replayed POST"
        );
        assert_eq!(upstream.bodies.lock().unwrap()[0]["background"], true);
        let rows = Entity::find()
            .all(state.storage.database.connection())
            .await
            .unwrap();
        assert_eq!(
            rows.len(),
            1,
            "one creation must retain one dispatch obligation"
        );
        let row = rows.into_iter().next().unwrap();
        assert_eq!(row.outcome, "pending");
        assert!(!row.complete);
        assert!(row.response_id.is_none());
        // Expire only this fixture's deadline; the normal recovery worker must
        // conservatively settle the unknown outcome without replaying creation.
        let id = row.id.clone();
        let mut active = row.into_active_model();
        active.deadline = Set(0);
        active.lease_until = Set(0);
        active.next_attempt = Set(0);
        active
            .update(state.storage.database.connection())
            .await
            .unwrap();
        let settled = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                let row = Entity::find_by_id(&id)
                    .one(state.storage.database.connection())
                    .await
                    .unwrap()
                    .unwrap();
                if row.complete {
                    break row;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(settled.outcome, "reserved_unknown");
        assert_eq!(settled.cost, Some(settled.reserved));
        let usage = state
            .storage
            .database
            .find_api_key_by_id(key.metadata.id)
            .await
            .unwrap()
            .unwrap()
            .usage_stats;
        assert_eq!(usage.unpriced_requests, 1);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.total_cost, 0.0);
        assert_eq!(upstream.posts.load(Ordering::SeqCst), 1);
    }
}
