use super::AppState;
use crate::config::Config;
use crate::core::guardrails::PromptInjectionConfig;
use crate::core::router::{HealthStatus, RouterError};
use crate::server::HttpServer;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::Notify;

const MODEL: &str = "gpt-4o-mini";
const DEPLOYMENT: &str = "test-openai-gpt-4o-mini";

async fn app_state() -> AppState {
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.router.load_balancer.health_check_enabled = false;
    let provider = &mut config.gateway.providers[0];
    provider.models = vec![MODEL.into()];
    provider.max_concurrent_requests = 1;
    provider.rpm = 100;
    provider.tpm = 1000;
    HttpServer::new(&config).await.unwrap().state().clone()
}

async fn apply(state: &AppState, update: impl FnOnce(&mut Config)) {
    let mut config = (*state.config()).clone();
    update(&mut config);
    state.apply_runtime(config).await.unwrap();
}

#[tokio::test]
async fn executing_request_retains_parallel_slot_and_settles_into_reloaded_limits() {
    let state = app_state().await;
    let old = state.pin_runtime();
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let task = {
        let router = Arc::clone(&old.unified_router);
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        tokio::spawn(async move {
            router
                .execute_with_selected_deployment_retry(MODEL, move |deployment| {
                    let entered = Arc::clone(&entered);
                    let release = Arc::clone(&release);
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        Ok((deployment.model.clone(), 17))
                    }
                })
                .await
                .unwrap()
        })
    };
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();

    apply(&state, |config| {
        config
            .gateway
            .model_aliases
            .insert("chat".into(), MODEL.into());
        let provider = &mut config.gateway.providers[0];
        provider.weight = 7.0;
        provider.rpm = 1;
        provider.tpm = 17;
    })
    .await;
    let live = state.unified_router();
    assert!(matches!(
        live.select_deployment_lease("chat"),
        Err(RouterError::NoAvailableDeployment(_))
    ));
    assert!(!task.is_finished(), "the old request is still executing");

    release.notify_one();
    assert_eq!(task.await.unwrap().0, MODEL);
    let deployment = live.get_deployment(DEPLOYMENT).unwrap();
    assert_eq!(deployment.state.active_requests.load(Ordering::Acquire), 0);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Acquire), 1);
    assert_eq!(deployment.state.tpm_current.load(Ordering::Acquire), 17);
    // Completion in the old generation counts against the new limits.
    assert!(live.select_deployment_lease("chat").is_err());

    apply(&state, |config| {
        config.gateway.providers[0].rpm = 2;
        config.gateway.providers[0].tpm = 100;
    })
    .await;
    let lease = state
        .unified_router()
        .select_deployment_lease("chat")
        .unwrap();
    assert_eq!(
        lease.deployment().state.rpm_current.load(Ordering::Acquire),
        1
    );
    drop(lease);
}

#[tokio::test]
async fn removing_and_readding_a_busy_resource_does_not_create_an_extra_slot() {
    let state = app_state().await;
    let old = state.unified_router();
    let lease = old.select_deployment_lease(MODEL).unwrap();

    apply(&state, |config| config.gateway.providers[0].enabled = false).await;
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    // Also cover an intermediate revision with no matching deployment.
    apply(&state, |config| config.gateway.cache.enabled = true).await;
    apply(&state, |config| config.gateway.providers[0].enabled = true).await;
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );

    drop(lease);
    let next = state
        .unified_router()
        .select_deployment_lease(MODEL)
        .unwrap();
    drop(next);
}

#[tokio::test]
async fn credential_rotation_isolates_resources_but_rotation_back_retains_old_occupancy() {
    let state = app_state().await;
    let key_a = state.unified_router();
    let lease_a = key_a.select_deployment_lease(MODEL).unwrap();
    let original_key = state.config().gateway.providers[0].api_key.clone();

    apply(&state, |config| {
        config.gateway.providers[0].api_key = "sk-new-account".into()
    })
    .await;
    let key_b = state.unified_router();
    let lease_b = key_b
        .select_deployment_lease(MODEL)
        .expect("new credential has its own quota");
    assert_eq!(
        lease_b
            .deployment()
            .state
            .active_requests
            .load(Ordering::Acquire),
        1
    );
    drop(lease_a);
    assert_eq!(
        lease_b
            .deployment()
            .state
            .active_requests
            .load(Ordering::Acquire),
        1
    );
    // Hold another A request while B is active, then rotate back to A.
    let lease_a = key_a.select_deployment_lease(MODEL).unwrap();
    apply(&state, |config| {
        config.gateway.providers[0].api_key = original_key
    })
    .await;
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    drop(lease_b);
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    drop(lease_a);
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_ok()
    );
}

#[tokio::test]
async fn endpoint_and_model_changes_do_not_inherit_unrelated_inflight_state() {
    let state = app_state().await;
    let lease = state
        .unified_router()
        .select_deployment_lease(MODEL)
        .unwrap();
    apply(&state, |config| {
        config.gateway.providers[0].base_url = Some("https://different.example/v1".into());
    })
    .await;
    let new_endpoint = state
        .unified_router()
        .select_deployment_lease(MODEL)
        .unwrap();
    drop(lease);
    assert_eq!(
        new_endpoint
            .deployment()
            .state
            .active_requests
            .load(Ordering::Acquire),
        1
    );

    apply(&state, |config| {
        config.gateway.providers[0].models = vec!["gpt-4o".into()];
    })
    .await;
    let new_model = state
        .unified_router()
        .select_deployment_lease("gpt-4o")
        .unwrap();
    drop(new_endpoint);
    assert_eq!(
        new_model
            .deployment()
            .state
            .active_requests
            .load(Ordering::Acquire),
        1
    );
    drop(new_model);
}

#[tokio::test]
async fn expanding_model_list_keeps_existing_model_limits_and_cooldown() {
    let state = app_state().await;
    let lease = state
        .unified_router()
        .select_deployment_lease(MODEL)
        .unwrap();
    apply(&state, |config| {
        config.gateway.providers[0].models.push("gpt-4o".into())
    })
    .await;
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    assert!(
        state
            .unified_router()
            .select_deployment_lease("gpt-4o")
            .is_ok()
    );
    drop(lease);

    let old = state.unified_router().get_deployment(DEPLOYMENT).unwrap();
    old.enter_cooldown(300);
    apply(&state, |config| config.gateway.providers[0].priority = 2).await;
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    assert!(
        state
            .unified_router()
            .get_deployment(DEPLOYMENT)
            .unwrap()
            .is_in_cooldown()
    );
    assert!(
        !old.publish_probe_health(HealthStatus::Healthy),
        "old probe cannot publish after reload"
    );
}

#[tokio::test]
async fn failed_candidate_does_not_rebind_request_state_or_retire_probe_generation() {
    let state = app_state().await;
    let old = state.pin_runtime();
    let lease = old.unified_router.select_deployment_lease(MODEL).unwrap();
    let mut invalid = (*state.config()).clone();
    let mut injection = PromptInjectionConfig::new();
    injection.enabled = true;
    injection.custom_patterns.push("(".into());
    invalid.gateway.guardrails.prompt_injection = Some(injection);
    assert!(state.apply_runtime(invalid).await.is_err());
    assert_eq!(state.pin_runtime().generation, old.generation);
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_err()
    );
    assert!(
        lease
            .deployment()
            .publish_probe_health(HealthStatus::Healthy)
    );
    drop(lease);
    assert!(
        state
            .unified_router()
            .select_deployment_lease(MODEL)
            .is_ok()
    );
}
