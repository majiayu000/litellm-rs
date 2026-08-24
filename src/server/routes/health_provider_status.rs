//! Per-provider readiness status derived from live router state.
//!
//! [`derive_status_for_provider`] uses the router's configured provider
//! identity and explicit active-probe evidence. Router request availability is
//! intentionally a separate, faster-changing signal.

use crate::core::router::{HealthStatus, UnifiedRouter};
use std::borrow::Cow;

/// Classify a provider from its deployment counts.
///
/// `healthy` requires a deployment with a successful probe and no conclusive
/// health failure. Deployments with no successful or conclusive failed probe
/// remain `unknown`.
fn classify(
    total: usize,
    probe_healthy: usize,
    has_failed_health: bool,
) -> (Cow<'static, str>, Option<String>) {
    match (total, probe_healthy, has_failed_health) {
        (0, _, _) => (
            Cow::Borrowed("unknown"),
            Some("no deployments registered for this provider".to_string()),
        ),
        (_, 0, true) => (
            Cow::Borrowed("unhealthy"),
            Some(format!(
                "{total} deployment(s), none currently probe-healthy"
            )),
        ),
        (_, 0, false) => (
            Cow::Borrowed("unknown"),
            Some("upstream health has not been established yet".to_string()),
        ),
        _ => (Cow::Borrowed("healthy"), None),
    }
}

/// Derive a provider's health status from the router's deployment state.
pub(super) fn derive_status_for_provider(
    router: &UnifiedRouter,
    name: &str,
    enabled: bool,
) -> (Cow<'static, str>, Option<String>) {
    if !enabled {
        return (Cow::Borrowed("disabled"), None);
    }

    let deployments = router.deployments_for_provider(name);
    let total = deployments.len();
    let mut probe_healthy = 0usize;
    let mut has_failed_health = false;
    for deployment in deployments {
        match deployment.state.probe_health_status() {
            HealthStatus::Healthy => probe_healthy += 1,
            HealthStatus::Unknown => {}
            HealthStatus::Degraded | HealthStatus::Unhealthy | HealthStatus::Cooldown => {
                has_failed_health = true;
            }
        }
    }

    classify(total, probe_healthy, has_failed_health)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;

    fn provider_config(name: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test-key".to_string(),
            models: vec!["readiness-model".to_string()],
            ..ProviderConfig::default()
        }
    }

    #[test]
    fn classify_no_deployments_reports_unknown() {
        let (status, _) = classify(0, 3, false);
        assert_eq!(status, "unknown");
    }

    #[test]
    fn classify_failed_probe_reports_unhealthy() {
        let (status, msg) = classify(2, 0, true);
        assert_eq!(status, "unhealthy");
        assert!(msg.unwrap().contains("2"));
    }

    #[test]
    fn classify_without_upstream_evidence_reports_unknown() {
        let (status, msg) = classify(2, 0, false);
        assert_eq!(status, "unknown");
        assert!(msg.unwrap().contains("not been established"));
    }

    #[test]
    fn classify_probe_healthy_reports_healthy() {
        let (status, msg) = classify(2, 1, false);
        assert_eq!(status, "healthy");
        assert!(msg.is_none());
    }

    #[tokio::test]
    async fn configured_name_and_probe_evidence_control_readiness() {
        let mut provider = provider_config("primary");
        provider.provider_type = "anthropic".to_string();
        provider.api_key = "sk-ant-test1234567890123".to_string();
        let router = UnifiedRouter::from_gateway_config(&[provider], None)
            .await
            .expect("gateway provider should construct");

        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
        let (canonical_status, _) = derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");

        router.record_success("primary-readiness-model", 1, 10);
        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");

        router
            .get_deployment("primary-readiness-model")
            .expect("configured-name deployment")
            .state
            .probe_health
            .store(
                HealthStatus::Healthy as u8,
                std::sync::atomic::Ordering::Release,
            );
        let (status, error) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "healthy");
        assert!(error.is_none());
    }

    #[tokio::test]
    async fn transient_capacity_does_not_change_probe_readiness() {
        let mut provider = provider_config("capacity");
        provider.max_concurrent_requests = 1;
        provider.rpm = 1;
        provider.tpm = 1;
        let router = UnifiedRouter::from_gateway_config(&[provider], None)
            .await
            .expect("gateway provider should construct");
        let deployment = router
            .get_deployment("capacity-readiness-model")
            .expect("capacity deployment");
        deployment.state.probe_health.store(
            HealthStatus::Healthy as u8,
            std::sync::atomic::Ordering::Release,
        );
        deployment
            .state
            .active_requests
            .store(1, std::sync::atomic::Ordering::Relaxed);
        deployment
            .state
            .rpm_current
            .store(1, std::sync::atomic::Ordering::Relaxed);
        deployment
            .state
            .tpm_current
            .store(1, std::sync::atomic::Ordering::Relaxed);

        let (status, _) = derive_status_for_provider(&router, "capacity", true);
        assert_eq!(status, "healthy");
    }

    #[tokio::test]
    async fn failed_degraded_threshold_and_recovery_are_non_optimistic() {
        let router = UnifiedRouter::from_gateway_config(&[provider_config("failed")], None)
            .await
            .expect("gateway provider should construct");
        let deployment = router
            .get_deployment("failed-readiness-model")
            .expect("failed deployment");

        deployment.state.probe_health.store(
            HealthStatus::Degraded as u8,
            std::sync::atomic::Ordering::Release,
        );
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "unhealthy");

        deployment.state.probe_health.store(
            HealthStatus::Unhealthy as u8,
            std::sync::atomic::Ordering::Release,
        );
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "unhealthy");

        deployment.state.probe_health.store(
            HealthStatus::Healthy as u8,
            std::sync::atomic::Ordering::Release,
        );
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "healthy");
    }
}
