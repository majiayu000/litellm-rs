//! Exact OpenAI catalog identity resolution types.
//!
//! [`ModelIdRef`](crate::core::types::model_id::ModelIdRef) remains a lossless
//! syntax view. These types describe semantic results only after an OpenAI
//! registry has accepted an exact key or one provider-qualified form.

use super::OpenAIModelSpec;

#[derive(Debug, Clone, Copy)]
pub(super) struct ExplicitOpenAIModelIdentity {
    pub catalog_key: &'static str,
    pub canonical_base_model_id: &'static str,
    pub source: &'static str,
}

// OpenAI documents these exact dated IDs as snapshots of the corresponding
// base aliases. No date-shape or family-shape inference is permitted here.
const EXPLICIT_MODEL_IDENTITIES: &[ExplicitOpenAIModelIdentity] = &[
    ExplicitOpenAIModelIdentity {
        catalog_key: "gpt-5.5-2026-04-23",
        canonical_base_model_id: "gpt-5.5",
        source: "https://developers.openai.com/api/docs/models/gpt-5.5",
    },
    ExplicitOpenAIModelIdentity {
        catalog_key: "gpt-5.5-pro-2026-04-23",
        canonical_base_model_id: "gpt-5.5-pro",
        source: "https://developers.openai.com/api/docs/models/gpt-5.5-pro",
    },
];

pub(super) fn explicit_identity_for(catalog_key: &str) -> Option<ExplicitOpenAIModelIdentity> {
    EXPLICIT_MODEL_IDENTITIES
        .iter()
        .copied()
        .find(|identity| identity.catalog_key == catalog_key)
}

