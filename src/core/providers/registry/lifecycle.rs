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

/// Temporary approved baseline for provider implementation directories that
/// GH837 has not yet deleted, demoted, wired, or explicitly exempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderOrphanBaselineEntry {
    pub module_name: &'static str,
    pub lane: &'static str,
    pub issue: &'static str,
    pub owner: &'static str,
    pub expires: &'static str,
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

pub static PROVIDER_ORPHAN_BASELINE: &[ProviderOrphanBaselineEntry] = &[
    baseline(
        "ai21",
        "delete-native",
        "macro-generated chat provider awaiting GH837 disposition approval",
    ),
    baseline(
        "amazon_nova",
        "demote-to-catalog",
        "catalog-backed duplicate with native macro provider retained until demote tranche",
    ),
    baseline("baseten", "delete-native", "unwired native chat provider"),
    baseline("clarifai", "delete-native", "unwired native chat provider"),
    baseline("codestral", "delete-native", "unwired native chat provider"),
    baseline(
        "custom_api",
        "exempt",
        "macro-generated custom provider needs explicit product/architecture decision",
    ),
    baseline(
        "databricks",
        "delete-native",
        "unwired native chat provider",
    ),
    baseline(
        "datarobot",
        "delete-native",
        "macro-generated chat provider awaiting GH837 disposition approval",
    ),
    baseline(
        "deepgram",
        "non-llm-lane",
        "audio provider module without a gateway LLM factory path",
    ),
    baseline(
        "deepl",
        "non-llm-lane",
        "translation provider uses macro-generated LLMProvider surface",
    ),
    baseline(
        "elevenlabs",
        "non-llm-lane",
        "audio provider module without a gateway LLM factory path",
    ),
    baseline(
        "empower",
        "delete-native",
        "macro-generated chat provider awaiting GH837 disposition approval",
    ),
    baseline("exa_ai", "delete-native", "unwired native chat provider"),
    baseline(
        "firecrawl",
        "delete-native",
        "macro-generated chat provider awaiting GH837 disposition approval",
    ),
    baseline("gigachat", "delete-native", "unwired native chat provider"),
    baseline(
        "github",
        "demote-to-catalog",
        "catalog-backed duplicate with native provider retained until demote tranche",
    ),
    baseline(
        "google_pse",
        "non-llm-lane",
        "search provider exposes LLMProvider",
    ),
    baseline(
        "gradient_ai",
        "delete-native",
        "unwired native chat provider",
    ),
    baseline(
        "huggingface",
        "delete-native",
        "unwired native chat provider",
    ),
    baseline(
        "jina",
        "non-llm-lane",
        "embedding/rerank provider exposes LLMProvider",
    ),
    baseline("langgraph", "delete-native", "unwired native chat provider"),
    baseline("manus", "delete-native", "unwired native chat provider"),
    baseline(
        "meta_llama",
        "demote-to-catalog",
        "catalog-backed duplicate with native provider retained until demote tranche",
    ),
    baseline(
        "milvus",
        "non-llm-lane",
        "vector provider exposes LLMProvider",
    ),
    baseline("morph", "delete-native", "unwired native chat provider"),
    baseline("nlp_cloud", "delete-native", "unwired native chat provider"),
    baseline("oci", "delete-native", "unwired native chat provider"),
    baseline(
        "ollama",
        "demote-to-catalog",
        "local OpenAI-compatible candidate",
    ),
    baseline("petals", "delete-native", "unwired native chat provider"),
    baseline("predibase", "delete-native", "unwired native chat provider"),
    baseline("ragflow", "delete-native", "unwired native chat provider"),
    baseline(
        "recraft",
        "non-llm-lane",
        "image provider exposes LLMProvider",
    ),
    baseline(
        "runwayml",
        "non-llm-lane",
        "video/image provider exposes LLMProvider",
    ),
    baseline("sagemaker", "delete-native", "unwired native chat provider"),
    baseline("sap_ai", "delete-native", "unwired native chat provider"),
    baseline(
        "searxng",
        "non-llm-lane",
        "search provider exposes LLMProvider",
    ),
    baseline("snowflake", "delete-native", "unwired native chat provider"),
    baseline("spark", "delete-native", "unwired native chat provider"),
    baseline(
        "stability",
        "non-llm-lane",
        "image provider exposes LLMProvider",
    ),
    baseline(
        "tavily",
        "non-llm-lane",
        "search provider exposes LLMProvider",
    ),
    baseline("topaz", "delete-native", "unwired native chat provider"),
    baseline("triton", "delete-native", "unwired native chat provider"),
    baseline(
        "v0",
        "demote-to-catalog",
        "catalog-backed duplicate with native provider retained until demote tranche",
    ),
    baseline("vercel_ai", "delete-native", "unwired native chat provider"),
    baseline(
        "voyage",
        "non-llm-lane",
        "embedding provider exposes LLMProvider",
    ),
    baseline("watsonx", "delete-native", "unwired native chat provider"),
];

pub fn provider_module_lifecycle() -> &'static [ProviderModuleLifecycleEntry] {
    PROVIDER_MODULE_LIFECYCLE
}

pub fn provider_orphan_baseline() -> &'static [ProviderOrphanBaselineEntry] {
    PROVIDER_ORPHAN_BASELINE
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

const fn baseline(
    module_name: &'static str,
    lane: &'static str,
    reason: &'static str,
) -> ProviderOrphanBaselineEntry {
    ProviderOrphanBaselineEntry {
        module_name,
        lane,
        issue: "GH837",
        owner: "coordinator",
        expires: "remove after GH837 disposition approval and tranche execution",
        reason,
    }
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
#[path = "lifecycle_tests.rs"]
mod tests;
