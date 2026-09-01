//! Legacy adapter availability matrix.
//!
//! This matrix records selector-based adapters from the legacy provider/config
//! paths. Canonical runtime entry points do not consult it: they derive support
//! from the selected deployment's `ProviderCapability`.

use super::{canonical_catalog_name, entry_for_name, get_definition};

/// Compatibility entry points covered by the legacy adapter matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAdapterSurface {
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

/// Availability state for one provider/legacy-adapter pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAdapterAvailability {
    Supported,
    Passthrough,
    FeatureGated(&'static str),
    Unsupported,
}

impl LegacyAdapterAvailability {
    /// True when the legacy adapter is available in the current binary.
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

/// One documented legacy adapter row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderLegacyAdapterSupport {
    pub selector: &'static str,
    pub http_chat: LegacyAdapterAvailability,
    pub http_chat_stream: LegacyAdapterAvailability,
    pub http_embeddings: LegacyAdapterAvailability,
    pub http_rerank: LegacyAdapterAvailability,
    pub http_image_generation: LegacyAdapterAvailability,
    pub sdk_chat: LegacyAdapterAvailability,
    pub sdk_chat_stream: LegacyAdapterAvailability,
    pub sdk_embeddings: LegacyAdapterAvailability,
    pub completion_chat: LegacyAdapterAvailability,
    pub completion_chat_stream: LegacyAdapterAvailability,
    pub notes: &'static str,
}

impl ProviderLegacyAdapterSupport {
    pub fn availability_for(self, surface: LegacyAdapterSurface) -> LegacyAdapterAvailability {
        match surface {
            LegacyAdapterSurface::HttpChat => self.http_chat,
            LegacyAdapterSurface::HttpChatStream => self.http_chat_stream,
            LegacyAdapterSurface::HttpEmbeddings => self.http_embeddings,
            LegacyAdapterSurface::HttpRerank => self.http_rerank,
            LegacyAdapterSurface::HttpImageGeneration => self.http_image_generation,
            LegacyAdapterSurface::SdkChat => self.sdk_chat,
            LegacyAdapterSurface::SdkChatStream => self.sdk_chat_stream,
            LegacyAdapterSurface::SdkEmbeddings => self.sdk_embeddings,
            LegacyAdapterSurface::CompletionChat => self.completion_chat,
            LegacyAdapterSurface::CompletionChatStream => self.completion_chat_stream,
        }
    }
}

const S: LegacyAdapterAvailability = LegacyAdapterAvailability::Supported;
const P: LegacyAdapterAvailability = LegacyAdapterAvailability::Passthrough;
const U: LegacyAdapterAvailability = LegacyAdapterAvailability::Unsupported;
const EXTRA: LegacyAdapterAvailability = LegacyAdapterAvailability::FeatureGated("providers-extra");
const EXTENDED: LegacyAdapterAvailability =
    LegacyAdapterAvailability::FeatureGated("providers-extended");

const CATALOG_HTTP_SUPPORT: ProviderLegacyAdapterSupport = row(
    "catalog_openai_like",
    [P, P, U, U, U, U, U, U, U],
    "Generic Tier 1 catalog providers route through OpenAILike for HTTP chat/stream only.",
);

/// Explicit rows for non-catalog providers and catalog providers with
/// additional legacy completion adapters.
pub static LEGACY_ADAPTER_MATRIX: &[ProviderLegacyAdapterSupport] = &[
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
        "Core Bedrock runtime supports chat, stream, and embeddings; legacy SDK and completion adapters are absent.",
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
        "Native HTTP provider; legacy SDK/completion adapters are not implemented.",
    ),
    row(
        "cloudflare",
        [S, U, U, U, U, U, U, U, U],
        "Workers AI chat only.",
    ),
    row(
        "deepgram",
        [U, U, U, U, U, U, U, U, U],
        "Native audio routes; this matrix covers chat, embeddings, and images.",
    ),
    row(
        "elevenlabs",
        [U, U, U, U, U, U, U, U, U],
        "Native audio routes; this matrix covers chat, embeddings, and images.",
    ),
    rerank_row(
        "cohere",
        [EXTENDED, EXTENDED, EXTENDED, U, U, U, U, U, U],
        EXTENDED,
        "Native provider is behind providers-extended.",
    ),
    rerank_row(
        "voyage",
        [U, U, S, U, U, U, U, U, U],
        S,
        "Native Voyage embeddings plus the shared HTTP rerank route.",
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
        "stability",
        [U, U, U, EXTENDED, U, U, U, U, U],
        "Native image generation/edit provider behind providers-extended.",
    ),
    row(
        "black_forest_labs",
        [U, U, U, EXTENDED, U, U, U, U, U],
        "Native asynchronous image generation/edit provider behind providers-extended.",
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

pub fn legacy_adapter_matrix() -> &'static [ProviderLegacyAdapterSupport] {
    LEGACY_ADAPTER_MATRIX
}

pub fn legacy_adapter_availability(
    provider_name: &str,
    surface: LegacyAdapterSurface,
) -> Option<LegacyAdapterAvailability> {
    let selector = canonical_selector(provider_name);

    LEGACY_ADAPTER_MATRIX
        .iter()
        .find(|entry| entry.selector == selector)
        .map(|entry| entry.availability_for(surface))
        .or_else(|| {
            get_definition(&selector).map(|_| CATALOG_HTTP_SUPPORT.availability_for(surface))
        })
}

pub fn supports_legacy_adapter(provider_name: &str, surface: LegacyAdapterSurface) -> bool {
    legacy_adapter_availability(provider_name, surface)
        .map(LegacyAdapterAvailability::is_available_in_current_build)
        .unwrap_or(false)
}

pub fn selector_has_legacy_adapter_entry(provider_name: &str) -> bool {
    let selector = canonical_selector(provider_name);
    LEGACY_ADAPTER_MATRIX
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
    support: [LegacyAdapterAvailability; 9],
    notes: &'static str,
) -> ProviderLegacyAdapterSupport {
    ProviderLegacyAdapterSupport {
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
    support: [LegacyAdapterAvailability; 9],
    http_rerank: LegacyAdapterAvailability,
    notes: &'static str,
) -> ProviderLegacyAdapterSupport {
    let mut result = row(selector, support, notes);
    result.http_rerank = http_rerank;
    result
}
