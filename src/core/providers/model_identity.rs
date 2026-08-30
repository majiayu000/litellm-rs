use crate::core::pricing_service::{PricingService, PricingSnapshot};
use crate::core::providers::registry::model_catalog_authority::{
    CatalogAuthority, CatalogResolution,
};
use crate::core::types::model::ProviderCapability;
use crate::core::types::model_id::ModelIdRef;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MODEL_IDENTITY_MAPPINGS_KEY: &str = "model_identity_mappings";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactPricingIdentity {
    provider: String,
    model: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactCapabilityIdentity {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentModelIdentity {
    wire_model: String,
    capability: Option<ExactCapabilityIdentity>,
    pricing: Option<ExactPricingIdentity>,
    pricing_scope: PricingIdentityScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PricingIdentityScope {
    AllSurfaces,
    ChatOnly,
}

pub(crate) enum DeploymentPricingIdentity<'a> {
    Priced { provider: &'a str, model: &'a str },
    Unpriced,
    NotApplicable,
}

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
        capability: Option<ExactCapabilityIdentity>,
        pricing: Option<ExactPricingIdentity>,
    ) -> Self {
        Self {
            wire_model: wire_model.into(),
            capability,
            pricing,
            pricing_scope: PricingIdentityScope::AllSurfaces,
        }
    }

    fn new_legacy_chat_mapping(
        wire_model: impl Into<String>,
        capability: ExactCapabilityIdentity,
        pricing: ExactPricingIdentity,
    ) -> Self {
        Self {
            wire_model: wire_model.into(),
            capability: Some(capability),
            pricing: Some(pricing),
            pricing_scope: PricingIdentityScope::ChatOnly,
        }
    }

    pub fn wire_model(&self) -> &str {
        &self.wire_model
    }

    pub fn capability_catalog_provider(&self) -> Option<&str> {
        self.capability
            .as_ref()
            .map(|identity| identity.provider.as_str())
    }

    pub fn capability_catalog_model(&self) -> Option<&str> {
        self.capability
            .as_ref()
            .map(|identity| identity.model.as_str())
    }

    pub fn pricing_provider(&self) -> Option<&str> {
        self.pricing.as_ref().map(ExactPricingIdentity::provider)
    }

    pub fn pricing_model(&self) -> Option<&str> {
        self.pricing.as_ref().map(ExactPricingIdentity::model)
    }

    pub(crate) fn pricing_identity_for_surface(
        &self,
        surface: &ProviderCapability,
    ) -> DeploymentPricingIdentity<'_> {
        if self.pricing_scope == PricingIdentityScope::ChatOnly
            && !matches!(
                surface,
                ProviderCapability::ChatCompletion | ProviderCapability::ChatCompletionStream
            )
        {
            return DeploymentPricingIdentity::NotApplicable;
        }
        match &self.pricing {
            Some(pricing) => DeploymentPricingIdentity::Priced {
                provider: pricing.provider(),
                model: pricing.model(),
            },
            None => DeploymentPricingIdentity::Unpriced,
        }
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
}

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
        return Ok(DeploymentModelIdentity::new_legacy_chat_mapping(
            wire_model,
            capability,
            pricing_identity,
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
                Some(ExactCapabilityIdentity {
                    provider: canonical_provider.to_string(),
                    model: model.catalog_model_id().to_string(),
                }),
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
        CatalogResolution::Unknown => Ok(DeploymentModelIdentity::new(wire_model, None, None)),
    }
}

fn validate_capability_target(
    provider_name: &str,
    selected_provider: &str,
    wire_model: &str,
    target: &str,
    catalog: &CatalogAuthority,
) -> Result<ExactCapabilityIdentity, ModelIdentityValidationError> {
    if target.trim().is_empty() {
        return Err(invalid_field(
            provider_name,
            wire_model,
            "capability_catalog_model",
            target,
            "empty target",
        ));
    }
    let qualifier = single_identity_qualifier(provider_name, wire_model, target)?;
    let capability_provider = match (selected_provider, qualifier) {
        ("openai", Some("openai")) | ("azure", Some("openai")) => "openai",
        ("azure_ai", Some(provider @ ("openai" | "azure_ai"))) => provider,
        ("openai_compatible", Some("xai")) | ("xai", Some("xai")) => "xai",
        (_, Some(_)) => {
            return Err(invalid_field(
                provider_name,
                wire_model,
                "capability_catalog_model",
                target,
                "wrong provider qualifier",
            ));
        }
        ("azure", None) => "openai",
        ("openai_compatible", None) => {
            return Err(invalid_field(
                provider_name,
                wire_model,
                "capability_catalog_model",
                target,
                "cross-provider target requires an exact provider qualifier",
            ));
        }
        (provider, None) => provider,
    };
    match catalog.resolve_model(capability_provider, target) {
        CatalogResolution::Callable(model) => Ok(ExactCapabilityIdentity {
            provider: capability_provider.to_string(),
            model: model.catalog_model_id().to_string(),
        }),
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
    let qualifier = single_identity_qualifier(provider_name, wire_model, target)?;
    let pricing_provider = match (selected_provider, qualifier) {
        ("openai_compatible", Some("xai")) => "xai",
        ("openai_compatible", _) => {
            return Err(invalid_field(
                provider_name,
                wire_model,
                "pricing_model",
                target,
                "cross-provider target requires an exact provider qualifier",
            ));
        }
        (provider, Some(qualifier)) if provider == qualifier => provider,
        (_, Some(_)) => {
            return Err(invalid_field(
                provider_name,
                wire_model,
                "pricing_model",
                target,
                "wrong or missing provider qualifier",
            ));
        }
        (provider, None) => provider,
    };
    let (model, _) = pricing
        .get_model_info_for_provider(pricing_provider, target)
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
        provider: pricing_provider.to_string(),
        model,
    })
}

