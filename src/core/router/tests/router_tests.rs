//! Core router tests

#![allow(deprecated)]

use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::{Deployment, HealthStatus};
use crate::core::router::unified::Router;
use crate::core::router::{
    DefaultRuntimeBinding, RuntimeBinding, RuntimeRequestContext, RuntimeRequestOptions,
    default_runtime, install_default_runtime,
};
use crate::core::types::model::ProviderCapability;
use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

async fn create_test_provider() -> Provider {
    let openai = OpenAIProvider::with_api_key("sk-test-key-for-unit-testing-only")
        .await
        .expect("Failed to create OpenAI provider");
    Provider::OpenAI(openai)
}

pub(crate) async fn create_test_deployment(id: &str, model_name: &str) -> Deployment {
    let provider = create_test_provider().await;
    Deployment::new(
        id.to_string(),
        provider,
        format!("{}-turbo", model_name),
        model_name.to_string(),
    )
}

#[tokio::test]
async fn test_router_creation() {
    let router = Router::default();
    assert_eq!(router.list_models().len(), 0);
    assert_eq!(router.list_deployments().len(), 0);
}

#[tokio::test]
async fn test_router_with_custom_config() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LeastBusy,
        num_retries: 5,
        timeout_secs: 120,
        ..Default::default()
    };

    let router = Router::new(config);
    assert_eq!(router.config().routing_strategy, RoutingStrategy::LeastBusy);
    assert_eq!(router.config().num_retries, 5);
    assert_eq!(router.config().timeout_secs, 120);
}

#[tokio::test]
async fn test_add_deployment() {
    let router = Arc::new(Router::default());
    let binding = RuntimeBinding::new(router.clone());
    let old = binding.bind();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);
    let current = binding.bind();

    assert_eq!(router.list_deployments().len(), 1);
    assert_eq!(router.list_models().len(), 1);
    assert!(router.list_models().contains(&"gpt-4".to_string()));
    assert!(current.generation() > old.generation());
    assert!(old.select_deployment_lease("gpt-4").is_err());
    current.select_deployment_lease("gpt-4").unwrap();
}

#[tokio::test]
async fn test_add_multiple_deployments_same_model() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-2", "gpt-4").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    assert_eq!(router.list_deployments().len(), 2);
    assert_eq!(router.list_models().len(), 1);

    let deployments = router.get_deployments_for_model("gpt-4");
    assert_eq!(deployments.len(), 2);
}

#[tokio::test]
async fn test_add_same_deployment_id_does_not_duplicate_model_index() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    assert_eq!(router.list_deployments().len(), 1);
    assert_eq!(router.get_deployments_for_model("gpt-4"), vec!["test-1"]);
}

#[tokio::test]
async fn test_add_same_deployment_id_reindexes_when_model_changes() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-1", "claude").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    assert!(router.get_deployments_for_model("gpt-4").is_empty());
    assert_eq!(router.get_deployments_for_model("claude"), vec!["test-1"]);

    assert!(router.select_deployment("gpt-4").is_err());
    let selected = router.select_deployment("claude").unwrap();
    assert_eq!(selected, "test-1");
    router.release_deployment(&selected);
}

#[tokio::test]
async fn test_add_same_deployment_id_preserves_runtime_state_when_model_changes() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    deployment1.state.tpm_current.store(321, Ordering::Relaxed);
    deployment1.state.rpm_current.store(9, Ordering::Relaxed);
    deployment1
        .state
        .active_requests
        .store(3, Ordering::Relaxed);
    deployment1.enter_cooldown(60);

    router.add_deployment(deployment1);
    router.add_deployment(create_test_deployment("test-1", "claude").await);

    let current = router.get_deployment("test-1").unwrap();
    assert_eq!(current.model_name, "claude");
    assert_eq!(current.state.tpm_current.load(Ordering::Relaxed), 321);
    assert_eq!(current.state.rpm_current.load(Ordering::Relaxed), 9);
    assert_eq!(current.state.active_requests.load(Ordering::Relaxed), 3);
    assert_eq!(current.state.health_status(), HealthStatus::Cooldown);
    assert!(current.is_in_cooldown());
}

#[tokio::test]
async fn test_add_multiple_models() {
    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-2", "gpt-3.5-turbo").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);

    assert_eq!(router.list_deployments().len(), 2);
    assert_eq!(router.list_models().len(), 2);
    assert!(router.list_models().contains(&"gpt-4".to_string()));
    assert!(router.list_models().contains(&"gpt-3.5-turbo".to_string()));
}

