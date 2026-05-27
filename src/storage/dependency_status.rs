//! Runtime status tracking for optional storage / runtime dependencies.
//!
//! Captures whether a configured dependency (database, redis, vector DB,
//! pricing service, budget persistence) is healthy, degraded, or unavailable.
//! Used to make the runtime state explicit so callers (and future readiness
//! reporting) can distinguish "intentionally off" from "configured but broken".

use serde::Serialize;

/// Status of a runtime-managed dependency.
///
/// Tracked per-dependency at startup so the gateway can report whether a
/// configured component is actually serving traffic, intentionally disabled,
/// or running in a degraded fallback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    /// Dependency is not configured / explicitly disabled. No fallback in
    /// effect; absence of the feature is intentional.
    Disabled,
    /// Dependency was configured but initialization has not been attempted
    /// yet (or no result is available). Reserved for future async health
    /// rechecks.
    Configured,
    /// Dependency initialized successfully and is in use.
    Healthy,
    /// Dependency was configured and failed to initialize, but the operator
    /// opted into `allow_degraded` so the gateway is running on an in-process
    /// / no-op fallback. Surfaced to make the trade-off visible.
    Degraded,
    /// Dependency was configured and failed to initialize. Not expected to
    /// be observed in normal operation because fail-fast is the default; this
    /// variant exists for completeness when reporting historical state.
    Unavailable,
}

impl DependencyStatus {
    /// Returns true when the dependency is actively serving traffic
    /// (regardless of whether through a real backend or a no-op fallback).
    pub fn is_available(self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Returns true when the dependency reported a configured-but-broken
    /// state and the gateway accepted a fallback.
    pub fn is_degraded(self) -> bool {
        matches!(self, Self::Degraded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_is_not_available() {
        assert!(!DependencyStatus::Disabled.is_available());
        assert!(!DependencyStatus::Disabled.is_degraded());
    }

    #[test]
    fn healthy_is_available_not_degraded() {
        assert!(DependencyStatus::Healthy.is_available());
        assert!(!DependencyStatus::Healthy.is_degraded());
    }

    #[test]
    fn degraded_is_available_and_degraded() {
        assert!(DependencyStatus::Degraded.is_available());
        assert!(DependencyStatus::Degraded.is_degraded());
    }

    #[test]
    fn unavailable_is_neither() {
        assert!(!DependencyStatus::Unavailable.is_available());
        assert!(!DependencyStatus::Unavailable.is_degraded());
    }

    #[test]
    fn serializes_to_snake_case() {
        let json = serde_json::to_string(&DependencyStatus::Healthy).unwrap();
        assert_eq!(json, "\"healthy\"");
        let json = serde_json::to_string(&DependencyStatus::Degraded).unwrap();
        assert_eq!(json, "\"degraded\"");
        let json = serde_json::to_string(&DependencyStatus::Disabled).unwrap();
        assert_eq!(json, "\"disabled\"");
    }
}
