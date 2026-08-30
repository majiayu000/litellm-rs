//! Canonical provider registry matrix.
//!
//! This module is the canonical provider identity matrix for enum variants:
//! aliases, display names, dispatchability, and factory support are derived
//! from these entries.

use crate::core::providers::provider_type::ProviderType;
use std::sync::LazyLock;

/// How a provider selector is currently dispatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderDispatchKind {
    /// Backed by a concrete `Provider` enum variant.
    Native,
    /// Backed by an explicit factory branch that creates `OpenAILikeProvider`.
    ExplicitOpenAiLike,
    /// Backed by the data-driven Tier-1 catalog.
    CatalogOpenAiLike,
    /// Represented in `ProviderType`, but not currently instantiable.
    UnsupportedEnum,
}

impl ProviderDispatchKind {
    pub fn is_dispatchable(self) -> bool {
        !matches!(self, Self::UnsupportedEnum)
    }
}

/// Canonical metadata for one non-custom `ProviderType` variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRegistryEntry {
    pub provider_type: ProviderType,
    pub canonical_name: &'static str,
    pub aliases: &'static [&'static str],
    pub dispatch_kind: ProviderDispatchKind,
    /// True when the canonical selector is present in `PROVIDER_CATALOG`.
    pub catalog_backed: bool,
}

impl ProviderRegistryEntry {
    pub fn is_dispatchable(&self) -> bool {
        self.dispatch_kind.is_dispatchable()
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.canonical_name).chain(self.aliases.iter().copied())
    }

    pub fn matches_name(&self, name: &str) -> bool {
        let normalized = name.trim().to_ascii_lowercase();
        self.names().any(|candidate| candidate == normalized)
    }
}

