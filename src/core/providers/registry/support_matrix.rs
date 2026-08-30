//! Cross-surface provider support matrix.
//!
//! This matrix answers which provider selectors are usable through each public
//! routing surface. Runtime capability checks still use `ProviderCapability`;
//! this file keeps SDK and `completion()` support from drifting away from the
//! core provider registry.

use super::{canonical_catalog_name, entry_for_name, get_definition};

/// Public route surfaces covered by the support matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRouteSurface {
    HttpChat,
    HttpChatStream,
    HttpEmbeddings,
    HttpRerank,
    HttpImageGeneration,
    SdkChat,
    SdkChatStream,
    SdkEmbeddings,
    CompletionChat,
    CompletionChatStream,
}

/// Support state for one provider/surface pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceSupport {
    Supported,
    Passthrough,
    FeatureGated(&'static str),
    Unsupported,
}

impl SurfaceSupport {
    /// True when the surface should be selected in the current binary.
    pub fn is_available_in_current_build(self) -> bool {
        match self {
            Self::Supported | Self::Passthrough => true,
            Self::FeatureGated("providers-extra") => cfg!(feature = "providers-extra"),
            Self::FeatureGated("providers-extended") => cfg!(feature = "providers-extended"),
            Self::FeatureGated(_) | Self::Unsupported => false,
        }
    }

    pub fn markdown_cell(self) -> &'static str {
        match self {
            Self::Supported => "yes",
            Self::Passthrough => "passthrough",
            Self::FeatureGated(feature) => feature,
            Self::Unsupported => "-",
        }
    }
}

/// One documented support row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderSurfaceSupport {
    pub selector: &'static str,
    pub http_chat: SurfaceSupport,
    pub http_chat_stream: SurfaceSupport,
    pub http_embeddings: SurfaceSupport,
    pub http_rerank: SurfaceSupport,
    pub http_image_generation: SurfaceSupport,
    pub sdk_chat: SurfaceSupport,
    pub sdk_chat_stream: SurfaceSupport,
    pub sdk_embeddings: SurfaceSupport,
    pub completion_chat: SurfaceSupport,
    pub completion_chat_stream: SurfaceSupport,
    pub notes: &'static str,
}

impl ProviderSurfaceSupport {
    pub fn support_for(self, surface: ProviderRouteSurface) -> SurfaceSupport {
        match surface {
            ProviderRouteSurface::HttpChat => self.http_chat,
            ProviderRouteSurface::HttpChatStream => self.http_chat_stream,
            ProviderRouteSurface::HttpEmbeddings => self.http_embeddings,
            ProviderRouteSurface::HttpRerank => self.http_rerank,
            ProviderRouteSurface::HttpImageGeneration => self.http_image_generation,
            ProviderRouteSurface::SdkChat => self.sdk_chat,
            ProviderRouteSurface::SdkChatStream => self.sdk_chat_stream,
            ProviderRouteSurface::SdkEmbeddings => self.sdk_embeddings,
            ProviderRouteSurface::CompletionChat => self.completion_chat,
            ProviderRouteSurface::CompletionChatStream => self.completion_chat_stream,
        }
    }
}

const S: SurfaceSupport = SurfaceSupport::Supported;
const P: SurfaceSupport = SurfaceSupport::Passthrough;
const U: SurfaceSupport = SurfaceSupport::Unsupported;
const EXTRA: SurfaceSupport = SurfaceSupport::FeatureGated("providers-extra");
const EXTENDED: SurfaceSupport = SurfaceSupport::FeatureGated("providers-extended");

const CATALOG_HTTP_SUPPORT: ProviderSurfaceSupport = row(
    "catalog_openai_like",
    [P, P, U, U, U, U, U, U, U],
    "Generic Tier 1 catalog providers route through OpenAILike for HTTP chat/stream only.",
);