#[tokio::test]
async fn test_ordered_model_groups_preserve_first_insertion_and_group_order() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("primary-1", "zz-primary").await);
    router.add_deployment(create_test_deployment("primary-2", "zz-primary").await);
    router.add_deployment(create_test_deployment("backup-1", "aa-backup").await);
    router
        .add_model_alias("primary-alias", "zz-primary")
        .unwrap();

    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["zz-primary", "aa-backup"]
    );
    assert_eq!(
        router.get_deployments_for_model("zz-primary"),
        vec!["primary-1", "primary-2"]
    );

    router.add_deployment(create_test_deployment("primary-1", "zz-primary").await);

    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["zz-primary", "aa-backup"]
    );
    assert_eq!(
        router.get_deployments_for_model("primary-alias"),
        vec!["primary-1", "primary-2"]
    );
}

#[tokio::test]
async fn test_ordered_model_groups_remove_only_last_and_reappend_removed_group() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("primary-1", "primary").await);
    router.add_deployment(create_test_deployment("primary-2", "primary").await);
    router.add_deployment(create_test_deployment("backup-1", "backup").await);

    router.remove_deployment("primary-1").unwrap();
    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["primary", "backup"]
    );
    assert_eq!(
        router.get_deployments_for_model("primary"),
        vec!["primary-2"]
    );

    router.remove_deployment("primary-2").unwrap();
    assert_eq!(router.list_models_in_insertion_order(), vec!["backup"]);
    assert!(router.get_deployments_for_model("primary").is_empty());

    router.add_deployment(create_test_deployment("primary-3", "primary").await);
    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["backup", "primary"]
    );
}

#[tokio::test]
async fn test_ordered_model_groups_reindex_to_existing_or_new_group() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("shared", "first").await);
    router.add_deployment(create_test_deployment("first-stable", "first").await);
    router.add_deployment(create_test_deployment("second-stable", "second").await);
    router.add_deployment(create_test_deployment("third-stable", "third").await);

    router.add_deployment(create_test_deployment("shared", "second").await);

    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["first", "second", "third"]
    );
    assert_eq!(
        router.get_deployments_for_model("first"),
        vec!["first-stable"]
    );
    assert_eq!(
        router.get_deployments_for_model("second"),
        vec!["second-stable", "shared"]
    );

    router.add_deployment(create_test_deployment("first-stable", "fourth").await);

    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["second", "third", "fourth"]
    );
    assert!(router.get_deployments_for_model("first").is_empty());
    assert_eq!(
        router.get_deployments_for_model("fourth"),
        vec!["first-stable"]
    );
}

#[tokio::test]
async fn test_get_deployment() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);

    let retrieved = router.get_deployment("test-1");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "test-1");

    let not_found = router.get_deployment("nonexistent");
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_remove_deployment() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);
    assert_eq!(router.list_deployments().len(), 1);

    let removed = router.remove_deployment("test-1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().id, "test-1");
    assert_eq!(router.list_deployments().len(), 0);

    let deployments = router.get_deployments_for_model("gpt-4");
    assert_eq!(deployments.len(), 0);
}

#[tokio::test]
async fn test_set_model_list() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("test-1", "gpt-4").await);
    router.add_deployment(create_test_deployment("test-2", "gpt-3.5-turbo").await);
    assert_eq!(router.list_deployments().len(), 2);

    let new_deployments = vec![
        create_test_deployment("test-3", "claude-3").await,
        create_test_deployment("test-4", "claude-3").await,
    ];

    router.set_model_list(new_deployments);

    assert_eq!(router.list_deployments().len(), 2);
    assert_eq!(router.list_models().len(), 1);
    assert!(router.list_models().contains(&"claude-3".to_string()));
    assert!(!router.list_models().contains(&"gpt-4".to_string()));
}

#[tokio::test]
async fn test_set_model_list_duplicate_id_reindexes_without_empty_old_model() {
    let router = Router::default();

    router.set_model_list(vec![
        create_test_deployment("test-1", "gpt-4").await,
        create_test_deployment("test-1", "claude").await,
    ]);

    assert!(!router.list_models().contains(&"gpt-4".to_string()));
    assert!(router.get_deployments_for_model("gpt-4").is_empty());
    assert_eq!(router.get_deployments_for_model("claude"), vec!["test-1"]);
}

