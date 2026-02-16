//! Router integration tests
//!
//! Tests unified router functionality after legacy load balancer removal.

#[cfg(test)]
mod tests {
    use litellm_rs::core::providers::Provider;
    use litellm_rs::core::providers::openai::OpenAIProvider;
    use litellm_rs::core::router::deployment::{
        Deployment, DeploymentConfig, DeploymentState, HealthStatus,
    };
    use litellm_rs::core::router::{RouterConfig, UnifiedRouter, UnifiedRoutingStrategy};
    use std::sync::atomic::Ordering;

    async fn create_test_provider() -> Provider {
        let openai = OpenAIProvider::with_api_key("sk-test-key-for-router-tests")
            .await
            .expect("failed to create OpenAI provider");
        Provider::OpenAI(openai)
    }

    #[tokio::test]
    async fn test_unified_router_creation() {
        let router = UnifiedRouter::new(RouterConfig::default());
        assert_eq!(router.list_deployments().len(), 0);
    }

    #[tokio::test]
    async fn test_unified_router_add_and_select() {
        let router = UnifiedRouter::new(RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::RoundRobin,
            ..Default::default()
        });

        let provider = create_test_provider().await;
        let deployment = Deployment::new(
            "router-test-1".to_string(),
            provider,
            "gpt-4-turbo".to_string(),
            "gpt-4".to_string(),
        );

        deployment
            .state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);

        router.add_deployment(deployment);
        assert_eq!(router.list_deployments().len(), 1);

        let selected = router
            .select_deployment("gpt-4")
            .expect("selection should succeed");
        assert_eq!(selected, "router-test-1");
        router.release_deployment(&selected);
    }

    #[test]
    fn test_health_status_u8_roundtrip() {
        assert_eq!(HealthStatus::from(0), HealthStatus::Unknown);
        assert_eq!(HealthStatus::from(1), HealthStatus::Healthy);
        assert_eq!(HealthStatus::from(2), HealthStatus::Degraded);
        assert_eq!(HealthStatus::from(3), HealthStatus::Unhealthy);
        assert_eq!(HealthStatus::from(4), HealthStatus::Cooldown);

        assert_eq!(u8::from(HealthStatus::Unknown), 0);
        assert_eq!(u8::from(HealthStatus::Healthy), 1);
        assert_eq!(u8::from(HealthStatus::Degraded), 2);
        assert_eq!(u8::from(HealthStatus::Unhealthy), 3);
        assert_eq!(u8::from(HealthStatus::Cooldown), 4);
    }

    #[test]
    fn test_deployment_config_defaults() {
        let config = DeploymentConfig::default();
        assert!(config.tpm_limit.is_none());
        assert!(config.rpm_limit.is_none());
        assert!(config.max_parallel_requests.is_none());
        assert_eq!(config.weight, 1);
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.priority, 0);
    }

    #[test]
    fn test_deployment_state_new() {
        let state = DeploymentState::new();
        assert_eq!(state.health_status(), HealthStatus::Healthy);
        assert_eq!(state.tpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(state.active_requests.load(Ordering::Relaxed), 0);
    }
}
