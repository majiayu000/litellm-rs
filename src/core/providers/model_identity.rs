//! Config-backed, exact model identity validation.

use crate::core::pricing_service::PricingService;
use crate::core::pricing_service::PricingSnapshot;
use crate::core::providers::registry::model_catalog_authority::{
    CatalogAuthority, CatalogResolution,
};
use crate::core::types::model_id::ModelIdRef;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MODEL_IDENTITY_MAPPINGS_KEY: &str = "model_identity_mappings";

/// Explicit semantic identities for one configured wire/deployment model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelIdentityMapping {
    #[serde(default)]
    pub capability_catalog_model: Option<String>,
    #[serde(default)]
    pub pricing_model: Option<String>,
}

impl ModelIdentityMapping {
    pub fn new(capability_catalog_model: Option<String>, pricing_model: Option<String>) -> Self {
        Self {
            capability_catalog_model,
            pricing_model,
        }
    }
}

/// Exact provider-scoped price address pinned during startup validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactPricingIdentity {
    provider: String,
    model: String,
}

impl ExactPricingIdentity {
    pub(crate) fn provider(&self) -> &str {
        &self.provider
    }

    pub(crate) fn model(&self) -> &str {
        &self.model
    }
}

/// Owned immutable identity attached to one provider clone in a deployment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentModelIdentity {
    wire_model: String,
    capability_catalog_model: Option<String>,
    pricing: Option<ExactPricingIdentity>,
}

/// Immutable provider-private binding created from one startup pricing authority.
#[derive(Clone, Debug)]
pub(crate) struct DeploymentProviderBinding {
    identity: DeploymentModelIdentity,
    pricing: Arc<PricingService>,
}

impl DeploymentProviderBinding {
    pub(crate) fn new(identity: DeploymentModelIdentity, pricing: Arc<PricingService>) -> Self {
        Self { identity, pricing }
    }

    pub(crate) fn identity(&self) -> &DeploymentModelIdentity {
        &self.identity
    }

    pub(crate) fn pricing(&self) -> &Arc<PricingService> {
        &self.pricing
    }
}

impl DeploymentModelIdentity {
    pub(crate) fn new(
        wire_model: impl Into<String>,
        capability_catalog_model: Option<String>,
        pricing: Option<ExactPricingIdentity>,
    ) -> Self {
        Self {
            wire_model: wire_model.into(),
            capability_catalog_model,
            pricing,
        }
    }

    pub fn wire_model(&self) -> &str {
        &self.wire_model
    }

    pub fn capability_catalog_model(&self) -> Option<&str> {
        self.capability_catalog_model.as_deref()
    }

    pub fn pricing_provider(&self) -> Option<&str> {
        self.pricing.as_ref().map(ExactPricingIdentity::provider)
    }

    pub fn pricing_model(&self) -> Option<&str> {
        self.pricing.as_ref().map(ExactPricingIdentity::model)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelIdentityValidationError {
    #[error("provider '{provider}' deployment '{deployment}' has unsupported identity provider")]
    UnsupportedProvider {
        provider: String,
        deployment: String,
    },
    #[error(
        "provider '{provider}' deployment '{deployment}' field '{field}' value '{value}' is invalid: {reason}"
    )]
    InvalidField {
        provider: String,
        deployment: String,
        field: &'static str,
        value: String,
        reason: &'static str,
    },
    #[error(
        "provider '{provider}' deployment '{deployment}' requires settings.{MODEL_IDENTITY_MAPPINGS_KEY} with an exact callable identity"
    )]
    UnmappedDeployment {
        provider: String,
        deployment: String,
    },
}