pub static PROVIDER_TYPE_REGISTRY: &[ProviderRegistryEntry] = &[
    entry(
        ProviderType::OpenAI,
        "openai",
        &[],
        ProviderDispatchKind::Native,
        false,
    ),
    entry(
        ProviderType::Anthropic,
        "anthropic",
        &[],
        ProviderDispatchKind::Native,
        false,
    ),
    entry(
        ProviderType::Bedrock,
        "bedrock",
        &["aws-bedrock"],
        ProviderDispatchKind::Native,
        false,
    ),
    entry(
        ProviderType::OpenRouter,
        "openrouter",
        &[],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::VertexAI,
        "vertex_ai",
        &["vertexai", "vertex-ai"],
        provider_extra_only_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::Gemini,
        "gemini",
        &["google-gemini", "google_ai", "google-ai"],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::Azure,
        "azure",
        &["azure-openai"],
        provider_extra_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::AzureAI,
        "azure_ai",
        &["azureai", "azure-ai"],
        provider_extra_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::AI21,
        "ai21",
        &["ai21_chat", "ai21-chat"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Baseten,
        "baseten",
        &[],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Cohere,
        "cohere",
        &["cohere-ai"],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::DeepSeek,
        "deepseek",
        &["deep-seek"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::DeepInfra,
        "deepinfra",
        &["deep-infra"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::V0,
        "v0",
        &[],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::MetaLlama,
        "meta_llama",
        &["llama", "meta-llama"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Ollama,
        "ollama",
        &[],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::Mistral,
        "mistral",
        &["mistralai"],
        ProviderDispatchKind::Native,
        false,
    ),
    entry(
        ProviderType::Moonshot,
        "moonshot",
        &["moonshot-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Minimax,
        "minimax",
        &["minimax-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Dashscope,
        "dashscope",
        &["alibaba", "qwen", "tongyi"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Groq,
        "groq",
        &[],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::XAI,
        "xai",
        &[],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Cloudflare,
        "cloudflare",
        &["cf", "workers-ai"],
        ProviderDispatchKind::Native,
        false,
    ),
    entry(
        ProviderType::Perplexity,
        "perplexity",
        &["perplexity-ai", "pplx"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Replicate,
        "replicate",
        &["replicate-ai"],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::FalAI,
        "fal_ai",
        &["fal-ai", "fal"],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::AmazonNova,
        "amazon_nova",
        &["amazon-nova", "nova"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::GitHub,
        "github",
        &["github-models"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::GitHubCopilot,
        "github_copilot",
        &["github-copilot", "copilot"],
        providers_extended_native_dispatch_kind(),
        false,
    ),
    entry(
        ProviderType::HuggingFace,
        "huggingface",
        &["hugging_face", "hugging-face"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Hyperbolic,
        "hyperbolic",
        &["hyperbolic-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Infinity,
        "infinity",
        &["infinity-embedding"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Novita,
        "novita",
        &["novita-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Volcengine,
        "volcengine",
        &["volc", "doubao", "bytedance"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Nebius,
        "nebius",
        &["nebius-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::Nscale,
        "nscale",
        &["nscale-ai"],
        ProviderDispatchKind::CatalogOpenAiLike,
        true,
    ),
    entry(
        ProviderType::PydanticAI,
        "pydantic_ai",
        &["pydantic-ai", "pydantic"],
        ProviderDispatchKind::UnsupportedEnum,
        false,
    ),
    entry(
        ProviderType::OpenAICompatible,
        "openai_compatible",
        &["openai-compatible", "openai_like", "openai-like"],
        ProviderDispatchKind::ExplicitOpenAiLike,
        false,
    ),
];

/// Catalog providers registered by the default router when their configured
/// environment credentials are available.
///
/// Keep this list in registry metadata instead of router startup code so runtime
/// registration and catalog definitions drift together.
pub static DEFAULT_CATALOG_RUNTIME_PROVIDERS: &[&str] = &[
    "openrouter",
    "deepseek",
    "moonshot",
    "minimax",
    "zhipu",
    "zai",
    "together_ai",
    "fireworks_ai",
    "aiml",
    "groq",
    "xiaomi_mimo",
    "xai",
];

static DISPATCHABLE_PROVIDER_TYPES: LazyLock<Vec<ProviderType>> = LazyLock::new(|| {
    PROVIDER_TYPE_REGISTRY
        .iter()
        .filter(|entry| entry.is_dispatchable())
        .map(|entry| entry.provider_type.clone())
        .collect()
});

pub fn provider_type_registry() -> &'static [ProviderRegistryEntry] {
    PROVIDER_TYPE_REGISTRY
}

pub fn entry_for_type(provider_type: &ProviderType) -> Option<&'static ProviderRegistryEntry> {
    PROVIDER_TYPE_REGISTRY
        .iter()
        .find(|entry| &entry.provider_type == provider_type)
}

pub fn catalog_dispatch_entry_for_type(
    provider_type: &ProviderType,
) -> Option<&'static ProviderRegistryEntry> {
    entry_for_type(provider_type)
        .filter(|entry| entry.dispatch_kind == ProviderDispatchKind::CatalogOpenAiLike)
}

pub fn catalog_dispatch_entries() -> impl Iterator<Item = &'static ProviderRegistryEntry> {
    PROVIDER_TYPE_REGISTRY
        .iter()
        .filter(|entry| entry.dispatch_kind == ProviderDispatchKind::CatalogOpenAiLike)
}

pub fn entry_for_name(name: &str) -> Option<&'static ProviderRegistryEntry> {
    PROVIDER_TYPE_REGISTRY
        .iter()
        .find(|entry| entry.matches_name(name))
}

pub fn dispatchable_provider_types() -> Vec<ProviderType> {
    dispatchable_provider_types_slice().to_vec()
}

pub fn dispatchable_provider_types_slice() -> &'static [ProviderType] {
    DISPATCHABLE_PROVIDER_TYPES.as_slice()
}

pub fn default_catalog_runtime_provider_names() -> &'static [&'static str] {
    DEFAULT_CATALOG_RUNTIME_PROVIDERS
}

const fn entry(
    provider_type: ProviderType,
    canonical_name: &'static str,
    aliases: &'static [&'static str],
    dispatch_kind: ProviderDispatchKind,
    catalog_backed: bool,
) -> ProviderRegistryEntry {
    ProviderRegistryEntry {
        provider_type,
        canonical_name,
        aliases,
        dispatch_kind,
        catalog_backed,
    }
}

const fn provider_extra_native_dispatch_kind() -> ProviderDispatchKind {
    if cfg!(feature = "providers-extra") {
        ProviderDispatchKind::Native
    } else {
        ProviderDispatchKind::ExplicitOpenAiLike
    }
}

const fn providers_extended_native_dispatch_kind() -> ProviderDispatchKind {
    if cfg!(feature = "providers-extended") {
        ProviderDispatchKind::Native
    } else {
        ProviderDispatchKind::UnsupportedEnum
    }
}

const fn provider_extra_only_native_dispatch_kind() -> ProviderDispatchKind {
    if cfg!(feature = "providers-extra") {
        ProviderDispatchKind::Native
    } else {
        ProviderDispatchKind::UnsupportedEnum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::{Provider, provider_type::all_non_custom_provider_types};
    use std::collections::HashSet;
    use std::str::FromStr;

    fn sorted_type_names(values: impl IntoIterator<Item = ProviderType>) -> Vec<String> {
        let mut values = values
            .into_iter()
            .map(|provider_type| provider_type.to_string())
            .collect::<Vec<_>>();
        values.sort();
        values
    }

    fn dispatch_kind_for(provider_type: &ProviderType) -> ProviderDispatchKind {
        match entry_for_type(provider_type) {
            Some(entry) => entry.dispatch_kind,
            None => panic!("missing provider registry entry for {:?}", provider_type),
        }
    }

    #[test]
    fn provider_registry_contains_all_non_custom_provider_types() {
        assert_eq!(
            sorted_type_names(all_non_custom_provider_types()),
            sorted_type_names(
                PROVIDER_TYPE_REGISTRY
                    .iter()
                    .map(|entry| entry.provider_type.clone())
            )
        );
    }

    #[test]
    fn provider_registry_aliases_parse_to_declared_type() {
        for entry in PROVIDER_TYPE_REGISTRY {
            for name in entry.names() {
                assert_eq!(
                    ProviderType::from_str(name).unwrap(),
                    entry.provider_type,
                    "alias {name} should parse to {:?}",
                    entry.provider_type
                );
            }
        }
    }

    #[test]
    fn provider_registry_display_names_match_canonical_names() {
        for entry in PROVIDER_TYPE_REGISTRY {
            assert_eq!(entry.provider_type.to_string(), entry.canonical_name);
        }
    }

    #[test]
    fn provider_registry_native_entries_match_provider_enum_variants() {
        let native_types = PROVIDER_TYPE_REGISTRY
            .iter()
            .filter(|entry| entry.dispatch_kind == ProviderDispatchKind::Native)
            .map(|entry| entry.provider_type.clone())
            .collect::<HashSet<_>>();
        let expected = [
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::Bedrock,
            ProviderType::Mistral,
            ProviderType::Cloudflare,
        ]
        .into_iter()
        .chain([
            #[cfg(feature = "providers-extra")]
            ProviderType::Azure,
            #[cfg(feature = "providers-extra")]
            ProviderType::AzureAI,
            #[cfg(feature = "providers-extra")]
            ProviderType::VertexAI,
            #[cfg(feature = "providers-extended")]
            ProviderType::Cohere,
            #[cfg(feature = "providers-extended")]
            ProviderType::FalAI,
            #[cfg(feature = "providers-extended")]
            ProviderType::Replicate,
            #[cfg(feature = "providers-extended")]
            ProviderType::Gemini,
            #[cfg(feature = "providers-extended")]
            ProviderType::GitHubCopilot,
            #[cfg(feature = "providers-extended")]
            ProviderType::Ollama,
        ])
        .collect::<HashSet<_>>();

        assert_eq!(native_types, expected);
    }

    #[test]
    fn provider_registry_phase0_key_provider_classifications() {
        assert_eq!(
            dispatch_kind_for(&ProviderType::Bedrock),
            ProviderDispatchKind::Native
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::VertexAI),
            provider_extra_only_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::Gemini),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::Ollama),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::Azure),
            provider_extra_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::AzureAI),
            provider_extra_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::GitHubCopilot),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::Cohere),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::FalAI),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::Replicate),
            providers_extended_native_dispatch_kind()
        );
        assert_eq!(
            dispatch_kind_for(&ProviderType::OpenAICompatible),
            ProviderDispatchKind::ExplicitOpenAiLike
        );
        for provider_type in [
            ProviderType::MetaLlama,
            ProviderType::V0,
            ProviderType::AmazonNova,
            ProviderType::GitHub,
        ] {
            assert_eq!(
                dispatch_kind_for(&provider_type),
                ProviderDispatchKind::CatalogOpenAiLike,
                "{provider_type:?} should be catalog-only metadata"
            );
        }
        assert_eq!(
            dispatch_kind_for(&ProviderType::PydanticAI),
            ProviderDispatchKind::UnsupportedEnum
        );
    }

    #[test]
    fn provider_registry_dispatchability_matches_factory_supported_types() {
        assert_eq!(
            sorted_type_names(dispatchable_provider_types()),
            sorted_type_names(Provider::factory_supported_provider_types().iter().cloned())
        );
    }

    #[test]
    fn provider_registry_catalog_flags_match_catalog() {
        for entry in PROVIDER_TYPE_REGISTRY {
            assert_eq!(
                super::super::catalog::get_definition(entry.canonical_name).is_some(),
                entry.catalog_backed,
                "{} catalog flag drifted",
                entry.canonical_name
            );
        }
    }

    #[test]
    fn provider_registry_catalog_dispatch_entries_have_catalog_definitions() {
        for entry in catalog_dispatch_entries() {
            assert!(
                entry.catalog_backed,
                "{} catalog dispatch entry must be marked catalog-backed",
                entry.canonical_name
            );
            assert!(
                super::super::catalog::get_definition(entry.canonical_name).is_some(),
                "{} catalog dispatch entry must have a provider definition",
                entry.canonical_name
            );
        }
    }

    #[test]
    fn provider_registry_default_runtime_catalog_providers_are_known_catalog_names() {
        for provider_name in default_catalog_runtime_provider_names() {
            assert!(
                super::super::catalog::get_definition(provider_name).is_some(),
                "{provider_name} default runtime provider must exist in the catalog"
            );

            if let Some(entry) = entry_for_name(provider_name) {
                assert_eq!(
                    entry.dispatch_kind,
                    ProviderDispatchKind::CatalogOpenAiLike,
                    "{provider_name} default runtime ProviderType entry must be catalog dispatchable"
                );
            }
        }
    }

    #[test]
    fn provider_registry_names_are_unique() {
        let mut seen = HashSet::new();
        for entry in PROVIDER_TYPE_REGISTRY {
            for name in entry.names() {
                assert!(
                    seen.insert(name),
                    "duplicate provider registry name: {name}"
                );
            }
        }
    }
}
