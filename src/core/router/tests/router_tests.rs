//! Core router tests

use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::Deployment;
use crate::core::router::unified::Router;
use crate::core::types::model::ProviderCapability;
use std::sync::atomic::Ordering;

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
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);

    assert_eq!(router.list_deployments().len(), 1);
    assert_eq!(router.list_models().len(), 1);
    assert!(router.list_models().contains(&"gpt-4".to_string()));
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
async fn test_model_aliases() {
    let router = Router::default();
    let deployment = create_test_deployment("test-1", "gpt-4").await;

    router.add_deployment(deployment);
    router.add_model_alias("gpt4", "gpt-4").unwrap();
    router.add_model_alias("gpt-4-latest", "gpt-4").unwrap();

    assert_eq!(router.resolve_model_name("gpt4"), "gpt-4");
    assert_eq!(router.resolve_model_name("gpt-4-latest"), "gpt-4");
    assert_eq!(router.resolve_model_name("gpt-4"), "gpt-4");
    assert_eq!(router.resolve_model_name("unknown"), "unknown");

    let deployments1 = router.get_deployments_for_model("gpt-4");
    let deployments2 = router.get_deployments_for_model("gpt4");
    let deployments3 = router.get_deployments_for_model("gpt-4-latest");

    assert_eq!(deployments1.len(), 1);
    assert_eq!(deployments2.len(), 1);
    assert_eq!(deployments3.len(), 1);
    assert_eq!(deployments1, deployments2);
    assert_eq!(deployments2, deployments3);
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
    let result = router.add_model_alias("b", "a");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Circular alias"));
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
    let router = Router::default();
    assert!(router.add_model_alias("gpt4", "gpt-4").is_ok());
    assert!(router.add_model_alias("gpt-latest", "gpt-4").is_ok());
    assert!(router.add_model_alias("best", "gpt-latest").is_ok());
}
