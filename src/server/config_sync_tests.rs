use super::*;
use crate::server::HttpServer;

fn sync(key: u8) -> Arc<ConfigSync> {
    Arc::new(ConfigSync {
        key: vec![key; 32],
        status: Default::default(),
    })
}

async fn state() -> AppState {
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    let server = HttpServer::new(&config).await.unwrap();
    let mut state = server.state().clone();
    state.config_sync = Some(sync(7));
    state
}

#[test]
fn snapshot_is_authenticated_and_excludes_node_local_settings() {
    let mut config = crate::server::valid_test_config();
    config.gateway.providers[0].api_key = "secret-provider-sentinel".into();
    let crypt = sync(7);
    let ciphertext = crypt.encrypt(&config, 4).unwrap();
    assert!(!ciphertext.contains("secret-provider-sentinel"));
    let json = decrypt_data(&crypt.key, &ciphertext).unwrap();
    assert!(json.contains("secret-provider-sentinel"));
    assert!(!json.contains("jwt_secret"));
    assert!(!json.contains("database"));
    let mut other = config.clone();
    other.gateway.server.port = 8123;
    let restored = crypt.candidate(&other, 4, &ciphertext).unwrap();
    assert_eq!(restored.gateway.server.port, 8123);
    assert_eq!(
        restored.gateway.providers[0].api_key,
        "secret-provider-sentinel"
    );
    assert!(crypt.candidate(&other, 5, &ciphertext).is_err());
    assert!(sync(9).candidate(&other, 4, &ciphertext).is_err());
}

#[tokio::test]
async fn two_nodes_converge_and_duplicates_do_not_rebuild() {
    let a = state().await;
    let mut b = state().await;
    b.storage = a.storage.clone();
    let mut candidate = (*a.config()).clone();
    candidate.gateway.providers[0].weight = 7.0;
    // Disabled Redis makes notification fail after the durable commit.
    assert!(a.apply_runtime(candidate).await.is_err());
    assert_eq!(a.pin_runtime().generation, 1);
    assert!(a.config_sync_status().last_sync_error.is_some());
    b.reconcile_runtime_config().await.unwrap();
    assert_eq!(b.pin_runtime().generation, 1);
    assert_eq!(b.config().gateway.providers[0].weight, 7.0);
    let pin = b.pin_runtime();
    b.reconcile_runtime_config().await.unwrap();
    assert!(Arc::ptr_eq(&pin, &b.pin_runtime()));
    assert!(matches!(
        b.apply_runtime_at((*b.config()).clone(), 0).await,
        Err(GatewayError::Conflict(_))
    ));
}

#[tokio::test]
async fn concurrent_database_writers_cannot_overwrite_a_new_revision() {
    let a = state().await;
    let mut b = state().await;
    b.storage = a.storage.clone();
    let mut first = (*a.config()).clone();
    first.gateway.providers[0].weight = 3.0;
    let mut second = (*b.config()).clone();
    second.gateway.providers[0].weight = 8.0;
    let (left, right) = tokio::join!(a.apply_runtime(first), b.apply_runtime(second));
    assert_eq!(
        [left, right]
            .iter()
            .filter(|r| matches!(r, Err(GatewayError::Conflict(_))))
            .count(),
        1
    );
    a.reconcile_runtime_config().await.unwrap();
    b.reconcile_runtime_config().await.unwrap();
    assert_eq!(a.pin_runtime().generation, 1);
    assert_eq!(b.pin_runtime().generation, 1);
    assert_eq!(
        a.config().gateway.providers[0].weight,
        b.config().gateway.providers[0].weight
    );
}

#[tokio::test]
async fn one_nodes_decryption_failure_keeps_healthy_nodes_active_and_is_visible() {
    let a = state().await;
    let mut b = state().await;
    b.storage = a.storage.clone();
    b.config_sync = Some(sync(9));
    let before = b.pin_runtime();
    assert!(a.apply_runtime((*a.config()).clone()).await.is_err());
    assert!(b.reconcile_runtime_config().await.is_err());
    assert!(Arc::ptr_eq(&before, &b.pin_runtime()));
    assert_eq!(a.pin_runtime().generation, 1);
    let status = b.config_sync_status();
    assert_eq!(status.observed_revision, 1);
    assert!(
        status
            .last_apply_error
            .unwrap()
            .contains("decryption failed")
    );
    b.config_sync = Some(sync(7));
    b.reconcile_runtime_config().await.unwrap();
    assert_eq!(b.pin_runtime().generation, 1);
    assert!(b.config_sync_status().last_apply_error.is_none());
}

