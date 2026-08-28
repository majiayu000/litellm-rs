//! Config-backed, exact model identity resolution.

use crate::core::providers::openai::models::OpenAIModelRegistry;
use crate::core::types::model_id::ModelIdRef;
use serde::Deserialize;
use std::collections::HashMap;

pub const MODEL_IDENTITY_MAPPINGS_KEY: &str = "model_identity_mappings";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelIdentityMapping {
    #[serde(default)]
    capability_catalog_model: Option<String>,
    #[serde(default)]
    pricing_model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelIdentityProvider {
    OpenAI,
    Azure,
    AzureAI,
}

/// Owned identity attached to one immutable router deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentModelIdentity {
    wire_model: String,
    capability_catalog_model: Option<String>,
    pricing_model: Option<String>,
}

impl DeploymentModelIdentity {
    pub fn new(
        wire_model: impl Into<String>,
        capability_catalog_model: Option<String>,
        pricing_model: Option<String>,
    ) -> Self {
        Self {
            wire_model: wire_model.into(),
            capability_catalog_model,
            pricing_model,
        }
    }

    pub fn wire_model(&self) -> &str {
        &self.wire_model
    }

    pub fn capability_catalog_model(&self) -> Option<&str> {
        self.capability_catalog_model.as_deref()
    }

    pub fn pricing_model(&self) -> Option<&str> {
        self.pricing_model.as_deref()
    }