#[tokio::test]
async fn test_set_model_list_installs_complete_snapshot_generation() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("old-1", "gpt-4").await);
    let old_snapshot = router.routing_snapshot.load_full();

    router.set_model_list(vec![create_test_deployment("new-1", "claude").await]);
    let new_snapshot = router.routing_snapshot.load_full();

    assert!(old_snapshot.deployments.contains_key("old-1"));
    assert_eq!(
        old_snapshot.model_index.get("gpt-4"),
        Some(&vec!["old-1".to_string()])
    );
    assert!(!old_snapshot.deployments.contains_key("new-1"));

    assert!(new_snapshot.deployments.contains_key("new-1"));
    assert_eq!(
        new_snapshot.model_index.get("claude"),
        Some(&vec!["new-1".to_string()])
    );
    assert!(!new_snapshot.deployments.contains_key("old-1"));
    assert!(!new_snapshot.model_index.contains_key("gpt-4"));
}

#[tokio::test]
async fn test_set_model_list_publishes_input_first_occurrence_order_atomically() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("old-1", "old-first").await);
    router.add_deployment(create_test_deployment("old-2", "old-second").await);
    let old_snapshot = router.routing_snapshot.load_full();

    router.set_model_list(vec![
        create_test_deployment("beta-1", "beta").await,
        create_test_deployment("alpha-1", "alpha").await,
        create_test_deployment("beta-2", "beta").await,
        create_test_deployment("gamma-1", "gamma").await,
    ]);
    let new_snapshot = router.routing_snapshot.load_full();

    assert_eq!(old_snapshot.model_order, vec!["old-first", "old-second"]);
    assert!(old_snapshot.deployments.contains_key("old-1"));
    assert!(!old_snapshot.deployments.contains_key("beta-1"));

    assert_eq!(new_snapshot.model_order, vec!["beta", "alpha", "gamma"]);
    assert_eq!(
        new_snapshot.model_index.get("beta"),
        Some(&vec!["beta-1".to_string(), "beta-2".to_string()])
    );
    assert!(!new_snapshot.deployments.contains_key("old-1"));
    assert!(new_snapshot.deployments.contains_key("gamma-1"));
    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["beta", "alpha", "gamma"]
    );
}

#[tokio::test]
async fn test_set_model_list_keeps_first_group_order_when_duplicate_id_moves_groups() {
    let router = Router::default();

    router.set_model_list(vec![
        create_test_deployment("shared-id", "alpha").await,
        create_test_deployment("shared-id", "beta").await,
        create_test_deployment("alpha-id", "alpha").await,
    ]);

    assert_eq!(
        router.list_models_in_insertion_order(),
        vec!["alpha", "beta"]
    );
    assert_eq!(router.get_deployments_for_model("alpha"), vec!["alpha-id"]);
    assert_eq!(router.get_deployments_for_model("beta"), vec!["shared-id"]);
}

