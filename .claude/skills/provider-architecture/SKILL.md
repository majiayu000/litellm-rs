---
name: provider-architecture
description: LiteLLM-RS provider system in two tiers - data-driven OpenAI-compatible catalog entries auto-routed through OpenAILikeProvider, plus code-based provider modules implementing the LLMProvider trait behind the closed Provider enum. Covers unified ProviderError handling, connection pooling, capabilities, and model metadata. Use when adding a new provider, editing registry/catalog.rs, or implementing/debugging the LLMProvider trait.
---

# Provider Architecture Guide

## Architecture Overview

Providers come in two tiers behind one implementation trait:

**Tier 1 — Catalog-only (zero code).** An OpenAI-compatible endpoint that differs only in
base URL, auth env var, and advertised capabilities/models. It has no Rust module: one
static entry in `src/core/providers/registry/catalog.rs` fully describes it, and the
factory builds an `OpenAILikeProvider` from that data at runtime. This is the primary
path for new integrations.

**Tier 2 — Code-based.** A provider needing custom request/response transformation,
custom auth signing, non-standard streaming, or rich model metadata lives in
`src/core/providers/<name>/`, implements `LLMProvider`, and is wired into routing.

Routing does **not** use trait objects. Router deployments store the closed `Provider`
enum (`src/core/providers/mod.rs`), which dispatches to concrete provider structs.
`LLMProvider` (`src/core/traits/provider/llm_provider/trait_definition.rs`) is the
interface every variant implements — implementing the trait alone does not make a
provider routeable; enum variant, dispatch arm, and factory wiring are crate-level
changes. Trait objects appear only at the edges: the error mapper
(`Box<dyn ErrorMapper<ProviderError>>`) and the streaming return type
(`Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>`).

### Enumerating Current Providers

Do not rely on memorized counts — they change often. Count from source:

```bash
# Tier 1: one def_chat()/def_local_chat() call per entry
# (each count includes the helper fn definition itself, so subtract 1)
grep -c 'def_chat(' src/core/providers/registry/catalog.rs
grep -c 'def_local_chat(' src/core/providers/registry/catalog.rs

# Tier 2: code-based provider modules (base/factory/macros/registry are infrastructure)
ls -d src/core/providers/*/ | grep -vE '/(base|factory|macros|registry)/'
```

---

## Adding a Tier 1 Provider (Primary Path)

Two edits, nothing else:

```rust
// src/core/providers/registry/catalog.rs
def_chat(
    "myprovider",
    "My Provider",
    "https://api.myprovider.com/v1",
    "MYPROVIDER_API_KEY",
),
```

```rust
// src/core/providers/mod.rs — annotation comment alongside the other Tier 1 notes
// myprovider: Tier 1 -> registry/catalog.rs
```

At runtime `create_provider` (`src/core/providers/factory/mod.rs`) matches the selector
against the catalog, builds an `OpenAILikeConfig` via
`ProviderDefinition::to_openai_like_config`, and constructs
`openai_like::OpenAILikeProvider::new_for_catalog(oai_config, def.capabilities)` into the
`Provider::OpenAILike` variant.

Keyless local servers use `def_local_chat` (`AuthType::None`, `skip_api_key = true`):

```rust
def_local_chat("myrunner", "My Runner", "http://localhost:1234/v1"),
```

Each entry is a `ProviderDefinition` (`src/core/providers/registry/definition.rs`).
Defaults from `def_chat` can be overridden with struct-update syntax:

- `alternate_auth_env_vars` — env vars checked after `auth_env_var` (see `together`)
- `model_prefix: Some("xai/")` — selector/model prefix stripping (see `xai`)
- custom `capabilities` profile replacing the default
  `OPENAI_LIKE_CATALOG_CAPABILITIES` (`ChatCompletion`, `ChatCompletionStream`,
  `ToolCalling`, `FunctionCalling`)
- name aliases resolve via `canonical_catalog_name()` in `catalog.rs` (e.g.
  `"zhipuai"` -> `"zhipu"`)

