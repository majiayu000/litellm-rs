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
        (_, _, true) => (
            Cow::Borrowed("unhealthy"),
            Some(format!(
                "{total} deployment(s), at least one probe is unhealthy"
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
    fn classify_failed_probe_takes_priority_over_healthy_probe() {
        let (status, _) = classify(2, 1, true);
        assert_eq!(status, "unhealthy");
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

        let (status, error) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
        assert_eq!(
            error.as_deref(),
            Some("upstream health has not been established yet")
        );
        let (canonical_status, canonical_error) =
            derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");
        assert_eq!(
            canonical_error.as_deref(),
            Some("no deployments registered for this provider")
        );

        router.record_success("primary-readiness-model", 1, 10);
        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");

        let old_deployment = router
            .get_deployment("primary-readiness-model")
            .expect("configured-name deployment");
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        let (status, error) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "healthy");
        assert!(error.is_none());

        let total_requests = old_deployment
            .state
            .total_requests
            .load(std::sync::atomic::Ordering::Relaxed);
        let replacement = old_deployment.as_ref().clone();
        router.set_model_list(vec![replacement]);
        let (status, error) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
        assert_eq!(
            error.as_deref(),
            Some("upstream health has not been established yet")
        );
        let (canonical_status, canonical_error) =
            derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");
        assert_eq!(
            canonical_error.as_deref(),
            Some("no deployments registered for this provider")
        );

        let replacement = router
            .get_deployment("primary-readiness-model")
            .expect("replacement deployment");
        assert_eq!(
            replacement
                .state
                .total_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            total_requests
        );
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Unknown);
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        assert_eq!(
            replacement.state.probe_health_status(),
            HealthStatus::Unknown
        );
        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
    }

    #[tokio::test]
    async fn add_deployment_preserves_name_but_invalidates_probe_evidence() {
        let mut provider = provider_config("primary");
        provider.provider_type = "anthropic".to_string();
        provider.api_key = "sk-ant-test1234567890123".to_string();
        let router = UnifiedRouter::from_gateway_config(&[provider], None)
            .await
            .expect("gateway provider should construct");
        let old_deployment = router
            .get_deployment("primary-readiness-model")
            .expect("configured-name deployment");
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "healthy");

        router.add_deployment(old_deployment.as_ref().clone());
        let replacement = router
            .get_deployment("primary-readiness-model")
            .expect("replacement deployment");
        let (status, error) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
        assert_eq!(
            error.as_deref(),
            Some("upstream health has not been established yet")
        );
        let (canonical_status, canonical_error) =
            derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");
        assert_eq!(
            canonical_error.as_deref(),
            Some("no deployments registered for this provider")
        );

        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Unknown);
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        assert_eq!(
            replacement.state.probe_health_status(),
            HealthStatus::Unknown
        );
        let (status, _) = derive_status_for_provider(&router, "primary", true);
        assert_eq!(status, "unknown");
    }

    #[tokio::test]
    async fn removed_deployments_reenter_with_fresh_probe_evidence() {
        let mut provider = provider_config("primary");
        provider.provider_type = "anthropic".to_string();
        provider.api_key = "sk-ant-test1234567890123".to_string();
        let router = UnifiedRouter::from_gateway_config(&[provider], None)
            .await
            .expect("gateway provider should construct");
        let old_deployment = router
            .get_deployment("primary-readiness-model")
            .expect("configured-name deployment");
        old_deployment
            .state
            .active_requests
            .store(7, std::sync::atomic::Ordering::Relaxed);
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        let removed = router
            .remove_deployment("primary-readiness-model")
            .expect("deployment should be removed");

        router.add_deployment(removed);
        let readded = router
            .get_deployment("primary-readiness-model")
            .expect("deployment should be re-added");
        assert_eq!(
            readded
                .state
                .active_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            7
        );
        assert_eq!(readded.state.probe_health_status(), HealthStatus::Unknown);
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Unknown);
        old_deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        assert_eq!(readded.state.probe_health_status(), HealthStatus::Unknown);
        let (configured_status, configured_error) =
            derive_status_for_provider(&router, "primary", true);
        assert_eq!(configured_status, "unknown");
        assert_eq!(
            configured_error.as_deref(),
            Some("no deployments registered for this provider")
        );
        let (canonical_status, canonical_error) =
            derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");
        assert_eq!(
            canonical_error.as_deref(),
            Some("upstream health has not been established yet")
        );

        readded
            .state
            .active_requests
            .store(9, std::sync::atomic::Ordering::Relaxed);
        readded.state.set_probe_health_status(HealthStatus::Healthy);
        let removed = router
            .remove_deployment("primary-readiness-model")
            .expect("re-added deployment should be removed");
        router.set_model_list(vec![removed]);
        let bulk_readded = router
            .get_deployment("primary-readiness-model")
            .expect("deployment should be bulk re-added");
        assert_eq!(
            bulk_readded
                .state
                .active_requests
                .load(std::sync::atomic::Ordering::Relaxed),
            9
        );
        assert_eq!(
            bulk_readded.state.probe_health_status(),
            HealthStatus::Unknown
        );
        readded.state.set_probe_health_status(HealthStatus::Unknown);
        readded.state.set_probe_health_status(HealthStatus::Healthy);
        assert_eq!(
            bulk_readded.state.probe_health_status(),
            HealthStatus::Unknown
        );
        let (configured_status, configured_error) =
            derive_status_for_provider(&router, "primary", true);
        assert_eq!(configured_status, "unknown");
        assert_eq!(
            configured_error.as_deref(),
            Some("no deployments registered for this provider")
        );
        let (canonical_status, canonical_error) =
            derive_status_for_provider(&router, "anthropic", true);
        assert_eq!(canonical_status, "unknown");
        assert_eq!(
            canonical_error.as_deref(),
            Some("upstream health has not been established yet")
        );
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
        deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
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

        deployment
            .state
            .set_probe_health_status(HealthStatus::Degraded);
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "unhealthy");

        deployment
            .state
            .set_probe_health_status(HealthStatus::Unhealthy);
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "unhealthy");

        deployment
            .state
            .set_probe_health_status(HealthStatus::Healthy);
        let (status, _) = derive_status_for_provider(&router, "failed", true);
        assert_eq!(status, "healthy");
    }
}