#[tokio::test]
async fn test_deployment_lease_releases_selected_snapshot_after_same_id_swap() {
    let router = Router::default();

    router.add_deployment(create_test_deployment("shared-id", "gpt-4").await);
    let lease = router.select_deployment_lease("gpt-4").unwrap();
    let selected_state = lease.deployment().state.clone();
    assert_eq!(selected_state.active_requests.load(Ordering::Relaxed), 1);

    router.set_model_list(vec![create_test_deployment("shared-id", "gpt-4").await]);

    let current = router.get_deployment("shared-id").unwrap();
    assert_eq!(current.state.active_requests.load(Ordering::Relaxed), 1);
    assert_eq!(selected_state.active_requests.load(Ordering::Relaxed), 1);

    drop(lease);

    assert_eq!(selected_state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(current.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_set_model_list_preserves_runtime_state_for_existing_deployment_id() {
    let router = Router::default();
    let deployment = create_test_deployment("shared-id", "gpt-4").await;
    deployment.state.tpm_current.store(123, Ordering::Relaxed);
    deployment.state.rpm_current.store(7, Ordering::Relaxed);
    deployment.state.active_requests.store(2, Ordering::Relaxed);
    deployment.enter_cooldown(60);
    router.add_deployment(deployment);

    router.set_model_list(vec![create_test_deployment("shared-id", "claude").await]);

    let current = router.get_deployment("shared-id").unwrap();
    assert_eq!(current.model_name, "claude");
    assert_eq!(current.state.tpm_current.load(Ordering::Relaxed), 123);
    assert_eq!(current.state.rpm_current.load(Ordering::Relaxed), 7);
    assert_eq!(current.state.active_requests.load(Ordering::Relaxed), 2);
    assert_eq!(current.state.health_status(), HealthStatus::Cooldown);
    assert!(current.is_in_cooldown());
}

#[tokio::test]
async fn test_model_aliases() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);
    router.add_model_alias("gpt4", "gpt-4").unwrap();
    router.add_model_alias("gpt-4-latest", "gpt-4").unwrap();
    router.add_model_alias("best", "gpt4").unwrap();

    assert_eq!(router.resolve_model_name("gpt4"), "gpt-4");
    assert_eq!(router.resolve_model_name("gpt-4-latest"), "gpt-4");
    assert_eq!(router.resolve_model_name("best"), "gpt-4");
    assert_eq!(router.resolve_model_name("gpt-4"), "gpt-4");
    assert_eq!(router.resolve_model_name("unknown"), "unknown");

    let deployments1 = router.get_deployments_for_model("gpt-4");
    let deployments2 = router.get_deployments_for_model("gpt4");
    let deployments3 = router.get_deployments_for_model("gpt-4-latest");
    let deployments4 = router.get_deployments_for_model("best");

    assert_eq!(deployments1.len(), 1);
    assert_eq!(deployments2.len(), 1);
    assert_eq!(deployments3.len(), 1);
    assert_eq!(deployments4.len(), 1);
    assert_eq!(deployments1, deployments2);
    assert_eq!(deployments2, deployments3);
    assert_eq!(deployments3, deployments4);

    let selected = router.select_deployment("best").unwrap();
    assert_eq!(selected, "test-1");
    router.release_deployment(&selected);
}

#[tokio::test]
async fn test_get_healthy_deployments() {
    use crate::core::router::deployment::HealthStatus;

    let router = Router::default();
    let deployment1 = create_test_deployment("test-1", "gpt-4").await;
    let deployment2 = create_test_deployment("test-2", "gpt-4").await;
    let deployment3 = create_test_deployment("test-3", "gpt-4").await;

    router.add_deployment(deployment1);
    router.add_deployment(deployment2);
    router.add_deployment(deployment3);

    let healthy = router.get_healthy_deployments("gpt-4");
    assert_eq!(healthy.len(), 3);

    if let Some(d) = router.get_deployment("test-1") {
        d.state
            .health
            .store(HealthStatus::Unhealthy as u8, Ordering::Relaxed);
    }

    let healthy = router.get_healthy_deployments("gpt-4");
    assert_eq!(healthy.len(), 2);
    assert!(healthy.contains(&"test-2".to_string()));
    assert!(healthy.contains(&"test-3".to_string()));

    if let Some(d) = router.get_deployment("test-2") {
        d.enter_cooldown(60);
    }

    let healthy = router.get_healthy_deployments("gpt-4");
    assert_eq!(healthy.len(), 1);
}

#[tokio::test]
async fn test_select_capability_deployment() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    let selected = router
        .select_capability_deployment("gpt-4", &ProviderCapability::ChatCompletion)
        .expect("chat-capable deployment should exist");

    assert_eq!(selected.deployment_id, "test-1");
    assert_eq!(selected.model, "gpt-4-turbo");
}

#[tokio::test]
async fn test_select_capability_deployment_with_alias() {
    let router = Router::default();
    router.add_deployment(create_test_deployment("test-1", "gpt-4").await);
    router.add_model_alias("gpt4", "gpt-4").unwrap();

    let selected = router
        .select_capability_deployment("gpt4", &ProviderCapability::ChatCompletion)
        .expect("alias should resolve to a chat-capable deployment");

    assert_eq!(selected.deployment_id, "test-1");
}

#[tokio::test]
async fn test_record_success() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_success("test-1", 1000, 50_000);

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.success_requests.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.tpm_current.load(Ordering::Relaxed), 1000);
        assert_eq!(d.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), 50_000);
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_record_failure() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.total_requests.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.fail_requests.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.fails_this_minute.load(Ordering::Relaxed), 1);
    } else {
        panic!("Deployment not found");
    }
}

