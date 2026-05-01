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
    /// The provider is reachable from the runtime factory or a supported runtime path.
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
    wire(
        "amazon_nova",
        "ProviderType::AmazonNova has an explicit OpenAI-compatible factory branch",
    ),
    wire("anthropic", "native Provider enum variant"),
    wire(
        "azure",
        "ProviderType::Azure has an explicit OpenAI-compatible factory branch",
    ),
    wire(
        "azure_ai",
        "ProviderType::AzureAI has an explicit OpenAI-compatible factory branch",
    ),
    internal("base", "shared provider infrastructure"),
    stub(
        "baseten",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire(
        "bedrock",
        "ProviderType::Bedrock has an explicit OpenAI-compatible factory branch",
    ),
    stub(
        "clarifai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire("cloudflare", "native Provider enum variant"),
    stub(
        "codestral",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "cohere",
        "specialized provider module; not wired through the LLM factory yet",
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
    wire(
        "fal_ai",
        "ProviderType::FalAI has an explicit OpenAI-compatible factory branch",
    ),
    stub(
        "firecrawl",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    stub(
        "gemini",
        "specialized provider module used for model metadata but not wired through the LLM factory",
    ),
    wire(
        "github",
        "ProviderType::GitHub has an explicit OpenAI-compatible factory branch",
    ),
    wire(
        "github_copilot",
        "ProviderType::GitHubCopilot has an explicit OpenAI-compatible factory branch",
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
    wire(
        "meta_llama",
        "ProviderType::MetaLlama has an explicit OpenAI-compatible factory branch",
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
    wire(
        "replicate",
        "ProviderType::Replicate has an explicit OpenAI-compatible factory branch",
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
    internal(
        "transform",
        "shared request/response transformation infrastructure",
    ),
    stub(
        "triton",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire(
        "v0",
        "ProviderType::V0 has an explicit OpenAI-compatible factory branch",
    ),
    stub(
        "vercel_ai",
        "specialized provider module; not wired through the LLM factory yet",
    ),
    wire(
        "vertex_ai",
        "ProviderType::VertexAI has an explicit OpenAI-compatible factory branch",
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
