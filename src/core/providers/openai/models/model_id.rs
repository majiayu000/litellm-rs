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

/// Provider-aware result of applying exact OpenAI catalog identity policy.
#[derive(Debug, Clone, Copy)]
pub enum OpenAIModelResolution<'registry, 'input> {
    /// The provider-local ID is an exact catalog entry or explicit alias.
    Resolved(ResolvedOpenAIModel<'registry, 'input>),
    /// The caller explicitly selected an OpenAI-hosting path, but the ID is
    /// OpenAI-shaped and absent from the exact catalog.
    ExplicitOpenAIUnknown {
        wire_id: &'input str,
        public_id: &'input str,
    },
    /// This ID belongs to a non-OpenAI model path and should be handled there.
    NotApplicable,
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
    lookup: impl FnOnce(&str) -> Option<(&'registry str, &'registry OpenAIModelSpec)>,
) -> OpenAIModelResolution<'registry, 'input> {
    let parsed = ModelIdRef::parse(model_id);
    let provider = parsed.provider();
    let is_openai_provider = provider.is_some_and(|value| value.eq_ignore_ascii_case("openai"));
    let is_azure_openai_provider = provider.is_some_and(|value| {
        value.eq_ignore_ascii_case("azure") || value.eq_ignore_ascii_case("azure_ai")
    });

    if provider.is_some() && !is_openai_provider && !is_azure_openai_provider {
        return OpenAIModelResolution::NotApplicable;
    }

    let public_id = parsed.model();
    if public_id.is_empty() {
        return if is_openai_provider {
            OpenAIModelResolution::ExplicitOpenAIUnknown {
                wire_id: parsed.raw(),
                public_id,
            }
        } else {
            OpenAIModelResolution::NotApplicable
        };
    }

    let identity = identity_for(public_id);
    let catalog_candidate = identity.map_or(public_id, |entry| entry.catalog_id);
    if let Some((catalog_id, spec)) = lookup(catalog_candidate) {
        return OpenAIModelResolution::Resolved(ResolvedOpenAIModel {
            wire_id: parsed.raw(),
            public_id,
            catalog_id,
            canonical_base_id: identity.and_then(|entry| entry.canonical_base_id),
            spec,
        });
    }

    if is_openai_provider || (is_azure_openai_provider && looks_like_openai_model(public_id)) {
        OpenAIModelResolution::ExplicitOpenAIUnknown {
            wire_id: parsed.raw(),
            public_id,
        }
    } else {
        OpenAIModelResolution::NotApplicable
    }
}

/// Whether a provider-local identifier belongs to an OpenAI naming family.
pub fn looks_like_openai_model(model: &str) -> bool {
    let model = model.to_ascii_lowercase();
    [
        "gpt-",
        "chatgpt-image-",
        "o1",
        "o3",
        "o4",
        "dall-e-",
        "whisper-",
        "tts-",
        "text-embedding-",
        "omni-moderation-",
        "computer-use-",
        "codex-",
    ]
    .iter()
    .any(|prefix| model.starts_with(prefix))
}
