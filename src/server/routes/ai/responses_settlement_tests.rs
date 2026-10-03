use super::*;
use crate::core::models::user::types::{User, UserStatus};
use crate::storage::database::entities::response_settlement::Entity;
use actix_web::{App, HttpResponse, HttpServer, web};
use sea_orm::{ConnectionTrait, DbBackend, EntityTrait, Statement};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn config(dir: &tempfile::TempDir) -> crate::Config {
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = true;
    config.gateway.storage.database.auto_migrate = true;
    config.gateway.storage.database.url = format!(
        "sqlite://{}?mode=rwc",
        dir.path().join("recovery.db").display()
    );
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config
}

async fn key(state: &AppState) -> uuid::Uuid {
    let mut user = User::new(
        "recovery-owner".into(),
        "recovery@example.com".into(),
        "unused-test-hash".into(),
    );
    user.status = UserStatus::Active;
    state.storage.database.create_user(&user).await.unwrap();
    state
        .auth
        .api_key()
        .create_key(
            Some(user.metadata.id),
            None,
            "recovery".into(),
            vec!["api.chat".into()],
        )
        .await
        .unwrap()
        .0
        .metadata
        .id
}

fn row(id: &str, key: Option<uuid::Uuid>) -> Model {
    Model {
        id: id.into(),
        owner: "user:owner".into(),
        api_key_id: key,
        provider: "openai".into(),
        model: "gpt-4o-mini".into(),
        deployment_id: "test".into(),
        deployment_binding: "test".into(),
        response_id: None,
        pricing_json: "null".into(),
        leases_json: "null".into(),
        reserved: 0.75,
        cost: None,
        tokens: 0,
        outcome: "pending".into(),
        key_settled: false,
        complete: false,
        deadline: chrono::Utc::now().timestamp() + 100,
        next_attempt: 0,
        lease_until: 0,
        revision: 0,
    }
}