#[tokio::test]
async fn test_minute_reset() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;
    router.add_deployment(deployment);

    router.record_success("test-1", 1000, 50_000);
    router.record_failure("test-1");

    if let Some(d) = router.get_deployment("test-1") {
        assert_eq!(d.state.tpm_current.load(Ordering::Relaxed), 1000);
        assert_eq!(d.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.fails_this_minute.load(Ordering::Relaxed), 1);

        router.reset_minute_counters();

        assert_eq!(d.state.tpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(d.state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(d.state.fails_this_minute.load(Ordering::Relaxed), 0);

        assert_eq!(d.state.total_requests.load(Ordering::Relaxed), 2);
        assert_eq!(d.state.success_requests.load(Ordering::Relaxed), 1);
        assert_eq!(d.state.fail_requests.load(Ordering::Relaxed), 1);
    } else {
        panic!("Deployment not found");
    }
}

#[test]
fn test_routing_strategy_default() {
    assert_eq!(RoutingStrategy::default(), RoutingStrategy::SimpleShuffle);
}

#[test]
fn test_router_config_default() {
    let config = RouterConfig::default();
    assert_eq!(config.routing_strategy, RoutingStrategy::SimpleShuffle);
    assert_eq!(config.num_retries, 3);
    assert_eq!(config.retry_after_secs, 0);
    assert_eq!(config.allowed_fails, 3);
    assert_eq!(config.cooldown_time_secs, 5);
    assert_eq!(config.timeout_secs, 60);
    assert_eq!(config.max_fallbacks, 5);
    assert!(config.enable_pre_call_checks);
}

// ==================== Alias Cycle Detection Tests ====================

#[test]
fn test_alias_direct_cycle() {
    let router = Router::default();
    router.add_model_alias("a", "b").unwrap();
    let generation = router.routing_snapshot.load().generation();
    let result = router.add_model_alias("b", "a");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Circular alias"));
    assert_eq!(router.routing_snapshot.load().generation(), generation);
}

#[test]
fn test_alias_transitive_cycle() {
    let router = Router::default();
    router.add_model_alias("a", "b").unwrap();
    router.add_model_alias("b", "c").unwrap();
    let result = router.add_model_alias("c", "a");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Circular alias"));
}

#[test]
fn test_alias_self_cycle() {
    let router = Router::default();
    let result = router.add_model_alias("a", "a");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Circular alias"));
}

#[test]
fn test_alias_no_cycle() {
    let router = Arc::new(Router::default());
    let binding = RuntimeBinding::new(router.clone());
    let old = binding.bind();
    assert!(router.add_model_alias("gpt4", "gpt-4").is_ok());
    assert!(router.add_model_alias("gpt-latest", "gpt-4").is_ok());
    assert!(router.add_model_alias("best", "gpt-latest").is_ok());
    let current = binding.bind();
    assert_eq!(old.snapshot().resolve_model_name("best"), "best");
    assert_eq!(current.snapshot().resolve_model_name("best"), "gpt-4");
}

#[test]
fn test_runtime_binding_rollback_and_request_context_validation() {
    let first = RuntimeBinding::new(Arc::new(Router::default()));
    let default = DefaultRuntimeBinding::new(first.clone());
    let first_generation = default.load().generation();
    let old = default.replace(RuntimeBinding::new(Arc::new(Router::default())));
    let second_generation = default.load().generation();
    default.replace(old);
    assert!(second_generation > first_generation);
    assert!(default.load().generation() > second_generation);
    let installed = install_default_runtime(first.clone()).unwrap().generation();
    assert!(install_default_runtime(first).is_err());
    assert_eq!(default_runtime().unwrap().generation(), installed);

    let mut policy = RouterConfig::default();
    let mut options = RuntimeRequestOptions {
        headers: Some([("authorization".into(), "secret".into())].into()),
        ..Default::default()
    };
    assert!(RuntimeRequestContext::validate(std::mem::take(&mut options), &policy).is_err());
    options.timeout = Some(Duration::ZERO);
    assert!(RuntimeRequestContext::validate(std::mem::take(&mut options), &policy).is_err());
    options.api_base = Some("file:///tmp/socket".into());
    assert!(RuntimeRequestContext::validate(std::mem::take(&mut options), &policy).is_err());
    policy.timeout_secs = 0;
    options.timeout = Some(Duration::from_secs(1));
    options.api_base = Some(" https://example.com/v1 ".into());
    assert!(RuntimeRequestContext::validate(options, &policy).is_ok());
}