Registry API (`src/core/providers/registry/`, re-exported in `registry/mod.rs`):
`is_tier1_provider(name)`, `get_definition(name)`, `canonical_catalog_name(name)`,
`PROVIDER_CATALOG`.

---

## Core Trait Definition: LLMProvider

There are **no associated types**. Every fallible method returns the unified
`ProviderError` directly.

```rust
// src/core/traits/provider/llm_provider/trait_definition.rs
pub trait LLMProvider: Send + Sync + Debug + 'static {
    // ===== Required =====
    fn name(&self) -> &str;
    fn capabilities(&self) -> &'static [ProviderCapability];
    fn models(&self) -> &[ModelInfo];

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str];
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError>;
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError>;
    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError>;
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>>;
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError>;
    async fn health_check(&self) -> HealthStatus;
    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError>;

    // ===== Provided (default = not_supported / trivial) =====
    fn error_provider_name(&self) -> &'static str { "provider" }
    fn supports_capability(&self, capability: &ProviderCapability) -> bool {
        self.capabilities().contains(capability)
    }
    fn supports_model(&self, model: &str) -> bool {
        self.models().iter().any(|m| m.id == model)
    }
    // supports_tools / supports_streaming / supports_embeddings /
    // supports_image_generation delegate to supports_capability();
    // supports_vision() currently returns false.
    async fn chat_completion_stream(&self, request: ChatRequest, context: RequestContext)
        -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>;
    async fn embeddings(&self, request: EmbeddingRequest, context: RequestContext)
        -> Result<EmbeddingResponse, ProviderError>;
    async fn image_generation(&self, request: ImageGenerationRequest, context: RequestContext)
        -> Result<ImageGenerationResponse, ProviderError>;
    async fn audio_transcription(&self, request: TranscriptionRequest, context: RequestContext)
        -> Result<TranscriptionResponse, ProviderError>;
    async fn audio_translation(&self, request: TranslationRequest, context: RequestContext)
        -> Result<TranslationResponse, ProviderError>;
    async fn text_to_speech(&self, request: SpeechRequest, context: RequestContext)
        -> Result<SpeechResponse, ProviderError>;
    async fn get_average_latency(&self) -> Result<std::time::Duration, ProviderError>; // default 100ms
    async fn get_success_rate(&self) -> Result<f32, ProviderError>;                   // default 0.99
    async fn estimate_tokens(&self, text: &str) -> Result<u32, ProviderError>;        // len()/4
}
```

Optional dispatch methods (streaming, embeddings, images, audio) must still be gated by
the matching `ProviderCapability` — route selection checks `supports_capability()`
before calling them.

---

## Tier 2: Code-Based Provider Pattern

### Directory Structure

Real examples: `src/core/providers/cloudflare/` (small) and `src/core/providers/openai_like/`.

```
src/core/providers/my_provider/
├── mod.rs           # Module exports, LLMProvider impl may live here too
├── config.rs        # Config struct (often via define_provider_config!)
├── provider.rs      # Provider struct + LLMProvider impl
├── model_info.rs    # Static model metadata
├── streaming.rs     # SSE parsing (optional)
└── error.rs         # Error helpers/mappers (optional; legacy name = ProviderError alias)
```

### Configuration

`BaseConfig` (`src/core/providers/base/config.rs`) carries `api_key`, `api_base`,
`endpoint_access`, `timeout` (secs), `max_retries`, `headers`, `organization`,
`api_version`, with env fallbacks (`{PROVIDER}_API_KEY`, `{PROVIDER}_API_BASE`, ...)
via `from_env(provider)` / `for_provider(provider)`.

Implement the `ProviderConfig` trait (`src/core/traits/provider/config.rs`) — note the
accessor names differ from `BaseConfig`'s helpers:

```rust
pub trait ProviderConfig: Send + Sync + Clone + Debug + 'static {
    fn validate(&self) -> Result<(), String>;
    fn api_key(&self) -> Option<&str>;
    fn api_base(&self) -> Option<&str>;
    fn timeout(&self) -> std::time::Duration;
    fn max_retries(&self) -> u32;
    fn endpoint_access(&self) -> ProviderEndpointAccess { ProviderEndpointAccess::PublicOnly }
    fn use_ssrf_safe_client(&self) -> bool { false }
    // validate_standard(provider_name): shared key/timeout/retries checks
}
```

The `define_provider_config!` macro (exported from `base/config.rs`) generates the
struct, builders (`with_api_key`, `with_base_url`, `with_timeout`), `from_env()`,
`get_api_key()`/`get_api_base()`, and the `ProviderConfig` impl in one call.

### Provider Implementation

Modeled on `src/core/providers/cloudflare/provider.rs`:

```rust
use std::sync::Arc;
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::ProviderError;
use crate::core::traits::error_mapper::DefaultErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

pub struct MyProvider {
    config: MyProviderConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl MyProvider {
    pub fn new(config: MyProviderConfig) -> Result<Self, ProviderError> {
        config.validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;
        Ok(Self {
            config,
            pool_manager: Arc::new(GlobalPoolManager::new()?),
            models: load_models(),
        })
    }
}

impl LLMProvider for MyProvider {
    fn name(&self) -> &'static str { PROVIDER_NAME }
    fn error_provider_name(&self) -> &'static str { PROVIDER_NAME }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        let api_key = self.config.api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key required"))?;
        let mut headers = vec![header("Authorization", format!("Bearer {api_key}"))];
        headers.push(header("Content-Type", "application/json".to_string()));

        // url/model/request_id derived from config + request (elided)
        let body = self.transform_request(request, context).await?;
        let response = self.pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(self.get_error_mapper().map_http_error(status, &body_text));
        }
        let raw = response.bytes().await?;
        self.transform_response(&raw, &model, &request_id).await
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }
    // ... remaining required methods
}
```

### Registration (routing) Requirements

Adding a Tier 2 provider means crate-level wiring, in this order of authority:

1. `src/core/providers/<name>/` directory + `pub mod <name>;` in
   `src/core/providers/mod.rs` (feature-gate as needed, e.g. `providers-extra`).
2. A variant in the closed `Provider` enum (`src/core/providers/mod.rs`).
3. Dispatch arms in the `dispatch_provider!` expansions in the same file.
4. Factory support: a builder branch under `src/core/providers/factory/`
   (`builder.rs`, `registry.rs` dispatch table).

---

## Connection Pooling

All providers share process-wide reqwest clients defined in
`src/core/providers/base/connection_pool.rs` (there is no `pool.rs`):

```rust
pub struct PoolConfig;
impl PoolConfig {
    pub const TIMEOUT_SECS: u64 = 600;
    pub const POOL_SIZE: usize = 80;      // pool_max_idle_per_host
    pub const KEEPALIVE_SECS: u64 = 90;   // pool_idle_timeout
}
```

```rust
// GlobalPoolManager: new() / new_for_provider(provider, BaseConfig) / shared()
pub async fn execute_request(
    &self,
    url: &str,
    method: HttpMethod,               // GET | POST | PUT | DELETE
    headers: Vec<HeaderPair>,         // (Cow<'static, str>, Cow<'static, str>)
    body: Option<serde_json::Value>,  // serialized as JSON; not generic
) -> Result<reqwest::Response, ProviderError>;

pub async fn execute_streaming_request(
    &self,
    url: &str,
    headers: Vec<HeaderPair>,
    body: serde_json::Value,
    legacy_provider: &'static str,
) -> Result<reqwest::Response, ProviderError>;
```

Build header pairs zero-copy where possible: `header(key, value)` (static key, owned
value), `header_static(key, value)`, `header_owned(key, value)`. Streaming callers get a
separate client without a total-body timeout via `streaming_unbounded_client()`; header
phase bounded by `STREAMING_HEADER_TIMEOUT_SECS`, error bodies by
`read_streaming_error_body` (10s / 64KiB caps).