    pub fn as_ref(&self) -> ConfiguredModelRef<'_> {
        ConfiguredModelRef {
            wire_model: &self.wire_model,
            capability_catalog_model: self.capability_catalog_model.as_deref(),
            pricing_model: self.pricing_model.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfiguredModelRef<'a> {
    pub wire_model: &'a str,
    pub capability_catalog_model: Option<&'a str>,
    pub pricing_model: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelIdentityError {
    WrongProvider,
    DoubleQualification,
    UnknownModel,
    InvalidConfiguration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogModelKind {
    Callable,
    PricingOnly,
}

pub trait ExactModelCatalog {
    fn model_kind(&self, model: &str) -> Option<CatalogModelKind>;
}

impl ExactModelCatalog for OpenAIModelRegistry {
    fn model_kind(&self, model: &str) -> Option<CatalogModelKind> {
        self.get_model_spec(model)?;
        Some(if self.is_callable_model(model) {
            CatalogModelKind::Callable
        } else {
            CatalogModelKind::PricingOnly
        })
    }
}

#[cfg(feature = "providers-extra")]
impl ExactModelCatalog for crate::core::providers::azure_ai::AzureAIModelRegistry {
    fn model_kind(&self, model: &str) -> Option<CatalogModelKind> {
        self.get_model(model).map(|_| CatalogModelKind::Callable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelIdentity<'a> {
    CatalogCallable {
        raw_model: &'a str,
        capability_catalog_model: &'a str,
        pricing_model: &'a str,
    },
    PricingOnly {
        raw_model: &'a str,
        pricing_model: &'a str,
    },
    ConfiguredDeployment {
        raw_model: &'a str,
        wire_model: &'a str,
        capability_catalog_model: Option<&'a str>,
        pricing_model: Option<&'a str>,
    },
    Invalid {
        raw_model: &'a str,
        reason: ModelIdentityError,
    },
}

impl<'a> ModelIdentity<'a> {
    pub fn capability_catalog_model(self) -> Option<&'a str> {
        match self {
            Self::CatalogCallable {
                capability_catalog_model,
                ..
            } => Some(capability_catalog_model),
            Self::ConfiguredDeployment {
                capability_catalog_model,
                ..
            } => capability_catalog_model,
            Self::PricingOnly { .. } | Self::Invalid { .. } => None,
        }
    }

    pub fn pricing_model(self) -> Option<&'a str> {
        match self {
            Self::CatalogCallable { pricing_model, .. }
            | Self::PricingOnly { pricing_model, .. } => Some(pricing_model),
            Self::ConfiguredDeployment { pricing_model, .. } => pricing_model,
            Self::Invalid { .. } => None,
        }
    }

    pub fn wire_model(self) -> Option<&'a str> {
        match self {
            Self::CatalogCallable { raw_model, .. } | Self::PricingOnly { raw_model, .. } => {
                Some(raw_model)
            }
            Self::ConfiguredDeployment { wire_model, .. } => Some(wire_model),
            Self::Invalid { .. } => None,
        }
    }

    pub fn invalid_reason(self) -> Option<ModelIdentityError> {
        match self {
            Self::Invalid { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

fn qualifier_provider(value: &str) -> Option<ModelIdentityProvider> {
    if value.eq_ignore_ascii_case("openai") {
        Some(ModelIdentityProvider::OpenAI)
    } else if value.eq_ignore_ascii_case("azure") || value.eq_ignore_ascii_case("azure-openai") {
        Some(ModelIdentityProvider::Azure)
    } else if value.eq_ignore_ascii_case("azure_ai")
        || value.eq_ignore_ascii_case("azure-ai")
        || value.eq_ignore_ascii_case("azureai")
    {
        Some(ModelIdentityProvider::AzureAI)
    } else {
        None
    }
}

fn known_provider_qualifier(value: &str) -> bool {
    qualifier_provider(value).is_some()
        || crate::core::providers::registry::entry_for_name(value).is_some()
}

fn qualifier_matches(selected: ModelIdentityProvider, qualifier: ModelIdentityProvider) -> bool {
    selected == qualifier
        || matches!(
            (selected, qualifier),
            (ModelIdentityProvider::Azure, ModelIdentityProvider::OpenAI)
                | (
                    ModelIdentityProvider::AzureAI,
                    ModelIdentityProvider::OpenAI
                )
        )
}

fn exact_catalog<'a>(
    catalog: &dyn ExactModelCatalog,
    raw: &'a str,
    key: &'a str,
) -> Option<ModelIdentity<'a>> {
    Some(match catalog.model_kind(key)? {
        CatalogModelKind::Callable => ModelIdentity::CatalogCallable {
            raw_model: raw,
            capability_catalog_model: key,
            pricing_model: key,
        },
        CatalogModelKind::PricingOnly => ModelIdentity::PricingOnly {
            raw_model: raw,
            pricing_model: key,
        },
    })
}

/// Resolve using exact catalog keys and an optional selected deployment record.
pub fn resolve_model_identity<'a>(
    selected: ModelIdentityProvider,
    raw_model: &'a str,
    configured: Option<ConfiguredModelRef<'a>>,
    catalog: &dyn ExactModelCatalog,
) -> ModelIdentity<'a> {
    // Full-key lookup precedes parsing because legitimate pricing keys contain `/`.
    if let Some(exact) = exact_catalog(catalog, raw_model, raw_model) {
        return exact;
    }
    let parsed = ModelIdRef::parse(raw_model);
    let qualified_catalog_model = if let Some(provider_text) = parsed.provider() {
        let Some(qualifier) = qualifier_provider(provider_text) else {
            let reason = if known_provider_qualifier(provider_text) {
                ModelIdentityError::WrongProvider
            } else {
                ModelIdentityError::UnknownModel
            };
            return ModelIdentity::Invalid { raw_model, reason };
        };
        if !qualifier_matches(selected, qualifier) {
            return ModelIdentity::Invalid {
                raw_model,
                reason: ModelIdentityError::WrongProvider,
            };
        }
        if ModelIdRef::parse(parsed.model())
            .provider()
            .is_some_and(known_provider_qualifier)
        {
            return ModelIdentity::Invalid {
                raw_model,
                reason: ModelIdentityError::DoubleQualification,
            };
        }
        Some(parsed.model())
    } else {
        None
    };

    if let Some(configured) = configured {
        let bad_capability = configured
            .capability_catalog_model
            .is_some_and(|model| catalog.model_kind(model) != Some(CatalogModelKind::Callable));
        let bad_pricing = configured
            .pricing_model
            .is_some_and(|model| catalog.model_kind(model).is_none());
        if bad_capability || bad_pricing {
            return ModelIdentity::Invalid {
                raw_model,
                reason: ModelIdentityError::InvalidConfiguration,
            };
        }
        return ModelIdentity::ConfiguredDeployment {
            raw_model,
            wire_model: configured.wire_model,
            capability_catalog_model: configured.capability_catalog_model,
            pricing_model: configured.pricing_model,
        };
    }

    let Some(catalog_model) = qualified_catalog_model else {
        return ModelIdentity::Invalid {
            raw_model,
            reason: ModelIdentityError::UnknownModel,
        };
    };
    exact_catalog(catalog, raw_model, catalog_model).unwrap_or(ModelIdentity::Invalid {
        raw_model,
        reason: ModelIdentityError::UnknownModel,
    })
}

/// Parse and validate gateway mappings before a routing snapshot is published.
pub fn take_validated_identity_mappings(
    provider_name: &str,
    provider: &crate::core::providers::Provider,
    configured_models: &[String],
    settings: &mut HashMap<String, serde_json::Value>,
) -> Result<HashMap<String, DeploymentModelIdentity>, String> {
    let Some(value) = settings.remove(MODEL_IDENTITY_MAPPINGS_KEY) else {
        return Ok(HashMap::new());
    };
    if !matches!(
        provider.provider_type(),
        crate::core::providers::ProviderType::OpenAI
            | crate::core::providers::ProviderType::Azure
            | crate::core::providers::ProviderType::AzureAI
    ) {
        return Err(format!(
            "provider '{provider_name}' does not support settings.{MODEL_IDENTITY_MAPPINGS_KEY}"
        ));
    }
    let mappings: HashMap<String, ModelIdentityMapping> = serde_json::from_value(value)
        .map_err(|error| {
            format!(
                "provider '{provider_name}': settings.{MODEL_IDENTITY_MAPPINGS_KEY} is invalid: {error}"
            )
        })?;
    let mut validated = HashMap::with_capacity(mappings.len());
    for (wire_model, mapping) in mappings {
        if !configured_models.iter().any(|model| model == &wire_model) {
            return Err(format!(
                "provider '{provider_name}': identity mapping key '{wire_model}' is not present in provider models"
            ));
        }
        if wire_model.trim().is_empty()
            || (mapping.capability_catalog_model.is_none() && mapping.pricing_model.is_none())
            || mapping
                .capability_catalog_model
                .as_deref()
                .is_some_and(|model| model.trim().is_empty())
            || mapping
                .pricing_model
                .as_deref()
                .is_some_and(|model| model.trim().is_empty())
        {
            return Err(format!(
                "provider '{provider_name}': identity mapping '{wire_model}' must contain at least one non-empty semantic identity"
            ));
        }
        let identity = DeploymentModelIdentity::new(
            &wire_model,
            mapping.capability_catalog_model,
            mapping.pricing_model,
        );
        let mut bound = provider.clone();
        bound.bind_deployment_identity(identity.clone());
        if let ModelIdentity::Invalid { reason, .. } = bound.resolve_model_identity(&wire_model) {
            return Err(format!(
                "provider '{provider_name}': invalid identity mapping for '{wire_model}': {reason:?}"
            ));
        }
        validated.insert(wire_model, identity);
    }
    Ok(validated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::openai::models::get_openai_registry;

    struct NativeSlashCatalog;

    impl ExactModelCatalog for NativeSlashCatalog {
        fn model_kind(&self, model: &str) -> Option<CatalogModelKind> {
            (model == "BAAI/bge-m3").then_some(CatalogModelKind::Callable)
        }
    }

    #[test]
    fn exact_qualified_pricing_only_and_native_slash_are_distinct() {
        let registry = get_openai_registry();
        for (raw, expected) in [
            ("gpt-4", "gpt-4"),
            ("openai/gpt-4", "gpt-4"),
            ("OPENAI/gpt-4", "gpt-4"),
            ("gpt-5.5-2026-04-23", "gpt-5.5-2026-04-23"),
        ] {
            assert_eq!(
                resolve_model_identity(ModelIdentityProvider::OpenAI, raw, None, registry)
                    .capability_catalog_model(),
                Some(expected)
            );
        }
        assert!(matches!(
            resolve_model_identity(
                ModelIdentityProvider::OpenAI,
                "1024-x-1024/dall-e-2",
                None,
                registry
            ),
            ModelIdentity::PricingOnly { .. }
        ));
        assert_eq!(
            resolve_model_identity(
                ModelIdentityProvider::OpenAI,
                "unknown/native/slash",
                None,
                registry
            )
            .invalid_reason(),
            Some(ModelIdentityError::UnknownModel)
        );
        assert_eq!(
            resolve_model_identity(
                ModelIdentityProvider::AzureAI,
                "BAAI/bge-m3",
                None,
                &NativeSlashCatalog,
            )
            .capability_catalog_model(),
            Some("BAAI/bge-m3")
        );
    }

    #[test]
    fn configured_deployment_keeps_all_three_identities_separate() {
        let registry = get_openai_registry();
        let owned =
            DeploymentModelIdentity::new("prod-west", Some("gpt-4".into()), Some("gpt-4".into()));
        let resolved = resolve_model_identity(
            ModelIdentityProvider::Azure,
            "customer-facing",
            Some(owned.as_ref()),
            registry,
        );
        assert_eq!(resolved.wire_model(), Some("prod-west"));
        assert_eq!(resolved.capability_catalog_model(), Some("gpt-4"));
        assert_eq!(resolved.pricing_model(), Some("gpt-4"));

        let unmapped = DeploymentModelIdentity::new("custom", None, None);
        assert!(matches!(
            resolve_model_identity(
                ModelIdentityProvider::OpenAI,
                "custom",
                Some(unmapped.as_ref()),
                registry
            ),
            ModelIdentity::ConfiguredDeployment {
                capability_catalog_model: None,
                pricing_model: None,
                ..
            }
        ));
    }

    #[test]
    fn invalid_qualifiers_targets_and_lookalikes_fail_closed() {
        let registry = get_openai_registry();
        for (raw, reason) in [
            ("anthropic/gpt-4", ModelIdentityError::WrongProvider),
            ("azure/gpt-4", ModelIdentityError::WrongProvider),
            (
                "openai/openai/gpt-4",
                ModelIdentityError::DoubleQualification,
            ),
            ("openai/fake-gpt-5", ModelIdentityError::UnknownModel),
            ("GPT-4", ModelIdentityError::UnknownModel),
            ("custom deployment", ModelIdentityError::UnknownModel),
        ] {
            assert_eq!(
                resolve_model_identity(ModelIdentityProvider::OpenAI, raw, None, registry)
                    .invalid_reason(),
                Some(reason),
                "{raw}"
            );
        }
        for identity in [
            DeploymentModelIdentity::new("prod", Some("gpt-4-suffix".into()), None),
            DeploymentModelIdentity::new(
                "prod",
                Some("1024-x-1024/dall-e-2".into()),
                Some("1024-x-1024/dall-e-2".into()),
            ),
        ] {
            assert_eq!(
                resolve_model_identity(
                    ModelIdentityProvider::Azure,
                    "prod",
                    Some(identity.as_ref()),
                    registry
                )
                .invalid_reason(),
                Some(ModelIdentityError::InvalidConfiguration)
            );
        }
    }
}