async fn stored(state: &AppState, id: &str) -> Model {
    Entity::find_by_id(id)
        .one(state.storage.database.connection())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn restart_and_two_workers_recover_frozen_price_without_replaying_post() {
    let gets = Arc::new(AtomicUsize::new(0));
    let posts = Arc::new(AtomicUsize::new(0));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let g = gets.clone();
    let p = posts.clone();
    let server = HttpServer::new(move || {
        let g = g.clone(); let p = p.clone();
        App::new().route("/v1/responses/already-created", web::get().to(move || {
            let g = g.clone(); async move {
                g.fetch_add(1, Ordering::SeqCst);
                HttpResponse::Ok().json(json!({"id":"already-created","status":"completed","usage":{"input_tokens":1000,"output_tokens":1000,"total_tokens":2000}}))
            }
        })).route("/v1/responses", web::post().to(move || {
            p.fetch_add(1, Ordering::SeqCst);
            async { HttpResponse::InternalServerError().finish() }
        }))
    }).workers(1).listen(listener).unwrap().run();
    let handle = server.handle();
    tokio::spawn(server);
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(&dir);
    config.gateway.providers = vec![crate::config::models::provider::ProviderConfig {
        name: "recovery-openai".into(),
        provider_type: "openai".into(),
        api_key: "sk-test-fake-key-12345678901234567890".into(),
        base_url: Some(format!("http://{address}/v1")),
        endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
        models: vec!["gpt-4o-mini".into()],
        ..Default::default()
    }];
    let creator = crate::server::HttpServer::new(&config).await.unwrap();
    let key_id = key(creator.state()).await;
    let mut intent = row("restart", Some(key_id));
    let router = creator.state().unified_router();
    intent.deployment_id = router.get_deployments_for_model("gpt-4o-mini")[0].clone();
    intent.deployment_binding = router
        .get_deployment(&intent.deployment_id)
        .unwrap()
        .provider
        .native_response_binding()
        .unwrap();
    intent.response_id = Some("already-created".into());
    let mut price = PricingService::with_embedded_default()
        .unwrap()
        .get_model_info_for_provider("openai", "gpt-4o-mini")
        .unwrap()
        .1;
    price.input_cost_per_token = Some(0.0001);
    price.output_cost_per_token = Some(0.0002);
    intent.pricing_json = serde_json::to_string(&Some((("openai", "gpt-4o-mini"), price))).unwrap();
    // A durable dispatch already exists; the creating process and its worker are gone.
    // Block claims until that process has been dropped, then expire its SQL lease.
    intent.lease_until = i64::MAX;
    creator
        .state()
        .storage
        .database
        .insert_response_settlement(intent)
        .await
        .unwrap();
    drop(creator);
    let first = crate::server::HttpServer::new(&config).await.unwrap();
    first
        .state()
        .storage
        .database
        .wake_response_settlement("restart")
        .await
        .unwrap();
    let second = crate::server::HttpServer::new(&config).await.unwrap();
    let (a, b) = tokio::join!(recover(first.state()), recover(second.state()));
    a.unwrap();
    b.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !stored(first.state(), "restart").await.complete {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // Simulate a lost SQL completion acknowledgement after both ledger writes.
    let mut intent = stored(first.state(), "restart").await;
    intent.complete = false;
    intent.lease_until = 0;
    use sea_orm::{ActiveModelTrait, IntoActiveModel};
    let mut active = intent.into_active_model();
    active.complete = sea_orm::Set(false);
    active.lease_until = sea_orm::Set(0);
    active.next_attempt = sea_orm::Set(0);
    active
        .update(first.state().storage.database.connection())
        .await
        .unwrap();
    recover(second.state()).await.unwrap();
    let usage = first
        .state()
        .storage
        .database
        .find_api_key_by_id(key_id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(usage.total_requests, 1);
    assert_eq!(usage.total_tokens, 2000);
    assert!((usage.total_cost - 0.3).abs() < 1e-9);
    assert_eq!(posts.load(Ordering::SeqCst), 0);
    assert!(gets.load(Ordering::SeqCst) >= 1);
    handle.stop(false).await;
}

#[tokio::test]
async fn sql_failure_rolls_back_receipt_with_key_usage_and_retry_is_once() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::server::HttpServer::new(&config(&dir)).await.unwrap();
    let state = server.state();
    let key_id = key(state).await;
    let mut intent = row("atomic", Some(key_id));
    intent.cost = Some(0.25);
    intent.tokens = 42;
    intent.outcome = "actual".into();
    intent.lease_until = i64::MAX;
    state
        .storage
        .database
        .insert_response_settlement(intent.clone())
        .await
        .unwrap();
    state.storage.database.connection().execute(Statement::from_string(DbBackend::Sqlite,
        "CREATE TRIGGER fail_response_key_usage BEFORE UPDATE ON api_keys BEGIN SELECT RAISE(ABORT, 'injected write failure'); END".to_string())).await.unwrap();
    assert!(
        state
            .storage
            .database
            .settle_response_key(&intent)
            .await
            .is_err()
    );
    assert!(!stored(state, "atomic").await.key_settled);
    state
        .storage
        .database
        .connection()
        .execute(Statement::from_string(
            DbBackend::Sqlite,
            "DROP TRIGGER fail_response_key_usage".to_string(),
        ))
        .await
        .unwrap();
    state
        .storage
        .database
        .settle_response_key(&intent)
        .await
        .unwrap();
    state
        .storage
        .database
        .settle_response_key(&intent)
        .await
        .unwrap();
    let usage = state
        .storage
        .database
        .find_api_key_by_id(key_id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(usage.total_requests, 1);
    assert_eq!(usage.total_tokens, 42);
    assert_eq!(usage.total_cost, 0.25);
}

#[tokio::test]
async fn unknown_dispatch_is_retained_separately_from_deleted_or_expired_content() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::server::HttpServer::new(&config(&dir)).await.unwrap();
    let state = server.state();
    let key_id = key(state).await;
    let mut intent = row("unknown", Some(key_id));
    intent.deadline = 0;
    state
        .storage
        .database
        .insert_response_settlement(intent)
        .await
        .unwrap();
    let content = response::Model {
        id: "expired-content".into(),
        owner: "user:owner".into(),
        response_json: "{}".into(),
        input_json: "{}".into(),
        deployment_id: None,
        deployment_binding: None,
        background: true,
        status: "queued".into(),
        expires_at: 1,
        lease_until: None,
        revision: 0,
    };
    state
        .storage
        .database
        .insert_response(content.clone(), 0)
        .await
        .unwrap();
    assert!(
        state
            .storage
            .database
            .owned_response("expired-content", "user:owner", 2)
            .await
            .unwrap()
            .is_none()
    );
    let mut live = content;
    live.id = "deleted-content".into();
    live.expires_at = chrono::Utc::now().timestamp() + 100;
    state
        .storage
        .database
        .insert_response(live, 2)
        .await
        .unwrap();
    assert!(
        state
            .storage
            .database
            .delete_owned_response("deleted-content", "user:owner", 2)
            .await
            .unwrap()
    );
    for id in ["deleted-content", "expired-content"] {
        let mut obligation = row(id, None);
        obligation.response_id = Some(id.into());
        obligation.deadline = 0;
        state
            .storage
            .database
            .insert_response_settlement(obligation)
            .await
            .unwrap();
    }
    assert_eq!(stored(state, "unknown").await.outcome, "pending");
    recover(state).await.unwrap();
    for id in ["deleted-content", "expired-content"] {
        let obligation = stored(state, id).await;
        assert!(obligation.complete);
        assert_eq!(obligation.outcome, "reserved_unknown");
    }
    let intent = stored(state, "unknown").await;
    assert!(intent.complete);
    assert_eq!(intent.outcome, "reserved_unknown");
    assert_eq!(intent.cost, Some(0.75));
    let usage = state
        .storage
        .database
        .find_api_key_by_id(key_id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(usage.total_cost, 0.0);
    assert_eq!(usage.total_tokens, 0);
    assert_eq!(usage.unpriced_requests, 1);
}

#[tokio::test]
async fn observed_terminal_usage_survives_upstream_handle_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::server::HttpServer::new(&config(&dir)).await.unwrap();
    let state = server.state();
    let key_id = key(state).await;
    let mut intent = row("observed", Some(key_id));
    intent.response_id = Some("deleted-upstream".into());
    intent.lease_until = i64::MAX;
    let price = PricingService::with_embedded_default()
        .unwrap()
        .get_model_info_for_provider("openai", "gpt-4o-mini")
        .unwrap()
        .1;
    intent.pricing_json = serde_json::to_string(&Some((("openai", "gpt-4o-mini"), price))).unwrap();
    state
        .storage
        .database
        .insert_response_settlement(intent)
        .await
        .unwrap();
    let usage =
        response_usage(&json!({"usage":{"input_tokens":10,"output_tokens":10,"total_tokens":20}}))
            .unwrap();
    // No configured deployment can retrieve deleted-upstream. The charge must
    // use the already-observed completion, rather than attempting another GET.
    submit_usage(state, "observed", Some(&usage)).await.unwrap();
    let completed = stored(state, "observed").await;
    assert!(completed.complete);
    assert_eq!(completed.outcome, "actual");
    assert_eq!(completed.tokens, 20);
    assert!(completed.cost.unwrap() > 0.0);
}

#[tokio::test]
async fn http_created_background_response_recovers_after_creator_runtime_stops() {
    use crate::server::middleware::AuthMiddleware;
    use actix_web::{http::StatusCode, test};
    use std::sync::atomic::AtomicBool;

    let posts = Arc::new(AtomicUsize::new(0));
    let gets = Arc::new(AtomicUsize::new(0));
    let terminal = Arc::new(AtomicBool::new(false));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream_address = listener.local_addr().unwrap();
    let (p, g, t) = (posts.clone(), gets.clone(), terminal.clone());
    let upstream = HttpServer::new(move || {
        let p = p.clone(); let g = g.clone(); let t = t.clone();
        App::new()
            .route("/v1/responses", web::post().to(move |body: web::Json<serde_json::Value>| {
                p.fetch_add(1, Ordering::SeqCst);
                assert_eq!(body["background"], true);
                async { HttpResponse::Ok().json(json!({"id":"created-before-stop","object":"response","status":"queued","output":[],"usage":null})) }
            }))
            .route("/v1/responses/created-before-stop", web::get().to(move || {
                g.fetch_add(1, Ordering::SeqCst);
                let complete = t.load(Ordering::SeqCst);
                async move { HttpResponse::Ok().json(if complete {
                    json!({"id":"created-before-stop","object":"response","status":"completed","output":[],"usage":{"input_tokens":1000,"output_tokens":1000,"total_tokens":2000}})
                } else {
                    json!({"id":"created-before-stop","object":"response","status":"queued","output":[],"usage":null})
                }) }
            }))
    }).workers(1).listen(listener).unwrap().run();
    let upstream_handle = upstream.handle();
    tokio::spawn(upstream);
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(&dir);
    config.gateway.pricing.source =
        crate::config::models::gateway::GatewayPricingConfig::default().source;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.providers = vec![crate::config::models::provider::ProviderConfig {
        name: "recovery-openai".into(),
        provider_type: "openai".into(),
        api_key: "sk-test-fake-key-12345678901234567890".into(),
        base_url: Some(format!("http://{upstream_address}/v1")),
        endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
        models: vec!["gpt-4o-mini".into()],
        ..Default::default()
    }];
    let creator_config = config.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
    // A separate runtime owns both the creator's HTTP workers/poller and its
    // ResponseSettlementTask. Joining it proves no creator task survives restart.
    let creator_thread = std::thread::spawn(move || {
        let runtime = actix_web::rt::System::new();
        runtime.block_on(async move {
            let creator = crate::server::HttpServer::new(&creator_config)
                .await
                .unwrap();
            let mut owner = User::new(
                "http-restart-owner".into(),
                "http-restart@example.com".into(),
                "unused-test-hash".into(),
            );
            owner.status = UserStatus::Active;
            creator
                .state()
                .storage
                .database
                .create_user(&owner)
                .await
                .unwrap();
            let (key, token) = creator
                .state()
                .auth
                .api_key()
                .create_key(
                    Some(owner.metadata.id),
                    None,
                    "http-restart".into(),
                    vec!["api.chat".into()],
                )
                .await
                .unwrap();
            let state = creator.state().clone();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let http = HttpServer::new(move || {
                App::new()
                    .app_data(web::Data::new(state.clone()))
                    .wrap(AuthMiddleware)
                    .configure(crate::server::routes::ai::configure_routes)
            })
            .workers(1)
            .listen(listener)
            .unwrap()
            .run();
            let handle = http.handle();
            tokio::spawn(http);
            ready_tx.send((address, key.metadata.id, token)).unwrap();
            let _ = stop_rx.await;
            handle.stop(false).await;
            drop(creator); // aborts its owned ResponseSettlementTask
        });
        drop(runtime); // aborts any detached creator tasks as a process stop would
    });
    let (creator_address, key_id, token) = ready_rx.await.unwrap();
    let response = reqwest::Client::new().post(format!("http://{creator_address}/v1/responses"))
        .header("authorization", format!("ApiKey {token}"))
        .json(&json!({"model":"gpt-4o-mini","input":"Hello","background":true,"store":true,"max_output_tokens":1000}))
        .send().await.unwrap();
    let status = response.status();
    let response: serde_json::Value = response.json().await.unwrap();
    assert_eq!(status, reqwest::StatusCode::OK, "{response}");
    assert_eq!(response["status"], "queued");
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    stop_tx.send(()).unwrap();
    tokio::task::spawn_blocking(move || creator_thread.join().unwrap())
        .await
        .unwrap();
    assert!(!terminal.load(Ordering::SeqCst));

    let storage = crate::storage::StorageLayer::new(&config.gateway.storage)
        .await
        .unwrap();
    let intent = Entity::find()
        .one(storage.database.connection())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(intent.response_id.as_deref(), Some("created-before-stop"));
    assert_eq!(intent.api_key_id, Some(key_id));
    assert!(!intent.complete);
    assert!(intent.cost.is_none());
    assert!(!intent.key_settled);
    assert!(!intent.pricing_json.is_empty());
    // Advance the persisted creator lease to its expiry without a nine-minute test.
    // The record itself, its response binding and frozen price came from the POST.
    storage
        .database
        .wake_response_settlement(&intent.id)
        .await
        .unwrap();
    terminal.store(true, Ordering::SeqCst);
    let (first, second) = tokio::join!(
        crate::server::HttpServer::new(&config),
        crate::server::HttpServer::new(&config)
    );
    let first = first.unwrap();
    let second = second.unwrap();
    // Only the normal startup workers drive recovery; no direct recover() call.
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        while !stored(second.state(), &intent.id).await.complete {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let settled = stored(second.state(), &intent.id).await;
    assert_eq!(settled.outcome, "actual");
    assert_eq!(settled.tokens, 2000);
    assert!((settled.cost.unwrap() - 0.00075).abs() < 1e-10);
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(second.state().clone()))
            .wrap(AuthMiddleware)
            .configure(crate::server::routes::ai::configure_routes),
    )
    .await;
    for _ in 0..3 {
        let result = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/v1/responses/created-before-stop")
                .insert_header(("authorization", format!("ApiKey {token}")))
                .to_request(),
        )
        .await;
        assert_eq!(result.status(), StatusCode::OK);
        let body: serde_json::Value = test::read_body_json(result).await;
        assert_eq!(body["status"], "completed");
    }
    let usage = storage
        .database
        .find_api_key_by_id(key_id)
        .await
        .unwrap()
        .unwrap()
        .usage_stats;
    assert_eq!(usage.total_requests, 1);
    assert_eq!(usage.total_tokens, 2000);
    assert!((usage.total_cost - 0.00075).abs() < 1e-10);
    assert!(gets.load(Ordering::SeqCst) >= 1);
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    drop(app);
    drop(first);
    drop(second);
    upstream_handle.stop(false).await;
}