#[tokio::test]
async fn invalid_candidate_never_changes_authoritative_revision() {
    let state = state().await;
    let mut candidate = (*state.config()).clone();
    candidate.gateway.providers[0].weight = -1.0;
    assert!(state.apply_runtime_at(candidate, 0).await.is_err());
    assert_eq!(
        state
            .storage
            .database
            .runtime_config()
            .await
            .unwrap()
            .revision,
        0
    );
    assert_eq!(state.pin_runtime().generation, 0);
    assert!(state.config_sync_status().last_apply_error.is_some());
}

#[tokio::test]
async fn revision_diagnostics_require_authentication_and_never_return_ciphertext() {
    use actix_web::{http::StatusCode, test, web};
    let state = state().await;
    let app = test::init_service(HttpServer::create_app(web::Data::new(state.clone()))).await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/routing/revision")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = test::read_body_json(response).await;
    assert_eq!(body["active_revision"], 0);
    assert!(body.get("encrypted_payload").is_none());
    let mut config = (*state.config()).clone();
    config.gateway.auth.enable_api_key = true;
    state.config.store(config);
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/admin/routing/revision")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn redis_broadcasts_only_ids_and_reconnect_reads_latest() {
    let Ok(url) = std::env::var("REDIS_URL") else {
        assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
        return;
    };
    let mut a = state().await;
    let mut storage = (*a.storage).clone();
    storage.redis = Arc::new(
        crate::storage::redis::RedisPool::new(&crate::config::models::storage::RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    a.storage = Arc::new(storage);
    let mut b = state().await;
    b.storage = a.storage.clone();
    let mut capture = a
        .storage
        .redis
        .subscribe(&[REVISION_CHANNEL.into()])
        .await
        .unwrap();
    a.apply_runtime((*a.config()).clone()).await.unwrap();
    let message = tokio::time::timeout(Duration::from_secs(3), capture.next_message())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(message.get_payload::<String>().unwrap(), "1");
    let task = start(b.clone()).unwrap();
    async fn reached(state: &AppState, revision: u64) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while state.pin_runtime().generation != revision {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("node must converge");
    }
    reached(&b, 1).await;
    drop(task); // Lose notifications while disconnected.
    a.apply_runtime((*a.config()).clone()).await.unwrap();
    a.apply_runtime((*a.config()).clone()).await.unwrap();
    let _task = start(b.clone()).unwrap();
    reached(&b, 3).await;
    let pin = b.pin_runtime();
    a.storage
        .redis
        .publish(REVISION_CHANNEL, "3")
        .await
        .unwrap();
    a.storage
        .redis
        .publish(REVISION_CHANNEL, "1")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        Arc::ptr_eq(&pin, &b.pin_runtime()),
        "duplicate/out-of-order events must not rebuild"
    );
    capture.unsubscribe_all().await.unwrap();
}

#[tokio::test]
async fn notification_failure_preserves_committed_admin_revision_records() {
    use actix_web::{http::StatusCode, test, web};
    let state = state().await;
    let provider = state.config().gateway.providers[0].name.clone();
    let app = test::init_service(HttpServer::create_app(web::Data::new(state.clone()))).await;
    for (path, body, method) in [
        (
            format!("/admin/providers/{provider}"),
            serde_json::json!({"weight": 2}),
            "PATCH",
        ),
        (
            "/admin/routing/policy".into(),
            serde_json::json!({"model_aliases": {}}),
            "PUT",
        ),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::default()
                .method(method.parse().unwrap())
                .uri(&path)
                .set_json(body)
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: serde_json::Value = test::read_body_json(response).await;
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("committed and applied")
        );
    }
    assert_eq!(state.pin_runtime().generation, 2);
    assert_eq!(
        state
            .storage
            .database
            .latest_provider_config_revision()
            .await
            .unwrap()
            .unwrap()
            .generation,
        1
    );
    assert_eq!(
        state
            .storage
            .database
            .latest_routing_policy_revision()
            .await
            .unwrap()
            .unwrap()
            .generation,
        2
    );
}
