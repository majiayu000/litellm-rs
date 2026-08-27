//! Explicit OpenAI catalog identities and aliases.

use crate::core::types::model_id::ModelIdRef;

use super::registry_types::OpenAIModelSpec;

/// An identity whose public API name needs metadata beyond the catalog key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OpenAIModelIdentity {
    pub public_id: &'static str,
    pub catalog_id: &'static str,
    pub canonical_base_id: Option<&'static str>,
}

/// A catalog hit that preserves the caller's exact wire identifier.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedOpenAIModel<'registry, 'input> {
    wire_id: &'input str,
    public_id: &'input str,
    catalog_id: &'registry str,
    canonical_base_id: Option<&'static str>,
    spec: &'registry OpenAIModelSpec,
}

impl<'registry, 'input> ResolvedOpenAIModel<'registry, 'input> {
    /// Return the exact identifier received from the caller.
    pub fn wire_id(self) -> &'input str {
        self.wire_id
    }

    /// Return the provider-local identifier exposed by the API.
    pub fn public_id(self) -> &'input str {
        self.public_id
    }

    /// Return the exact key used to retrieve the catalog entry.
    pub fn catalog_id(self) -> &'registry str {
        self.catalog_id
    }

    /// Return an explicitly declared canonical base model, when one exists.
    pub fn canonical_base_id(self) -> Option<&'static str> {
        self.canonical_base_id
    }

    /// Return the resolved model specification.
    pub fn spec(self) -> &'registry OpenAIModelSpec {
        self.spec
    }
}

const MODEL_IDENTITIES: &[OpenAIModelIdentity] = &[OpenAIModelIdentity {
    public_id: "gpt-5.5-2026-04-23",
    catalog_id: "gpt-5.5-2026-04-23",
    canonical_base_id: Some("gpt-5.5"),
}];

/// Look up metadata for an exact public model identifier.
pub(super) fn identity_for(public_id: &str) -> Option<OpenAIModelIdentity> {
    MODEL_IDENTITIES
        .iter()
        .copied()
        .find(|identity| identity.public_id == public_id)
}

pub(super) fn resolve_with_catalog<'registry, 'input>(
    model_id: &'input str,
    mut lookup: impl FnMut(&str) -> Option<(&'registry str, &'registry OpenAIModelSpec)>,
) -> Option<ResolvedOpenAIModel<'registry, 'input>> {
    let parsed = ModelIdRef::parse(model_id);
    let public_id = parsed.for_provider("openai")?;

    let identity = identity_for(public_id);
    let catalog_entry = identity
        .and_then(|entry| lookup(entry.catalog_id))
        .or_else(|| parsed.provider().and_then(|_| lookup(parsed.raw())))
        .or_else(|| lookup(public_id));

    catalog_entry.map(|(catalog_id, spec)| ResolvedOpenAIModel {
        wire_id: parsed.raw(),
        public_id,
        catalog_id,
        canonical_base_id: identity.and_then(|entry| entry.canonical_base_id),
        spec,
    })
}