/// Validate one deployment using the maintained catalog and the injected
/// runtime pricing snapshot. Explicit mapping wins over legacy and raw catalog.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_deployment_identity(
    provider_name: &str,
    provider: &str,
    wire_model: &str,
    explicit: Option<&ModelIdentityMapping>,
    legacy_openai_target: Option<&str>,
    catalog: &CatalogAuthority,
    pricing: &PricingSnapshot,
) -> Result<DeploymentModelIdentity, ModelIdentityValidationError> {
    let canonical_provider = canonical_identity_provider(provider).ok_or_else(|| {
        ModelIdentityValidationError::UnsupportedProvider {
            provider: provider_name.to_string(),
            deployment: wire_model.to_string(),
        }
    })?;
    if wire_model.trim().is_empty() {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "wire_model",
            wire_model,
            "empty deployment model",
        ));
    }

    if let Some(mapping) = explicit {
        let capability = mapping
            .capability_catalog_model
            .as_deref()
            .map(|target| {
                validate_capability_target(
                    provider_name,
                    canonical_provider,
                    wire_model,
                    target,
                    catalog,
                )
            })
            .transpose()?;
        if capability.is_none() && mapping.pricing_model.is_none() {
            return Err(invalid_field(
                provider_name,
                wire_model,
                MODEL_IDENTITY_MAPPINGS_KEY,
                wire_model,
                "mapping has no callable capability identity",
            ));
        }
        let pricing_identity = mapping
            .pricing_model
            .as_deref()
            .map(|target| {
                validate_pricing_target(
                    provider_name,
                    canonical_provider,
                    wire_model,
                    target,
                    pricing,
                )
            })
            .transpose()?;
        return Ok(DeploymentModelIdentity::new(
            wire_model,
            capability,
            pricing_identity,
        ));
    }

    if canonical_provider == "openai"
        && let Some(target) = legacy_openai_target.filter(|target| *target != wire_model)
    {
        let capability = validate_capability_target(
            provider_name,
            canonical_provider,
            wire_model,
            target,
            catalog,
        )?;
        let pricing_identity = validate_pricing_target(
            provider_name,
            canonical_provider,
            wire_model,
            target,
            pricing,
        )?;
        return Ok(DeploymentModelIdentity::new(
            wire_model,
            Some(capability),
            Some(pricing_identity),
        ));
    }

    match catalog.resolve_model(canonical_provider, wire_model) {
        CatalogResolution::Callable(model) => {
            let pricing_identity = pricing
                .get_model_info_for_provider(canonical_provider, model.pricing_key())
                .map(|(resolved, _)| ExactPricingIdentity {
                    provider: canonical_provider.to_string(),
                    model: resolved,
                });
            Ok(DeploymentModelIdentity::new(
                wire_model,
                Some(model.catalog_model_id().to_string()),
                pricing_identity,
            ))
        }
        CatalogResolution::PricingOnly => Err(invalid_field(
            provider_name,
            wire_model,
            "model",
            wire_model,
            "pricing-only identity is not callable",
        )),
        CatalogResolution::Unreviewed => Err(invalid_field(
            provider_name,
            wire_model,
            "model",
            wire_model,
            "unreviewed catalog identity is not callable",
        )),
        CatalogResolution::Unknown => Err(ModelIdentityValidationError::UnmappedDeployment {
            provider: provider_name.to_string(),
            deployment: wire_model.to_string(),
        }),
    }
}

fn validate_capability_target(
    provider_name: &str,
    selected_provider: &str,
    wire_model: &str,
    target: &str,
    catalog: &CatalogAuthority,
) -> Result<String, ModelIdentityValidationError> {
    if target.trim().is_empty() {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "capability_catalog_model",
            target,
            "empty target",
        ));
    }
    reject_wrong_or_double_qualifier(provider_name, selected_provider, wire_model, target)?;
    let capability_provider = match selected_provider {
        "azure" => "openai",
        other => other,
    };
    match catalog.resolve_model(capability_provider, target) {
        CatalogResolution::Callable(model) => Ok(model.catalog_model_id().to_string()),
        CatalogResolution::PricingOnly => Err(invalid_field(
            provider_name,
            wire_model,
            "capability_catalog_model",
            target,
            "pricing-only target",
        )),
        CatalogResolution::Unreviewed => Err(invalid_field(
            provider_name,
            wire_model,
            "capability_catalog_model",
            target,
            "unreviewed target",
        )),
        CatalogResolution::Unknown => Err(invalid_field(
            provider_name,
            wire_model,
            "capability_catalog_model",
            target,
            "unknown target or wrong provider",
        )),
    }
}