/// Explicit rows for non-catalog providers and catalog providers with
/// additional completion() routing.
pub static PROVIDER_SURFACE_MATRIX: &[ProviderSurfaceSupport] = &[
    row(
        "openai",
        [S, S, S, S, S, S, S, S, S],
        "Reference provider across HTTP, SDK, and completion().",
    ),
    row(
        "anthropic",
        [S, S, U, U, S, S, U, S, S],
        "Native Anthropic chat/stream adapter.",
    ),
    row(
        "azure",
        [P, P, EXTRA, EXTRA, U, U, S, P, P],
        "SDK currently exposes Azure embeddings only.",
    ),
    row(
        "azure_ai",
        [P, P, EXTRA, EXTRA, U, U, U, EXTRA, EXTRA],
        "completion() supports azure_ai/ and azure-ai/ dynamic routes with providers-extra.",
    ),
    row(
        "bedrock",
        [S, S, S, U, U, U, U, U, U],
        "Core Bedrock runtime supports chat, stream, and embeddings; public completion() routing is not registered.",
    ),
    row(
        "databricks",
        [P, P, U, U, U, U, U, U, U],
        "Databricks Model Serving OpenAI-compatible chat and SSE.",
    ),
    row(
        "snowflake",
        [P, P, U, U, U, U, U, U, U],
        "Snowflake Cortex OpenAI-compatible chat and SSE.",
    ),
    rerank_row(
        "oci",
        [P, P, S, U, U, U, U, U, U],
        S,
        "OCI compatible mode provides chat/SSE; IAM native mode provides embeddings and rerank.",
    ),
    rerank_row(
        "watsonx",
        [S, U, S, U, U, U, U, U, U],
        S,
        "watsonx native chat, embeddings, and rerank; streaming is not implemented.",
    ),
    row(
        "sagemaker",
        [S, U, U, U, U, U, U, U, U],
        "SageMaker InvokeEndpoint chat requires an explicit supported payload transformer.",
    ),
    row(
        "mistral",
        [S, S, P, U, U, U, U, U, U],
        "Native HTTP provider; SDK/completion adapters are not implemented.",
    ),
    row(
        "cloudflare",
        [S, U, U, U, U, U, U, U, U],
        "Workers AI chat only.",
    ),
    rerank_row(
        "cohere",
        [EXTENDED, EXTENDED, EXTENDED, U, U, U, U, U, U],
        EXTENDED,
        "Native provider is behind providers-extended.",
    ),
    row(
        "vertex_ai",
        [EXTRA, EXTRA, EXTRA, EXTRA, U, U, U, U, U],
        "Native provider is behind providers-extra; SDK GoogleVertex is not implemented.",
    ),
    row(
        "gemini",
        [EXTENDED, EXTENDED, U, U, U, U, U, U, U],
        "Native provider is behind providers-extended; SDK Google chat is not implemented.",
    ),
    row(
        "github_copilot",
        [EXTENDED, EXTENDED, U, U, U, U, U, U, U],
        "Native provider is behind providers-extended; SDK adapter is not implemented.",
    ),
    row(
        "google",
        [U, U, U, U, U, U, U, U, U],
        "SDK Google selector is intentionally unsupported until a real adapter exists.",
    ),
    row(
        "fal_ai",
        [U, U, U, EXTENDED, U, U, U, U, U],
        "Image-generation provider behind providers-extended.",
    ),
    row(
        "replicate",
        [EXTENDED, EXTENDED, U, EXTENDED, U, U, U, U, U],
        "Prediction lifecycle provider behind providers-extended.",
    ),
    row(
        "pydantic_ai",
        [U, U, U, U, U, U, U, U, U],
        "ProviderType registry entry is not currently dispatchable.",
    ),
    row(
        "openai_compatible",
        [P, P, U, U, U, U, U, S, S],
        "HTTP and completion() OpenAI-compatible passthrough.",
    ),
    row(
        "sdk_custom",
        [U, U, U, U, U, U, S, U, U],
        "SDK custom providers support embeddings when a base_url is configured.",
    ),
    row(
        "ollama",
        [EXTENDED, EXTENDED, EXTENDED, U, U, S, U, U, U],
        "Native HTTP chat, NDJSON streaming, and embeddings require providers-extended; SDK streaming keeps its existing OpenAI-compatible parser.",
    ),
    row(
        "openrouter",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
    row(
        "deepseek",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
    row(
        "moonshot",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
    row(
        "minimax",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
    row(
        "zhipu",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with zhipu/ and glm/ completion() routes.",
    ),
    row(
        "zai",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with zai/ completion() dynamic route.",
    ),
    row(
        "together",
        [P, P, U, U, U, U, U, S, S],
        "Legacy Tier 1 catalog selector with together/ completion() dynamic route.",
    ),
    row(
        "together_ai",
        [P, P, U, U, U, U, U, S, S],
        "LiteLLM Tier 1 catalog selector with together_ai/ completion() dynamic route.",
    ),
    row(
        "fireworks",
        [P, P, U, U, U, U, U, S, S],
        "Legacy Tier 1 catalog selector with fireworks/ completion() dynamic route.",
    ),
    row(
        "fireworks_ai",
        [P, P, U, U, U, U, U, S, S],
        "LiteLLM Tier 1 catalog selector with fireworks_ai/ completion() dynamic route.",
    ),
    row(
        "aiml",
        [P, P, U, U, U, U, U, S, S],
        "LiteLLM Tier 1 catalog selector with aiml/ completion() dynamic route.",
    ),
    row(
        "aiml_api",
        [P, P, U, U, U, U, U, S, S],
        "Legacy Tier 1 catalog selector with aiml_api/ completion() dynamic route.",
    ),
    row(
        "groq",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
    row(
        "xiaomi_mimo",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with xiaomi_mimo/ and mimo/ completion() routes.",
    ),
    row(
        "xai",
        [P, P, U, U, U, U, U, S, S],
        "Tier 1 catalog provider with completion() dynamic route.",
    ),
];

pub fn provider_surface_matrix() -> &'static [ProviderSurfaceSupport] {
    PROVIDER_SURFACE_MATRIX
}

pub fn support_state_for_surface(
    provider_name: &str,
    surface: ProviderRouteSurface,
) -> Option<SurfaceSupport> {
    let selector = canonical_selector(provider_name);

    PROVIDER_SURFACE_MATRIX
        .iter()
        .find(|entry| entry.selector == selector)
        .map(|entry| entry.support_for(surface))
        .or_else(|| get_definition(&selector).map(|_| CATALOG_HTTP_SUPPORT.support_for(surface)))
}

pub fn supports_provider_surface(provider_name: &str, surface: ProviderRouteSurface) -> bool {
    support_state_for_surface(provider_name, surface)
        .map(SurfaceSupport::is_available_in_current_build)
        .unwrap_or(false)
}

pub fn selector_has_matrix_entry(provider_name: &str) -> bool {
    let selector = canonical_selector(provider_name);
    PROVIDER_SURFACE_MATRIX
        .iter()
        .any(|entry| entry.selector == selector)
        || get_definition(&selector).is_some()
}

pub fn canonical_selector(provider_name: &str) -> String {
    let lowered = provider_name.trim().to_ascii_lowercase();
    if let Some(entry) = entry_for_name(&lowered) {
        return entry.canonical_name.to_string();
    }
    if let Some(canonical) = canonical_catalog_name(&lowered) {
        return canonical.to_string();
    }

    let normalized = lowered.replace('-', "_");

    match normalized.as_str() {
        "aws_bedrock" => "bedrock".to_string(),
        "google_vertex" => "vertex_ai".to_string(),
        "openai_like" => "openai_compatible".to_string(),
        "custom" => "sdk_custom".to_string(),
        "huggingface" | "hugging_face" => "huggingface".to_string(),
        other => entry_for_name(other)
            .map(|entry| entry.canonical_name.to_string())
            .or_else(|| canonical_catalog_name(other).map(str::to_string))
            .unwrap_or_else(|| other.to_string()),
    }
}

const fn row(
    selector: &'static str,
    support: [SurfaceSupport; 9],
    notes: &'static str,
) -> ProviderSurfaceSupport {
    ProviderSurfaceSupport {
        selector,
        http_chat: support[0],
        http_chat_stream: support[1],
        http_embeddings: support[2],
        http_rerank: U,
        http_image_generation: support[3],
        sdk_chat: support[4],
        sdk_chat_stream: support[5],
        sdk_embeddings: support[6],
        completion_chat: support[7],
        completion_chat_stream: support[8],
        notes,
    }
}

const fn rerank_row(
    selector: &'static str,
    support: [SurfaceSupport; 9],
    http_rerank: SurfaceSupport,
    notes: &'static str,
) -> ProviderSurfaceSupport {
    let mut result = row(selector, support, notes);
    result.http_rerank = http_rerank;
    result
}
