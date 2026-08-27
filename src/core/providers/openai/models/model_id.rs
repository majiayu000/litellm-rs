//! Explicit OpenAI catalog identities and aliases.

/// An identity whose public API name needs metadata beyond the catalog key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OpenAIModelIdentity {
    pub public_id: &'static str,
    pub catalog_id: &'static str,
    pub canonical_base_id: Option<&'static str>,
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
