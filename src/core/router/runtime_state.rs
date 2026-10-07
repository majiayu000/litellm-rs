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
use std::sync::Arc;
#[cfg(feature = "gateway")]
use std::sync::atomic::Ordering;

#[cfg(feature = "gateway")]
impl super::Deployment {
    /// Shared state follows the same resource identity as the local registry.
    pub(crate) fn shared_state_id(&self) -> String {
        match &self.state.runtime_identity {
            Some(identity) => format!("{}:{}", self.id, hex::encode(identity.0)),
            None => self.id.clone(),
        }
    }
}

/// Keep removed resource states while a routing/state handle can still use
/// them, or they retain current minute usage or cooldown. One shared entry per
/// resource prevents registry copies from counting as live request handles.
#[cfg(feature = "gateway")]
#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeStateRegistry {
    states: HashMap<(String, GatewayRuntimeIdentity), Arc<DeploymentState>>,
}

#[cfg(feature = "gateway")]
impl RuntimeStateRegistry {
    pub(super) fn prune_retired(&mut self) {
        let now = current_timestamp();
        self.states.retain(|_, state| {
            // Check ownership first: an idle old snapshot can start a request
            // after this check, or a finishing request can change usage before
            // dropping its final handle. Only unowned states are quiescent.
            if state.has_other_runtime_handles() {
                return true;
            }
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
                    .entry((id.clone(), identity.clone()))
                    // Matching snapshots inherit this entry's shared state.
                    // Replacing it would make old registry copies look like
                    // additional live state handles and prevent retirement.
                    .or_insert_with(|| Arc::new(deployment.state.clone()));
            }
        }
    }

    pub(super) fn find(&self, id: &str, state: &DeploymentState) -> Option<&DeploymentState> {
        self.states
            .get(&(id.to_owned(), state.runtime_identity.clone()?))
            .map(Arc::as_ref)
    }
}

#[cfg(all(test, feature = "gateway"))]
mod tests {
    use super::*;
    use crate::core::providers::{Provider, openai::OpenAIProvider};
    use crate::core::router::{Deployment, UnifiedRouter};

    async fn registry_fixture() -> (RuntimeStateRegistry, UnifiedRouter) {
        let provider = OpenAIProvider::with_api_key("sk-runtime-registry-test")
            .await
            .unwrap();
        let mut deployment = Deployment::new(
            "deployment".into(),
            Provider::OpenAI(provider),
            "gpt-4o-mini".into(),
            "chat".into(),
        );
        deployment.state.runtime_identity = Some(GatewayRuntimeIdentity([0; 32]));
        let router = UnifiedRouter::default();
        router.add_deployment(deployment);
        let mut registry = RuntimeStateRegistry::default();
        registry.remember(&router.load_routing_snapshot());
        (registry, router)
    }

    #[tokio::test]
    async fn registry_copies_do_not_prevent_retirement_after_the_last_pin() {
        let (mut previous, router) = registry_fixture().await;
        let pin = router.load_routing_snapshot();
        let mut current = previous.clone();
        current.remember(&pin);
        drop(router);

        current.prune_retired();
        assert_eq!(current.states.len(), 1, "an idle pin can still admit work");

        drop(pin);
        current.prune_retired();
        assert!(current.states.is_empty(), "registry copies are not pins");
        previous.prune_retired();
        assert!(previous.states.is_empty());
    }

    #[tokio::test]
    async fn usage_and_cooldown_outlive_pins_but_expired_resources_are_retired() {
        let (mut registry, router) = registry_fixture().await;
        let deployment = router.get_deployment("deployment").unwrap();
        deployment.record_success(17, 1);
        deployment.enter_cooldown(300);
        drop(deployment);
        drop(router);

        registry.prune_retired();
        assert_eq!(registry.states.len(), 1);
        let retained = registry.states.values().next().unwrap();
        assert_eq!(retained.minute_counters(current_timestamp()).tpm, 17);
        retained
            .minute_reset_at
            .store(current_timestamp() - 60, Ordering::Release);

        registry.prune_retired();
        assert_eq!(registry.states.len(), 1, "cooldown outlives the minute");
        let retained = registry.states.values().next().unwrap();
        assert_eq!(retained.minute_counters(current_timestamp()).tpm, 0);
        retained.cooldown_until.store(0, Ordering::Release);

        registry.prune_retired();
        assert!(registry.states.is_empty());
    }
}
