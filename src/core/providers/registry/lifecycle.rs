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
    wire("bedrock", "native Provider enum variant"),
    providers_extended_wire(
        "black_forest_labs",
        "ProviderType::BlackForestLabs dispatches to native asynchronous image endpoints",
    ),
    wire("cloudflare", "native Provider enum variant"),
    wire("databricks", "typed Databricks Model Serving runtime"),
    wire("deepgram", "native audio Provider enum variant"),
    wire("elevenlabs", "native audio Provider enum variant"),
    providers_extended_wire(
        "cohere",
        "ProviderType::Cohere dispatches to native Cohere API paths when providers-extended is enabled",
    ),
    internal("factory", "provider construction infrastructure"),
    providers_extended_wire(
        "fal_ai",
        "ProviderType::FalAI dispatches to native image-generation endpoints when providers-extended is enabled",
    ),
    providers_extended_wire(
        "gemini",
        "ProviderType::Gemini dispatches to native Google AI Studio Gemini auth when providers-extended is enabled",
    ),
    providers_extended_wire(
        "github_copilot",
        "ProviderType::GitHubCopilot dispatches to native GitHub Copilot auth when providers-extended is enabled",
    ),
    internal("macros", "provider macro infrastructure"),
    wire("mistral", "native Provider enum variant"),
    providers_extended_wire(
        "ollama",
        "ProviderType::Ollama dispatches to the native Ollama protocol when providers-extended is enabled",
    ),
    wire("openai", "native Provider enum variant"),
    wire("openai_like", "shared OpenAI-compatible runtime provider"),
    wire("oci", "typed OCI compatible and native retrieval runtime"),
    internal("registry", "provider catalog and lifecycle infrastructure"),
    providers_extended_wire(
        "replicate",
        "ProviderType::Replicate dispatches to native prediction lifecycle paths when providers-extended is enabled",
    ),
    wire("sagemaker", "typed SageMaker InvokeEndpoint runtime"),
    wire("snowflake", "typed Snowflake Cortex runtime"),
    providers_extended_wire(
        "stability",
        "ProviderType::Stability dispatches to native v2beta image endpoints",
    ),
    internal("thinking", "shared thinking/reasoning support"),
    provider_extra_wire(
        "vertex_ai",
        "ProviderType::VertexAI dispatches to native Vertex AI auth when providers-extra is enabled",
    ),
    wire("watsonx", "typed watsonx native inference runtime"),
    wire(
        "voyage",
        "ProviderType::Voyage dispatches to native embedding and rerank endpoints",
    ),
];

pub static PROVIDER_ORPHAN_BASELINE: &[ProviderOrphanBaselineEntry] = &[];

pub fn provider_module_lifecycle() -> &'static [ProviderModuleLifecycleEntry] {
    PROVIDER_MODULE_LIFECYCLE
}

pub fn provider_orphan_baseline() -> &'static [ProviderOrphanBaselineEntry] {
    PROVIDER_ORPHAN_BASELINE
}

const fn wire(module_name: &'static str, reason: &'static str) -> ProviderModuleLifecycleEntry {
    entry(module_name, ProviderModuleLifecycle::Wire, reason)
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
#[path = "lifecycle_tests.rs"]
mod tests;
