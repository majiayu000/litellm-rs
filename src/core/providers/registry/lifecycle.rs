//! Provider module lifecycle decisions.
//!
//! This manifest records the lifecycle decision for every directory under
//! `src/core/providers`.  It is deliberately conservative: modules that compile
//! but are not reachable from the gateway LLM factory are kept as `Stub` until a
//! focused PR either wires them, moves them to catalog-only metadata, or deletes
//! them with owner confirmation.

/// Lifecycle decision for a provider module directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderModuleLifecycle {
    /// The provider module is reachable through a native runtime `Provider` enum branch.
    Wire,
    /// The provider should be represented by catalog metadata instead of a code module.
    CatalogOnly,
    /// The code module is intentionally retained but is not runtime-wired yet.
    Stub,
    /// The directory is shared infrastructure, not a provider implementation.
    Internal,
    /// The directory is a candidate for removal after owner confirmation.
    Delete,
}

/// Lifecycle entry for one directory under `src/core/providers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderModuleLifecycleEntry {
    pub module_name: &'static str,
    pub lifecycle: ProviderModuleLifecycle,
    pub reason: &'static str,
}

pub static PROVIDER_MODULE_LIFECYCLE: &[ProviderModuleLifecycleEntry] = &[
    stub(
        "ai21",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "amazon_nova",
        "native module retained; ProviderType::AmazonNova currently uses a generic OpenAI-compatible adapter",
    ),
    wire("anthropic", "native Provider enum variant"),
    provider_extra_wire(
        "azure",
        "native Provider enum variant when providers-extra is enabled; otherwise unsupported without generic fallback",
    ),
    provider_extra_wire(
        "azure_ai",
        "native Provider enum variant when providers-extra is enabled; otherwise unsupported without generic fallback",
    ),
    internal("base", "shared provider infrastructure"),
    stub(
        "baseten",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire("bedrock", "native Provider enum variant"),
    stub(
        "clarifai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire("cloudflare", "native Provider enum variant"),
    stub(
        "codestral",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    providers_extended_wire(
        "cohere",
        "ProviderType::Cohere dispatches to native Cohere API paths when providers-extended is enabled",
    ),
    stub(
        "custom_api",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "databricks",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "datarobot",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "deepgram",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "deepl",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "elevenlabs",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "empower",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "exa_ai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    internal("factory", "provider construction infrastructure"),
    providers_extended_wire(
        "fal_ai",
        "ProviderType::FalAI dispatches to native image-generation endpoints when providers-extended is enabled",
    ),
    stub(
        "firecrawl",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    providers_extended_wire(
        "gemini",
        "ProviderType::Gemini dispatches to native Google AI Studio Gemini auth when providers-extended is enabled",
    ),
    stub(
        "github",
        "native GitHub module retained; ProviderType::GitHub currently uses a generic OpenAI-compatible adapter",
    ),
    providers_extended_wire(
        "github_copilot",
        "ProviderType::GitHubCopilot dispatches to native GitHub Copilot auth when providers-extended is enabled",
    ),
    stub(
        "gigachat",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "google_pse",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "gradient_ai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "huggingface",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "jina",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "langgraph",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    internal("macros", "provider macro infrastructure"),
    stub(
        "manus",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "meta_llama",
        "native Meta Llama module retained; ProviderType::MetaLlama currently uses a generic OpenAI-compatible adapter",
    ),
    internal(
        "milvus",
        "vector-store provider module, outside LLM factory dispatch",
    ),
    wire("mistral", "native Provider enum variant"),
    stub(
        "morph",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "nlp_cloud",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "oci",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "ollama",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire("openai", "native Provider enum variant"),
    wire("openai_like", "shared OpenAI-compatible runtime provider"),
    stub(
        "petals",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    internal(
        "pg_vector",
        "vector-store provider module, outside LLM factory dispatch",
    ),
    stub(
        "predibase",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "ragflow",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "recraft",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    internal("registry", "provider catalog and lifecycle infrastructure"),
    providers_extended_wire(
        "replicate",
        "ProviderType::Replicate dispatches to native prediction lifecycle paths when providers-extended is enabled",
    ),
    stub(
        "runwayml",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "sagemaker",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "sap_ai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "searxng",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "snowflake",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "spark",
        "specialized provider module used for model metadata but not wired through the LLM factory",
    ),
    stub(
        "stability",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "tavily",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    internal("thinking", "shared thinking/reasoning support"),
    stub(
        "topaz",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "triton",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "v0",
        "native V0 module retained; ProviderType::V0 currently uses a generic OpenAI-compatible adapter",
    ),
    stub(
        "vercel_ai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    provider_extra_wire(
        "vertex_ai",
        "ProviderType::VertexAI dispatches to native Vertex AI auth when providers-extra is enabled",
    ),
    stub(
        "voyage",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "watsonx",
        "specialized provider module; not wired through the LLM factory yet",
    ),
];

pub fn provider_module_lifecycle() -> &'static [ProviderModuleLifecycleEntry] {
    PROVIDER_MODULE_LIFECYCLE
}

const fn wire(module_name: &'static str, reason: &'static str) -> ProviderModuleLifecycleEntry {
    entry(module_name, ProviderModuleLifecycle::Wire, reason)
}

const fn stub(module_name: &'static str, reason: &'static str) -> ProviderModuleLifecycleEntry {
    entry(module_name, ProviderModuleLifecycle::Stub, reason)
}

const fn internal(module_name: &'static str, reason: &'static str) -> ProviderModuleLifecycleEntry {
    entry(module_name, ProviderModuleLifecycle::Internal, reason)
}

const fn provider_extra_wire(
    module_name: &'static str,
    reason: &'static str,
) -> ProviderModuleLifecycleEntry {
    let lifecycle = if cfg!(feature = "providers-extra") {
        ProviderModuleLifecycle::Wire
    } else {
        ProviderModuleLifecycle::Stub
    };
    entry(module_name, lifecycle, reason)
}

const fn providers_extended_wire(
    module_name: &'static str,
    reason: &'static str,
) -> ProviderModuleLifecycleEntry {
    let lifecycle = if cfg!(feature = "providers-extended") {
        ProviderModuleLifecycle::Wire
    } else {
        ProviderModuleLifecycle::Stub
    };
    entry(module_name, lifecycle, reason)
}

const fn entry(
    module_name: &'static str,
    lifecycle: ProviderModuleLifecycle,
    reason: &'static str,
) -> ProviderModuleLifecycleEntry {
    ProviderModuleLifecycleEntry {
        module_name,
        lifecycle,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    fn lifecycle_module_names() -> BTreeSet<&'static str> {
        PROVIDER_MODULE_LIFECYCLE
            .iter()
            .map(|entry| entry.module_name)
            .collect()
    }

    fn lifecycle_for(module_name: &str) -> ProviderModuleLifecycle {
        PROVIDER_MODULE_LIFECYCLE
            .iter()
            .find(|entry| entry.module_name == module_name)
            .unwrap_or_else(|| panic!("missing lifecycle entry for {module_name}"))
            .lifecycle
    }

    #[test]
    fn lifecycle_classifies_phase0_key_provider_modules() {
        assert_eq!(lifecycle_for("bedrock"), ProviderModuleLifecycle::Wire);
        assert_eq!(
            lifecycle_for("vertex_ai"),
            if cfg!(feature = "providers-extra") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("azure"),
            if cfg!(feature = "providers-extra") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("azure_ai"),
            if cfg!(feature = "providers-extra") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("github_copilot"),
            if cfg!(feature = "providers-extended") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("cohere"),
            if cfg!(feature = "providers-extended") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("fal_ai"),
            if cfg!(feature = "providers-extended") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("replicate"),
            if cfg!(feature = "providers-extended") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
        assert_eq!(
            lifecycle_for("gemini"),
            if cfg!(feature = "providers-extended") {
                ProviderModuleLifecycle::Wire
            } else {
                ProviderModuleLifecycle::Stub
            }
        );
    }

    #[test]
    fn lifecycle_wire_entries_are_native_runtime_modules() {
        let actual = PROVIDER_MODULE_LIFECYCLE
            .iter()
            .filter(|entry| entry.lifecycle == ProviderModuleLifecycle::Wire)
            .map(|entry| entry.module_name)
            .collect::<BTreeSet<_>>();
        let expected = [
            "anthropic",
            #[cfg(feature = "providers-extra")]
            "azure",
            #[cfg(feature = "providers-extra")]
            "azure_ai",
            "bedrock",
            "cloudflare",
            #[cfg(feature = "providers-extended")]
            "cohere",
            #[cfg(feature = "providers-extended")]
            "fal_ai",
            #[cfg(feature = "providers-extended")]
            "gemini",
            #[cfg(feature = "providers-extended")]
            "github_copilot",
            #[cfg(feature = "providers-extended")]
            "replicate",
            "mistral",
            "openai",
            "openai_like",
            #[cfg(feature = "providers-extra")]
            "vertex_ai",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }

    #[test]
    fn lifecycle_covers_every_provider_directory() {
        let providers_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers");
        let actual = fs::read_dir(providers_dir)
            .expect("providers directory should be readable")
            .filter_map(|entry| {
                let entry = entry.expect("provider directory entry should be readable");
                if !entry
                    .file_type()
                    .expect("file type should be readable")
                    .is_dir()
                {
                    return None;
                }
                entry.file_name().into_string().ok()
            })
            .collect::<BTreeSet<_>>();
        let declared = lifecycle_module_names()
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, declared);
    }

    #[test]
    fn lifecycle_has_no_delete_decisions_without_owner_confirmation() {
        assert!(
            PROVIDER_MODULE_LIFECYCLE
                .iter()
                .all(|entry| entry.lifecycle != ProviderModuleLifecycle::Delete),
            "Delete lifecycle requires explicit owner confirmation"
        );
    }

    #[test]
    fn lifecycle_entries_have_reasons() {
        for entry in PROVIDER_MODULE_LIFECYCLE {
            assert!(
                !entry.reason.trim().is_empty(),
                "{} lifecycle entry must include a reason",
                entry.module_name
            );
        }
    }
}