/// Why a catalog lookup failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OpenAICatalogResolveError<'input> {
    /// No exact catalog key or explicit provider-qualified form exists.
    #[error("OpenAI catalog has no exact entry for `{model_id}`")]
    UnknownModel { model_id: &'input str },
    /// The caller explicitly selected another provider.
    #[error("model `{model_id}` is qualified for `{provider}`, not `openai`")]
    WrongProvider {
        model_id: &'input str,
        provider: &'input str,
    },
    /// More than one OpenAI provider layer was supplied.
    #[error("model `{model_id}` contains more than one `openai` provider qualifier")]
    DoubleQualification { model_id: &'input str },
}

/// Runtime meaning of an OpenAI catalog lookup.
///
/// Keeping these states distinct prevents exact pricing rows from inheriting
/// provider-wide capabilities while preserving pass-through for deployments
/// that are absent from the static catalog.
#[derive(Debug, Clone, Copy)]
pub enum OpenAICatalogRuntimeResolution<'registry, 'input> {
    /// An exact catalog model with callable capability metadata.
    Callable(&'registry OpenAIModelSpec),
    /// An exact catalog row used only for pricing/metadata.
    PricingOnly(&'registry OpenAIModelSpec),
    /// No exact OpenAI catalog identity was resolved.
    Unresolved(OpenAICatalogResolveError<'input>),
}

/// An exact OpenAI catalog entry and its optional runnable-model identity.
///
/// Slash-bearing raw catalog keys are conservatively treated as pricing-only.
/// They may gain capabilities only through a separately reviewed, explicit
/// alias; this resolver never infers an alias from a final path segment.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedOpenAICatalogEntry<'registry, 'input> {
    raw_id: &'input str,
    catalog_key: &'registry str,
    catalog_spec: &'registry OpenAIModelSpec,
    capability_identity: Option<(&'registry str, &'registry OpenAIModelSpec)>,
    canonical_base_model_id: Option<&'static str>,
    alias_source: Option<&'static str>,
}

impl<'registry, 'input> ResolvedOpenAICatalogEntry<'registry, 'input> {
    pub(super) fn new(
        raw_id: &'input str,
        catalog_key: &'registry str,
        catalog_spec: &'registry OpenAIModelSpec,
        capability_identity: Option<(&'registry str, &'registry OpenAIModelSpec)>,
        canonical_base_model_id: Option<&'static str>,
        alias_source: Option<&'static str>,
    ) -> Self {
        Self {
            raw_id,
            catalog_key,
            catalog_spec,
            capability_identity,
            canonical_base_model_id,
            alias_source,
        }
    }

    /// Return the exact identifier supplied by the caller for wire forwarding.
    pub fn raw_id(&self) -> &'input str {
        self.raw_id
    }

    /// Return the exact key stored in the OpenAI catalog.
    pub fn catalog_key(&self) -> &'registry str {
        self.catalog_key
    }

    /// Return metadata for the exact catalog entry, including pricing-only rows.
    pub fn catalog_spec(&self) -> &'registry OpenAIModelSpec {
        self.catalog_spec
    }

    /// Return the exact runnable-model key, if this entry has capability semantics.
    pub fn capability_model_id(&self) -> Option<&'registry str> {
        self.capability_identity.map(|(model_id, _)| model_id)
    }

    /// Return model capability metadata without promoting pricing-only rows.
    pub fn capability_spec(&self) -> Option<&'registry OpenAIModelSpec> {
        self.capability_identity.map(|(_, spec)| spec)
    }

    /// Return the explicitly sourced base alias for a dated identity.
    pub fn canonical_base_model_id(&self) -> Option<&'static str> {
        self.canonical_base_model_id
    }

    /// Return the official source supporting the explicit base/snapshot relation.
    pub fn alias_source(&self) -> Option<&'static str> {
        self.alias_source
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::OpenAIModelRegistry, OpenAICatalogResolveError, OpenAICatalogRuntimeResolution,
    };

    #[test]
    fn runtime_resolution_keeps_callable_pricing_only_and_unknown_distinct() {
        let registry = OpenAIModelRegistry::new();

        assert!(matches!(
            registry.resolve_runtime_identity("openai/gpt-4"),
            OpenAICatalogRuntimeResolution::Callable(spec)
                if spec.model_info.id == "gpt-4"
        ));
        assert!(matches!(
            registry.resolve_runtime_identity("openai/1024-x-1024/dall-e-2"),
            OpenAICatalogRuntimeResolution::PricingOnly(spec)
                if spec.model_info.id == "1024-x-1024/dall-e-2"
        ));
        assert!(matches!(
            registry.resolve_runtime_identity("custom-openai-deployment"),
            OpenAICatalogRuntimeResolution::Unresolved(
                OpenAICatalogResolveError::UnknownModel { .. }
            )
        ));
    }

    #[test]
    fn resolves_exact_and_single_openai_qualification() {
        let registry = OpenAIModelRegistry::new();

        let exact = registry
            .resolve_catalog_identity("gpt-4")
            .expect("exact model should resolve");
        let qualified = registry
            .resolve_catalog_identity("openai/gpt-4")
            .expect("qualified model should resolve");
        let mixed_provider_case = registry
            .resolve_catalog_identity("OPENAI/gpt-4")
            .expect("provider segment should be ASCII-case-insensitive");

        assert_eq!(exact.raw_id(), "gpt-4");
        assert_eq!(qualified.raw_id(), "openai/gpt-4");
        assert_eq!(mixed_provider_case.raw_id(), "OPENAI/gpt-4");
        assert_eq!(exact.catalog_key(), "gpt-4");
        assert_eq!(qualified.catalog_key(), "gpt-4");
        assert_eq!(mixed_provider_case.catalog_key(), "gpt-4");
        assert!(exact.capability_spec().is_some());
        assert!(qualified.capability_spec().is_some());
    }

    #[test]
    fn prefers_exact_native_slash_keys() {
        let registry = OpenAIModelRegistry::new();
        let key = "1024-x-1024/dall-e-2";

        let exact = registry
            .resolve_catalog_identity(key)
            .expect("exact slash-bearing pricing key should resolve");
        let qualified = registry
            .resolve_catalog_identity("openai/1024-x-1024/dall-e-2")
            .expect("one OpenAI provider layer should resolve the exact nested key");

        assert_eq!(exact.raw_id(), key);
        assert_eq!(exact.catalog_key(), key);
        assert_eq!(qualified.catalog_key(), key);
        assert!(exact.capability_spec().is_none());
        assert!(qualified.capability_spec().is_none());
    }

    #[test]
    fn every_exact_slash_catalog_key_is_reachable() {
        let registry = OpenAIModelRegistry::new();

        for model in registry
            .get_all_models()
            .into_iter()
            .filter(|model| model.id.contains('/'))
        {
            let resolved = registry
                .resolve_catalog_identity(&model.id)
                .unwrap_or_else(|error| panic!("{} should resolve exactly: {error}", model.id));

            assert_eq!(resolved.catalog_key(), model.id);
        }
    }

    #[test]
    fn normalizes_only_the_provider_segment_for_stored_openai_keys() {
        let registry = OpenAIModelRegistry::new();

        let resolved = registry
            .resolve_catalog_identity("OPENAI/container")
            .expect("canonical stored key should resolve with provider-case normalization");
        assert_eq!(resolved.catalog_key(), "openai/container");
        assert!(resolved.capability_spec().is_none());

        assert!(matches!(
            registry.resolve_catalog_identity("openai/GPT-4"),
            Err(OpenAICatalogResolveError::UnknownModel { .. })
        ));
    }

    #[test]
    fn rejects_unknown_collisions_and_wrong_qualifiers() {
        let registry = OpenAIModelRegistry::new();

        for model in [
            "gpt-4-fake",
            "gpt-4-2099-01-01",
            "prefix-gpt-4",
            "openai/gpt-4-fake",
        ] {
            assert!(matches!(
                registry.resolve_catalog_identity(model),
                Err(OpenAICatalogResolveError::UnknownModel { .. })
            ));
        }

        assert!(matches!(
            registry.resolve_catalog_identity("anthropic/gpt-4"),
            Err(OpenAICatalogResolveError::WrongProvider {
                provider: "anthropic",
                ..
            })
        ));
        assert!(matches!(
            registry.resolve_catalog_identity("openai/openai/gpt-4"),
            Err(OpenAICatalogResolveError::DoubleQualification { .. })
        ));
    }

    #[test]
    fn keeps_base_and_dated_catalog_identities_distinct() {
        let registry = OpenAIModelRegistry::new();

        let base = registry
            .resolve_catalog_identity("openai/gpt-5.5")
            .expect("base model should resolve");
        let dated = registry
            .resolve_catalog_identity("openai/gpt-5.5-2026-04-23")
            .expect("dated model should resolve");

        assert_eq!(base.catalog_key(), "gpt-5.5");
        assert_eq!(dated.catalog_key(), "gpt-5.5-2026-04-23");
        assert_ne!(base.catalog_key(), dated.catalog_key());
        assert_eq!(base.canonical_base_model_id(), None);
        assert_eq!(dated.canonical_base_model_id(), Some("gpt-5.5"));
        assert_eq!(
            dated.alias_source(),
            Some("https://developers.openai.com/api/docs/models/gpt-5.5")
        );

        let pro_snapshot = registry
            .resolve_catalog_identity("gpt-5.5-pro-2026-04-23")
            .expect("Pro snapshot should resolve");
        assert_eq!(pro_snapshot.canonical_base_model_id(), Some("gpt-5.5-pro"));
        assert_eq!(
            pro_snapshot.alias_source(),
            Some("https://developers.openai.com/api/docs/models/gpt-5.5-pro")
        );
    }
}
