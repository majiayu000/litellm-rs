//! Stable resource identity for gateway runtime revisions.

use crate::config::models::provider::ProviderConfig;
use sha2::{Digest, Sha256};
use std::fmt;

/// Construction-time digest: no raw credential or provider settings are
/// retained in the state registry, serialized, or exposed through Debug.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct GatewayRuntimeIdentity([u8; 32]);

impl fmt::Debug for GatewayRuntimeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GatewayRuntimeIdentity([REDACTED])")
    }
}

impl GatewayRuntimeIdentity {
    /// Use effective construction inputs, after credentials have been resolved
    /// from provider settings/environment and before they enter the factory.
    /// Routing policy, tags, limits, model lists and probe policy are excluded:
    /// editing those must not reset the underlying resource's quota state.
    pub(super) fn for_provider(config: &ProviderConfig) -> Self {
        let mut value = serde_json::json!({
            "schema": 1,
            "provider": config.effective_provider_selector().to_ascii_lowercase(),
            "api_key": config.api_key,
            "base_url": config.base_url,
            "endpoint_access": config.endpoint_access,
            "api_version": config.api_version,
            "organization": config.organization,
            "project": config.project,
            "settings": config.settings,
        });
        value.sort_all_objects();
        Self(Sha256::digest(value.to_string().as_bytes()).into())
    }

    pub(super) fn for_model(&self, model: &str) -> Self {
        let mut digest = Sha256::new();
        digest.update(self.0);
        digest.update(model.as_bytes());
        Self(digest.finalize().into())
    }
}

#[cfg(feature = "gateway")]
use super::{DeploymentState, RoutingSnapshot, deployment::current_timestamp};
#[cfg(feature = "gateway")]
use std::collections::HashMap;
#[cfg(feature = "gateway")]
use std::sync::atomic::Ordering;

/// Keep removed resource states only while they still hold requests, current
/// minute usage or cooldown. Disabling/re-enabling a provider, or rotating A ->
/// B -> A while A has work in flight, cannot grant it a fresh quota bucket.
#[cfg(feature = "gateway")]
#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeStateRegistry {
    states: HashMap<(String, GatewayRuntimeIdentity), DeploymentState>,
}

#[cfg(feature = "gateway")]
impl RuntimeStateRegistry {
    pub(super) fn prune_retired(&mut self) {
        let now = current_timestamp();
        self.states.retain(|_, state| {
            let minute = state.minute_counters(now);
            state.active_requests.load(Ordering::Acquire) > 0
                || minute.rpm > 0
                || minute.tpm > 0
                || minute.failures > 0
                || state.cooldown_until.load(Ordering::Acquire) > now
        });
    }

    pub(super) fn remember(&mut self, snapshot: &RoutingSnapshot) {
        for (id, deployment) in &snapshot.deployments {
            if let Some(identity) = &deployment.state.runtime_identity {
                self.states
                    .insert((id.clone(), identity.clone()), deployment.state.clone());
            }
        }
    }

    pub(super) fn find(&self, id: &str, state: &DeploymentState) -> Option<&DeploymentState> {
        self.states
            .get(&(id.to_owned(), state.runtime_identity.clone()?))
    }
}