fn single_identity_qualifier(
    provider_name: &str,
    wire_model: &str,
    target: &str,
) -> Result<Option<&'static str>, ModelIdentityValidationError> {
    let parsed = ModelIdRef::parse(target);
    let Some(qualifier) = parsed.provider() else {
        return Ok(None);
    };
    let Some(qualified_provider) = canonical_identity_provider(qualifier) else {
        return Ok(None);
    };
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
    Ok(Some(qualified_provider))
}

pub(crate) fn canonical_identity_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" => Some("openai"),
        "azure" | "azure-openai" | "azure_openai" => Some("azure"),
        "azure_ai" | "azure-ai" | "azureai" => Some("azure_ai"),
        "xai" => Some("xai"),
        "openai_compatible" => Some("openai_compatible"),
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
    let usage = crate::core::pricing_service::PricingUsage::new(input_tokens, output_tokens);
    if let Some(identity) = provider.deployment_model_identity() {
        let result = match (identity.pricing_provider(), identity.pricing_model()) {
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
        };
        return Some(result);
    }
    let pricing_provider = match provider {
        crate::core::providers::Provider::OpenAI(_) => "openai",
        #[cfg(feature = "providers-extra")]
        crate::core::providers::Provider::Azure(_) => "azure",
        #[cfg(feature = "providers-extra")]
        crate::core::providers::Provider::AzureAI(_) => "azure_ai",
        crate::core::providers::Provider::OpenAILike(inner)
            if inner.config().provider_name == "xai"
                && super::openai_like::models::is_xai_current_model(wire_model) =>
        {
            "xai"
        }
        _ => return None,
    };
    Some(
        crate::core::pricing_service::PricingService::shared_embedded_default()
            .and_then(|pricing| {
                pricing
                    .calculate_loaded_usage_cost_for_provider(
                        pricing_provider,
                        super::openai_like::models::xai_current_pricing_model(
                            pricing_provider,
                            wire_model,
                        ),
                        &usage,
                    )
                    .map(|cost| cost.total_cost)
            })
            .map_err(|error| {
                crate::core::providers::ProviderError::configuration("pricing", error.to_string())
            }),
    )
}

#[cfg(test)]
mod tests {
    use super::{DeploymentModelIdentity, ModelIdentityMapping, validate_deployment_identity};
    use crate::core::pricing_service::{LiteLLMModelInfo, PricingService};
    use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;
    use std::collections::HashMap;

