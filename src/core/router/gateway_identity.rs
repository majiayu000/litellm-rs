//! Startup-only validation and binding for deployment model identities.

use super::config::RouterConfig;
use super::error::RouterError;
use super::unified::Router;
use crate::config::models::provider::ProviderConfig;
use crate::core::pricing_service::{PricingService, PricingSnapshot};
use crate::core::providers::Provider;
use crate::core::providers::model_identity::{
    MODEL_IDENTITY_MAPPINGS_KEY, ModelIdentityMapping, canonical_identity_provider,
    validate_deployment_identity,
};
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry::model_catalog_authority::{
    CatalogAuthority, CatalogResolution,
};
use std::collections::HashMap;
use std::sync::Arc;

/// One immutable authority generation used while a routing snapshot is staged.
pub(super) struct GatewayIdentityAuthority {
    pricing: Arc<PricingService>,
    snapshot: PricingSnapshot,
    catalog: CatalogAuthority,
}

impl GatewayIdentityAuthority {
    pub(super) fn new(pricing: Arc<PricingService>) -> Result<Self, RouterError> {
        let snapshot = pricing.snapshot();
        let catalog = CatalogAuthority::from_embedded()
            .map_err(|error| RouterError::InvalidConfiguration(error.to_string()))?;
        Ok(Self {
            pricing,
            snapshot,
            catalog,
        })
    }

    pub(super) fn bind(
        &self,
        provider_name: &str,
        provider: &mut Provider,
        wire_model: &str,
        mapping: Option<&ModelIdentityMapping>,
    ) -> Result<(), RouterError> {
        let Some(identity_provider) = identity_provider(provider) else {
            return Ok(());
        };
        debug_assert!(canonical_identity_provider(identity_provider).is_some());
        let legacy_target = provider
            .legacy_openai_model_target(wire_model)
            .map(str::to_owned);
        let identity = validate_deployment_identity(
            provider_name,
            identity_provider,
            wire_model,
            mapping,
            legacy_target.as_deref(),
            &self.catalog,
            &self.snapshot,
        )
        .map_err(|error| RouterError::InvalidConfiguration(error.to_string()))?;
        provider
            .bind_deployment_model_identity(identity, Arc::clone(&self.pricing))
            .map_err(RouterError::InvalidConfiguration)
    }
}

pub(super) fn default_models(
    provider: &Provider,
    authority: Option<&GatewayIdentityAuthority>,
) -> Vec<String> {
    let identity_provider = identity_provider(provider);
    provider
        .list_models()
        .iter()
        .filter(|model| match (authority, identity_provider) {
            (Some(authority), Some(identity_provider)) => matches!(
                authority
                    .catalog
                    .resolve_model(identity_provider, &model.id),
                CatalogResolution::Callable(_)
            ),
            _ => true,
        })
        .map(|model| model.id.clone())
        .collect()
}

fn identity_provider(provider: &Provider) -> Option<&'static str> {
    match provider.provider_type() {
        ProviderType::OpenAI => Some("openai"),
        #[cfg(feature = "providers-extra")]
        ProviderType::Azure => Some("azure"),
        #[cfg(feature = "providers-extra")]
        ProviderType::AzureAI => Some("azure_ai"),
        _ => None,
    }
}

impl Router {
    /// Create a router using one runtime pricing authority for identity validation.
    pub async fn from_gateway_config_with_pricing(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
        pricing: Arc<PricingService>,
    ) -> Result<Self, RouterError> {
        Self::from_gateway_config_with_aliases_and_pricing(
            providers,
            router_config,
            &HashMap::new(),
            pricing,
        )
        .await
    }

    /// Create a router with aliases and a shared runtime pricing authority.
    pub async fn from_gateway_config_with_aliases_and_pricing(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
        model_aliases: &HashMap<String, String>,
        pricing: Arc<PricingService>,
    ) -> Result<Self, RouterError> {
        let authority = GatewayIdentityAuthority::new(pricing)?;
        Self::from_gateway_config_with_identity(
            providers,
            router_config,
            model_aliases,
            Some(&authority),
        )
        .await
    }
}

pub(super) fn take_identity_mappings(
    config: &mut ProviderConfig,
    authority: Option<&GatewayIdentityAuthority>,
) -> Result<HashMap<String, ModelIdentityMapping>, RouterError> {
    let Some(value) = config.settings.remove(MODEL_IDENTITY_MAPPINGS_KEY) else {
        return Ok(HashMap::new());
    };
    if authority.is_none() {
        return Err(RouterError::InvalidConfiguration(format!(
            "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} requires the pricing-aware router constructor",
            config.name
        )));
    }
    let mappings: HashMap<String, ModelIdentityMapping> =
        serde_json::from_value(value).map_err(|error| {
            RouterError::InvalidConfiguration(format!(
                "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} is invalid: {error}",
                config.name
            ))
        })?;
    for deployment in mappings.keys() {
        if deployment.trim().is_empty() || !config.models.iter().any(|model| model == deployment) {
            return Err(RouterError::InvalidConfiguration(format!(
                "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} key '{}' does not name a configured model",
                config.name, deployment
            )));
        }
    }
    Ok(mappings)
}
