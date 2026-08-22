---
name: provider-architecture
description: LiteLLM-RS Provider Architecture Guide. Covers 66+ provider integration, trait object design, unified error handling, connection pooling, and LLMProvider trait implementation patterns. Use when adding a new provider or implementing/debugging the LLMProvider trait.
---

# Provider Architecture Guide

## Architecture Overview

LiteLLM-RS uses a **Trait Object + Unified Error** architecture optimized for 66+ LLM providers. This design prioritizes maintainability, compile time, and binary size over micro-optimizations.

### Why Trait Objects Over Enum Dispatch

| Metric | Enum Dispatch | Trait Object | Winner |
|--------|---------------|--------------|--------|
| Method call latency | ~480ns | ~5,900ns | Enum |
| Binary size (66 providers) | ~50MB | ~10MB | **Trait Object** |
| Compile time | ~10 min | ~2 min | **Trait Object** |
| Runtime extensibility | No | Yes | **Trait Object** |
| Code maintainability | Poor | Excellent | **Trait Object** |

**Key insight**: 5μs dispatch overhead is negligible compared to 500ms-5000ms API latency (0.001%-0.01%).

---

## Core Trait Definition

```rust
// src/core/traits/provider/llm_provider/trait_definition.rs

#[async_trait]
pub trait LLMProvider: Send + Sync {
    type Config: ProviderConfig;
    type Error: From<ProviderError>;
    type ErrorMapper: ErrorMapper<Error = Self::Error>;

    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [ProviderCapability];
    fn models(&self) -> &[ModelInfo];

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str];

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error>;

    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, Self::Error>;

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, Self::Error>;

    fn get_error_mapper(&self) -> Self::ErrorMapper;

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error>;

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>;

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, Self::Error>;

    async fn health_check(&self) -> HealthStatus;

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error>;
}
```

---

## Provider Implementation Pattern

### Directory Structure

```
src/core/providers/my_provider/
├── mod.rs           # Module exports
├── config.rs        # ProviderConfig implementation
├── provider.rs      # LLMProvider implementation
├── model_info.rs    # Model definitions and capabilities
├── streaming.rs     # SSE stream parsing (optional)
└── error.rs         # Error mapper (if not using unified)
```

### Configuration Implementation

```rust
// config.rs
use crate::core::providers::base::BaseProviderConfig;
use crate::core::traits::config::ProviderConfig;

#[derive(Debug, Clone)]
pub struct MyProviderConfig {
    pub base: BaseProviderConfig,
    pub custom_option: Option<String>,
}

impl Default for MyProviderConfig {
    fn default() -> Self {
        Self {
            base: BaseProviderConfig {
                api_key: std::env::var("MY_PROVIDER_API_KEY").ok(),
                api_base: Some("https://api.myprovider.com/v1".to_string()),
                ..Default::default()
            },
            custom_option: None,
        }
    }
}

impl ProviderConfig for MyProviderConfig {
    fn validate(&self) -> Result<(), String> {
        if self.base.api_key.is_none() {
            return Err("MY_PROVIDER_API_KEY is required".to_string());
        }
        Ok(())
    }

    fn get_api_key(&self) -> Option<String> {
        self.base.api_key.clone()
    }

    fn get_api_base(&self) -> String {
        self.base.api_base.clone()
            .unwrap_or_else(|| "https://api.myprovider.com/v1".to_string())
    }
}
```

### Provider Implementation (Unified Error)