---

## Model Information

`ModelInfo` (`src/core/types/model.rs`) is a plain serializable struct with `Default`:

```rust
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub max_context_length: u32,
    pub max_output_length: Option<u32>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_multimodal: bool,
    pub input_cost_per_1k_tokens: Option<f64>,
    pub output_cost_per_1k_tokens: Option<f64>,
    pub currency: String,
    pub capabilities: Vec<ProviderCapability>,
    pub created_at: Option<SystemTime>,
    pub updated_at: Option<SystemTime>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

Costs are per **1K tokens** (catalog pricing constants in `registry/catalog.rs` are per
million and divided by 1_000 when converted). Return `&self.models` from
`LLMProvider::models()`; `supports_model()` scans that slice.

---

## Provider Capabilities

`ProviderCapability` (`src/core/types/model.rs`) — the full current variant list:

```rust
pub enum ProviderCapability {
    ChatCompletion,
    ChatCompletionStream,
    Embeddings,
    ImageGeneration,
    ImageEdit,
    ImageVariation,
    AudioTranscription,
    AudioTranslation,
    TextToSpeech,
    Moderation,
    Rerank,
    ToolCalling,
    FunctionCalling,
    CodeExecution,
    FileUpload,
    FineTuning,
    BatchProcessing,
    RealtimeApi,
    GeminiGenerateContent,
}
```

Return a `static` slice (a temporary array literal cannot back `&'static [...]`):

```rust
const MY_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

fn capabilities(&self) -> &'static [ProviderCapability] {
    MY_CAPABILITIES
}
```

---

## Unified Error Handling

`ProviderError` (`src/core/providers/unified_provider_error.rs`, re-exported as
`crate::core::providers::ProviderError` and `unified_provider::ProviderError`) is the
single error type across the trait. Construct with factory methods
(`src/core/providers/unified_provider_methods.rs`) rather than struct literals — some
variants carry extra optional fields (e.g. `RateLimit` rpm/tpm limits):

```rust
ProviderError::authentication(provider, msg)
ProviderError::rate_limit(provider, retry_after: Option<u64>)
ProviderError::rate_limit_with_retry(provider, msg, retry_after)
ProviderError::model_not_found(provider, model)
ProviderError::invalid_request(provider, msg)
ProviderError::network(provider, msg)
ProviderError::timeout(provider, msg)
ProviderError::api_error(provider, status: u16, msg)
ProviderError::provider_unavailable(provider, msg)
ProviderError::not_supported(provider, feature)
ProviderError::configuration(provider, msg)
ProviderError::serialization(provider, msg)
```

`ErrorMapper<E>` (`src/core/traits/error_mapper/trait_def.rs`) converts HTTP statuses
and JSON error bodies into errors: required `map_http_error(u16, &str)`; defaulted
`map_json_error`, `map_network_error`, `map_parsing_error`, `map_timeout_error`.
Ready-made mappers:

- `GenericErrorMapper` — `core::traits::error_mapper::types`, aliased
  `DefaultErrorMapper` at `core::traits::error_mapper` (what most providers return from
  `get_error_mapper()`)
- `OpenAIErrorMapper`, `AnthropicErrorMapper` — `core::traits::error_mapper::implementations`

Legacy per-provider error enums were removed; surviving names like `AnthropicError` or
`GeminiError` are `pub type ... = ProviderError` aliases
(see [reference/migration-from-legacy-errors.md](reference/migration-from-legacy-errors.md)).

---

## References

- [reference/best-practices-and-checklist.md](reference/best-practices-and-checklist.md) — error factory conventions, coding practices, and new-provider checklist (Tier 1 and Tier 2)
- [reference/migration-from-legacy-errors.md](reference/migration-from-legacy-errors.md) — how legacy per-provider error types map onto unified ProviderError