    fn pricing_info(provider: &str) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(1024),
            input_cost_per_token: Some(0.01),
            output_cost_per_token: Some(0.02),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: None,
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        }
    }

    fn authorities() -> (CatalogAuthority, PricingService) {
        let catalog = CatalogAuthority::from_embedded().expect("embedded catalog authority");
        let pricing = PricingService::new(None);
        (catalog, pricing)
    }

    #[test]
    fn mapping_serde_round_trip_preserves_explicit_unpriced() {
        let mapping = ModelIdentityMapping::new(Some("gpt-4".to_string()), None);
        let json = serde_json::to_value(&mapping).expect("serialize mapping");
        assert!(
            json.get("pricing_model")
                .is_some_and(serde_json::Value::is_null)
        );
        assert_eq!(
            serde_json::from_value::<ModelIdentityMapping>(json).expect("deserialize mapping"),
            mapping
        );
    }

    #[test]
    fn runtime_pricing_mapping_uses_injected_snapshot_and_preserves_wire_model() {
        let (catalog, pricing) = authorities();
        pricing.add_custom_model("runtime-only-price".to_string(), pricing_info("openai"));
        let snapshot = pricing.snapshot();
        let mapping = ModelIdentityMapping::new(
            Some("gpt-4".to_string()),
            Some("runtime-only-price".to_string()),
        );

        let identity = validate_deployment_identity(
            "edge-openai",
            "openai",
            "wire-deployment",
            Some(&mapping),
            None,
            &catalog,
            &snapshot,
        )
        .expect("runtime-only target should validate against injected snapshot");

        assert_eq!(identity.wire_model(), "wire-deployment");
        assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
        assert_eq!(identity.pricing_provider(), Some("openai"));
        assert_eq!(identity.pricing_model(), Some("runtime-only-price"));
    }

    #[test]
    fn explicit_unpriced_never_inherits_raw_wire_pricing() {
        let (catalog, pricing) = authorities();
        pricing.add_custom_model("gpt-4".to_string(), pricing_info("openai"));
        let mapping = ModelIdentityMapping::new(Some("gpt-4".to_string()), None);
        let identity = validate_deployment_identity(
            "edge-openai",
            "openai",
            "gpt-4",
            Some(&mapping),
            None,
            &catalog,
            &pricing.snapshot(),
        )
        .expect("explicit unpriced mapping is valid");
        assert_eq!(identity.pricing_model(), None);
    }

    #[test]
    fn explicit_mapping_precedes_legacy_and_raw_catalog() {
        let (catalog, pricing) = authorities();
        for model in ["gpt-4", "gpt-4o-mini"] {
            pricing.add_custom_model(model.to_string(), pricing_info("openai"));
        }
        let explicit = ModelIdentityMapping::new(
            Some("gpt-4o-mini".to_string()),
            Some("gpt-4o-mini".to_string()),
        );
        let identity = validate_deployment_identity(
            "edge-openai",
            "openai",
            "gpt-4",
            Some(&explicit),
            Some("gpt-4"),
            &catalog,
            &pricing.snapshot(),
        )
        .expect("explicit mapping should win");
        assert_eq!(identity.capability_catalog_model(), Some("gpt-4o-mini"));
        assert_eq!(identity.pricing_model(), Some("gpt-4o-mini"));
    }

    #[test]
    fn wrong_provider_unknown_and_pricing_only_capability_fail_closed() {
        let (catalog, pricing) = authorities();
        let snapshot = pricing.snapshot();
        for (provider, capability, expected) in [
            ("azure", "azure_ai/Phi-4", "provider"),
            ("openai", "fake-gpt-5-2099-01-01", "unknown"),
            ("openai", "openai/container", "pricing-only"),
        ] {
            let mapping = ModelIdentityMapping::new(Some(capability.to_string()), None);
            let error = validate_deployment_identity(
                "edge",
                provider,
                "wire",
                Some(&mapping),
                None,
                &catalog,
                &snapshot,
            )
            .expect_err("invalid capability target must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn exact_catalog_auto_resolution_needs_no_redundant_mapping() {
        let (catalog, pricing) = authorities();
        pricing.add_custom_model("gpt-4".to_string(), pricing_info("openai"));
        let identity: DeploymentModelIdentity = validate_deployment_identity(
            "edge-openai",
            "openai",
            "gpt-4",
            None,
            None,
            &catalog,
            &pricing.snapshot(),
        )
        .expect("exact callable catalog model should auto-resolve");
        assert_eq!(identity.wire_model(), "gpt-4");
        assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
    }

    #[test]
    fn azure_transports_preserve_wire_identity_and_resolve_underlying_capability_provider() {
        let (catalog, pricing) = authorities();
        for (transport, target, expected_provider, expected_model) in [
            ("azure", "openai/gpt-4", "openai", "gpt-4"),
            ("azure_ai", "openai/gpt-4", "openai", "gpt-4"),
            ("azure_ai", "azure_ai/Phi-4", "azure_ai", "Phi-4"),
        ] {
            let mapping = ModelIdentityMapping::new(Some(target.to_string()), None);
            let identity = validate_deployment_identity(
                "edge",
                transport,
                "wire-deployment",
                Some(&mapping),
                None,
                &catalog,
                &pricing.snapshot(),
            )
            .expect("valid underlying capability qualifier should resolve");

            assert_eq!(identity.wire_model(), "wire-deployment");
            assert_eq!(
                identity.capability_catalog_provider(),
                Some(expected_provider)
            );
            assert_eq!(identity.capability_catalog_model(), Some(expected_model));
        }
    }

    #[test]
    fn azure_capability_mapping_rejects_wrong_and_double_qualifiers() {
        let (catalog, pricing) = authorities();
        for target in ["azure/gpt-4", "azure_ai/Phi-4", "openai/openai/gpt-4"] {
            let mapping = ModelIdentityMapping::new(Some(target.to_string()), None);
            let error = validate_deployment_identity(
                "edge-azure",
                "azure",
                "wire-deployment",
                Some(&mapping),
                None,
                &catalog,
                &pricing.snapshot(),
            )
            .expect_err("wrong or double capability qualifier must fail closed");
            assert!(error.to_string().contains("provider"), "{error}");
        }
    }

    #[test]
    fn explicit_openai_compatible_mapping_can_target_qualified_xai_only() {
        let (catalog, pricing) = authorities();
        pricing.add_custom_model("xai/grok-4.6".to_string(), pricing_info("xai"));
        let validate = |mapping: &ModelIdentityMapping| {
            validate_deployment_identity(
                "vertex",
                "openai_compatible",
                "xai/grok-4.6",
                Some(mapping),
                None,
                &catalog,
                &pricing.snapshot(),
            )
        };
        let qualified = "xai/grok-4.6".to_string();
        let mapping = ModelIdentityMapping::new(Some(qualified.clone()), Some(qualified));
        validate(&mapping).expect("qualified xAI mapping should validate");
        let bare = ModelIdentityMapping::new(Some("grok-4.6".to_string()), None);
        assert!(validate(&bare).is_err());
    }
}