```rust
// provider.rs
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

const PROVIDER_NAME: &str = "my_provider";

#[derive(Debug, Clone)]
pub struct MyProvider {
    config: MyProviderConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl MyProvider {
    pub fn new(config: MyProviderConfig) -> Result<Self, ProviderError> {
        config.validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?
        );

        Ok(Self { config, pool_manager, models: get_models() })
    }

    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(MyProviderConfig::default())
    }
}

#[async_trait]
impl LLMProvider for MyProvider {
    type Config = MyProviderConfig;
    type Error = ProviderError;  // ← Unified error type
    type ErrorMapper = crate::core::providers::base::GenericErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        let api_key = self.config.get_api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key required"))?;

        let response = self.pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !response.status().is_success() {
            return Err(self.map_http_error(response.status().as_u16(), &body));
        }

        // Parse and return response...
    }

    // ... other trait methods
}

impl MyProvider {
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, body),
            429 => ProviderError::rate_limit(PROVIDER_NAME, self.parse_retry_after(body)),
            400 => ProviderError::invalid_request(PROVIDER_NAME, body),
            500..=599 => ProviderError::provider_unavailable(PROVIDER_NAME, body),
            _ => ProviderError::api_error(PROVIDER_NAME, status, body),
        }
    }
}
```

---

## Connection Pooling

### GlobalPoolManager

All providers share a single HTTP connection pool for efficiency:

```rust
// src/core/providers/base/pool.rs

pub struct GlobalPoolManager {
    client: reqwest::Client,
}

impl GlobalPoolManager {
    pub fn new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(Duration::from_secs(300))
            .build()?;

        Ok(Self { client })
    }

    pub async fn execute_request<T: Serialize>(
        &self,
        url: &str,
        method: HttpMethod,
        headers: Vec<HeaderPair>,
        body: Option<T>,
    ) -> Result<Response, reqwest::Error> {
        let mut request = match method {
            HttpMethod::GET => self.client.get(url),
            HttpMethod::POST => self.client.post(url),
            HttpMethod::PUT => self.client.put(url),
            HttpMethod::DELETE => self.client.delete(url),
        };

        for (key, value) in headers {
            request = request.header(key, value);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        request.send().await
    }
}
```

### Usage Pattern

```rust
// In provider constructor
let pool_manager = Arc::new(GlobalPoolManager::new()?);

// Shared across all requests
let response = self.pool_manager
    .execute_request(&url, HttpMethod::POST, headers, Some(body))
    .await?;
```

---

## Model Information

### Static Model Registry

```rust
// model_info.rs
use std::collections::HashMap;
use std::sync::LazyLock;

pub struct ModelInfo {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub context_length: u32,
    pub max_output_tokens: u32,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub is_embedding: bool,
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
}

static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    configs.insert("model-name", ModelInfo {
        model_id: "model-name",
        display_name: "Model Display Name",
        context_length: 128000,
        max_output_tokens: 4096,
        supports_tools: true,
        supports_vision: false,
        is_embedding: false,
        input_cost_per_million: 0.50,
        output_cost_per_million: 1.50,
    });

    configs
});

pub fn get_model_info(model_id: &str) -> Option<&'static ModelInfo> {
    MODEL_CONFIGS.get(model_id)
}

pub fn get_available_models() -> Vec<&'static str> {
    MODEL_CONFIGS.keys().copied().collect()
}
```

---

## Provider Registration

### Module Export

```rust
// src/core/providers/my_provider/mod.rs
mod config;
mod model_info;
mod provider;

pub use config::MyProviderConfig;
pub use provider::MyProvider;
pub use model_info::get_models;
```

### Registry Registration

```rust
// src/core/providers/mod.rs
pub mod my_provider;
pub use my_provider::{MyProvider, MyProviderConfig};

// In router or gateway initialization
let provider: Box<dyn LLMProvider<Error = ProviderError>> =
    Box::new(MyProvider::from_env()?);
```

---

## Provider Capabilities

```rust
pub enum ProviderCapability {
    ChatCompletion,
    ChatCompletionStream,
    Embeddings,
    ToolCalling,
    VisionInput,
    AudioInput,
    AudioOutput,
    ImageGeneration,
    TextToSpeech,
    SpeechToText,
}

// In provider implementation
fn capabilities(&self) -> &'static [ProviderCapability] {
    &[
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
    ]
}
```

---

## References

- [reference/best-practices-and-checklist.md](reference/best-practices-and-checklist.md) — error factory conventions, coding practices, and new-provider checklist
- [reference/migration-from-legacy-errors.md](reference/migration-from-legacy-errors.md) — migrating legacy per-provider error enums to unified ProviderError