fn validate_pricing_target(
    provider_name: &str,
    selected_provider: &str,
    wire_model: &str,
    target: &str,
    pricing: &PricingSnapshot,
) -> Result<ExactPricingIdentity, ModelIdentityValidationError> {
    if target.trim().is_empty() {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "pricing_model",
            target,
            "empty target",
        ));
    }
    reject_wrong_or_double_qualifier(provider_name, selected_provider, wire_model, target)?;
    let (model, _) = pricing
        .get_model_info_for_provider(selected_provider, target)
        .ok_or_else(|| {
            invalid_field(
                provider_name,
                wire_model,
                "pricing_model",
                target,
                "target is absent from the injected provider-scoped pricing snapshot",
            )
        })?;
    Ok(ExactPricingIdentity {
        provider: selected_provider.to_string(),
        model,
    })
}

fn reject_wrong_or_double_qualifier(
    provider_name: &str,
    selected_provider: &str,
    wire_model: &str,
    target: &str,
) -> Result<(), ModelIdentityValidationError> {
    let parsed = ModelIdRef::parse(target);
    let Some(qualifier) = parsed.provider() else {
        return Ok(());
    };
    let Some(qualified_provider) = canonical_identity_provider(qualifier) else {
        return Ok(());
    };
    if qualified_provider != selected_provider {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "identity_target",
            target,
            "wrong provider qualifier",
        ));
    }
    if ModelIdRef::parse(parsed.model())
        .provider()
        .and_then(canonical_identity_provider)
        .is_some()
    {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "identity_target",
            target,
            "double provider qualification",
        ));
    }
    Ok(())
}

pub(crate) fn canonical_identity_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("openai"),
        "azure" | "azure-openai" | "azure_openai" => Some("azure"),
        "azure_ai" | "azure-ai" | "azureai" => Some("azure_ai"),
        _ => None,
    }
}

fn invalid_field(
    provider: &str,
    deployment: &str,
    field: &'static str,
    value: &str,
    reason: &'static str,
) -> ModelIdentityValidationError {
    ModelIdentityValidationError::InvalidField {
        provider: provider.to_string(),
        deployment: deployment.to_string(),
        field,
        value: value.to_string(),
        reason,
    }
}

pub(crate) fn calculate_managed_provider_cost(
    provider: &crate::core::providers::Provider,
    wire_model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> Option<Result<f64, crate::core::providers::ProviderError>> {
    let pricing_provider = canonical_identity_provider(match provider.provider_type() {
        crate::core::providers::provider_type::ProviderType::OpenAI => "openai",
        #[cfg(feature = "providers-extra")]
        crate::core::providers::provider_type::ProviderType::Azure => "azure",
        #[cfg(feature = "providers-extra")]
        crate::core::providers::provider_type::ProviderType::AzureAI => "azure_ai",
        _ => return None,
    })?;
    let usage = crate::core::pricing_service::PricingUsage::new(input_tokens, output_tokens);
    let result = if let Some(identity) = provider.deployment_model_identity() {
        match (identity.pricing_provider(), identity.pricing_model()) {
            (Some(pricing_provider), Some(pricing_model)) => provider
                .runtime_pricing()
                .ok_or_else(|| {
                    crate::core::providers::ProviderError::configuration(
                        "pricing",
                        "runtime pricing authority is missing",
                    )
                })
                .and_then(|pricing| {
                    pricing
                        .calculate_loaded_usage_cost_for_provider(
                            pricing_provider,
                            pricing_model,
                            &usage,
                        )
                        .map(|cost| cost.total_cost)
                        .map_err(|error| {
                            crate::core::providers::ProviderError::configuration(
                                "pricing",
                                error.to_string(),
                            )
                        })
                }),
            _ => Err(crate::core::providers::ProviderError::configuration(
                "pricing",
                format!(
                    "deployment '{}' is explicitly unpriced",
                    identity.wire_model()
                ),
            )),
        }
    } else {
        crate::core::pricing_service::PricingService::shared_embedded_default()
            .and_then(|pricing| {
                pricing
                    .calculate_loaded_usage_cost_for_provider(pricing_provider, wire_model, &usage)
                    .map(|cost| cost.total_cost)
            })
            .map_err(|error| {
                crate::core::providers::ProviderError::configuration("pricing", error.to_string())
            })
    };
    Some(result)
}
