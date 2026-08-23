//! Per-provider readiness status derived from live router state.
//!
//! [`derive_status_for_provider`] mirrors the router's own candidate filter
//! (`is_in_cooldown` / `is_healthy`), so readiness reflects what the router
//! would actually route to instead of a hardcoded placeholder.

use crate::core::router::UnifiedRouter;
use std::borrow::Cow;

/// Classify a provider from its deployment counts.
///
/// `healthy` when at least one deployment is routable now, `unhealthy` when
/// deployments exist but none is routable, `unknown` when none registered.
fn classify(total: usize, routable: usize) -> (Cow<'static, str>, Option<String>) {
    match (total, routable) {
        (0, _) => (
            Cow::Borrowed("unknown"),
            Some("no deployments registered for this provider".to_string()),
        ),
        (_, 0) => (
            Cow::Borrowed("unhealthy"),
            Some(format!("{total} deployment(s), none currently routable")),
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

    let mut total = 0usize;
    let mut routable = 0usize;
    for id in router.list_deployments() {
        let Some(deployment) = router.get_deployment(id.as_str()) else {
            continue;
        };
        if deployment.provider.name() != name {
            continue;
        }
        total += 1;
        if !deployment.is_in_cooldown() && deployment.is_healthy() {
            routable += 1;
        }
    }

    classify(total, routable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_no_deployments_reports_unknown() {
        let (status, _) = classify(0, 3);
        assert_eq!(status, "unknown");
    }

    #[test]
    fn classify_none_routable_reports_unhealthy() {
        let (status, msg) = classify(2, 0);
        assert_eq!(status, "unhealthy");
        assert!(msg.unwrap().contains("2"));
    }

    #[test]
    fn classify_routable_reports_healthy() {
        let (status, msg) = classify(2, 1);
        assert_eq!(status, "healthy");
        assert!(msg.is_none());
    }
}
