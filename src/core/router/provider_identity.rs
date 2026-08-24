//! Configured provider identity retained alongside immutable routing snapshots.

use super::deployment::{Deployment, LegacySelectorMetadata};
use super::unified::{Router, RoutingSnapshot};
use std::sync::Arc;

impl RoutingSnapshot {
    pub(super) fn retained_provider_name(&self, id: &str, fallback: &str) -> String {
        self.provider_names
            .get(id)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    pub(super) fn restore_provider_names(
        &mut self,
        previous: std::collections::HashMap<String, String>,
    ) {
        self.provider_names.extend(
            previous
                .into_iter()
                .filter(|(id, _)| self.deployments.contains_key(id)),
        );
    }

    pub(super) fn insert_gateway_deployment(
        &mut self,
        deployment: Deployment,
        legacy_metadata: Option<LegacySelectorMetadata>,
        provider_name: &str,
    ) {
        let deployment_id = deployment.id.clone();
        match legacy_metadata {
            Some(metadata) => self.insert_deployment_with_legacy_metadata(deployment, metadata),
            None => self.insert_deployment(deployment),
        }
        self.provider_names
            .insert(deployment_id, provider_name.to_string());
    }
}

impl Router {
    /// Return one consistent snapshot of deployments created for a configured
    /// provider name. Ad-hoc deployments retain their provider's canonical name.
    pub(crate) fn deployments_for_provider(&self, provider_name: &str) -> Vec<Arc<Deployment>> {
        let snapshot = self.load_routing_snapshot();
        snapshot
            .provider_names
            .iter()
            .filter(|(_, configured_name)| configured_name.as_str() == provider_name)
            .filter_map(|(id, _)| snapshot.deployments.get(id).cloned())
            .collect()
    }
}
