# LiteLLM-RS Provider Architecture
## External Rust providers

Implement `litellm_rs::core::providers::ExternalProvider` to add an in-process provider without changing this crate. Return `BoxFuture`/`BoxStream` from the execution methods and use the existing `ProviderError` so retry hints and HTTP status remain intact. Declare implemented capabilities on both the provider and its `ModelInfo` entries; routing requires both declarations.

Wrap the implementation in `Provider::External(Arc::new(provider))`, construct a `Deployment`, and register it with `UnifiedRouter::add_deployment`. The [external integration test](../../tests/external_provider_registration.rs) is a complete downstream example covering registration, model selection, synchronous output, streaming and errors.

Chat is required; streaming, embeddings, image generation/editing and audio operations are optional and return `NotSupported` by default. Unknown cost is an error, not zero; configured gateway pricing remains authoritative. Configured native health probes invoke `ExternalProvider::health_check`; the default callback returns Unknown, and overrides must provide actual upstream evidence. External code owns its HTTP transport and credentials. Registration is programmatic, with no runtime plugin loading or configuration-based construction. Native Responses, native Gemini, rerank, moderation and image variations are not exposed by this extension interface.

## Single Provider Implementation Guide

This document outlines the architecture for implementing individual providers in LiteLLM-RS, using **DeepSeek** as a comprehensive example. This complements the main [System Overview](./system-overview.md) by focusing on provider-specific implementation patterns.

## Overview

Each provider in LiteLLM-RS follows a modular, trait-based architecture that ensures consistency, maintainability, and extensibility. The architecture is inspired by Python LiteLLM's provider system but leverages Rust's type safety and zero-cost abstractions.

## Provider Architecture Principles

### 1. **Modular Organization**
```
src/core/providers/deepseek/
├── mod.rs              # Module organization & exports
├── client.rs           # HTTP client & request execution  
├── config.rs           # Configuration & validation
├── error.rs            # Provider-specific error types
├── models.rs           # Model registry & specifications
├── provider.rs         # Main provider implementation
├── streaming.rs        # Streaming response handling
└── tests.rs           # Unit & integration tests
```

### 2. **Separation of Concerns**
- **Configuration**: Environment & YAML config handling
- **Client**: HTTP communication & API interaction
- **Transformation**: Request/response format conversion
- **Error Handling**: Provider-specific error mapping
- **Model Registry**: Dynamic model discovery & capabilities
- **Streaming**: Real-time response processing

### 3. **Trait-Based Design**
Each provider implements standardized traits for consistent behavior across the system.

## Provider Implementation Components

### 1. **Module Organization (`mod.rs`)**

```rust
//! DeepSeek AI Provider Module
//! 
//! DeepSeek V3.1 models with competitive performance and pricing:
//! - deepseek-chat: Non-thinking mode for general tasks
//! - deepseek-reasoner: Thinking mode for advanced reasoning

pub mod client;
pub mod config; 
pub mod error;
pub mod models;
pub mod provider;
pub mod streaming;

// Re-exports for easy access
pub use client::DeepSeekProvider;
pub use config::DeepSeekConfig;
pub use error::DeepSeekError;
pub use models::{get_deepseek_registry, DeepSeekModelRegistry};
```

**Purpose**: Central module organization following Rust best practices with minimal coupling.

### 2. **Configuration System (`config.rs`)**

```rust
/// Provider configuration with validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSeekConfig {
    /// API key (env: DEEPSEEK_API_KEY)
    pub api_key: Option<String>,
    /// Base API URL
    pub api_base: String,
    /// Request timeout
    pub timeout_seconds: u64,
    /// Custom headers
    pub headers: HashMap<String, String>,
    /// Retry configuration
    pub max_retries: u32,
    /// Extra parameters for requests
    pub extra_params: HashMap<String, Value>,
}

impl ProviderConfig for DeepSeekConfig {
    fn validate(&self) -> Result<(), String>;
    fn api_key(&self) -> Option<&str>;
    fn timeout(&self) -> Duration;
}
```

**Key Features**:
- Environment variable integration
- Validation logic
- Default implementations
- Type safety with serde

### 3. **Error Handling (`error.rs`)**

```rust
/// Provider-specific error types
#[derive(Debug, thiserror::Error)]
pub enum DeepSeekError {
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Rate limit exceeded")]
    RateLimit(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Model not found: {0}")]
    UnsupportedModel(String),
}

/// Error mapping to unified system
impl ProviderErrorTrait for DeepSeekError {
    fn error_type(&self) -> &'static str;
    fn is_retryable(&self) -> bool;
    fn http_status(&self) -> u16;
}
```

**Design Principles**:
- Comprehensive error coverage
- Integration with unified error system
- Retry logic information
- HTTP status mapping

### 4. **Model Registry (`models.rs`)**

```rust
/// Model specifications with features
pub struct ModelSpec {
    pub model_info: ModelInfo,
    pub features: Vec<ModelFeature>,
    pub config: ModelConfig,
}

/// Model feature detection
#[derive(Debug, Clone, PartialEq)]
pub enum ModelFeature {
    ReasoningMode,      // deepseek-reasoner
    FunctionCalling,    // Tool/function support
    StreamingSupport,   // Real-time responses
    SystemMessages,     // System prompt support
}

/// Dynamic model registry
pub struct DeepSeekModelRegistry {
    models: HashMap<String, ModelSpec>,
}

impl DeepSeekModelRegistry {
    /// Load models from pricing database
    fn load_models(&mut self);
    /// Detect model capabilities
    fn detect_features(&self, model_info: &ModelInfo) -> Vec<ModelFeature>;
    /// Get models supporting specific features
    pub fn get_models_with_feature(&self, feature: &ModelFeature) -> Vec<String>;
}
```

**Architecture Benefits**:
- Dynamic model discovery
- Feature-based capability detection
- Integration with pricing system
- Extensible model metadata

### 5. **HTTP Client (`client.rs`)**

```rust
/// Main provider implementation
#[derive(Debug, Clone)]
pub struct DeepSeekProvider {
    client: Client,
    config: DeepSeekConfig,
    base_url: String,
    models: Vec<ModelInfo>,
}

impl DeepSeekProvider {
    /// Constructor with validation
    pub async fn new(config: DeepSeekConfig) -> Result<Self, DeepSeekError>;
    
    /// HTTP request execution
    async fn execute_request<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: Value,
    ) -> Result<T, DeepSeekError>;
}

/// Unified provider trait implementation
#[async_trait]
impl LLMProvider for DeepSeekProvider {
    type Config = DeepSeekConfig;
    type Error = DeepSeekError;
    type ErrorMapper = DeepSeekErrorMapper;

    fn name(&self) -> &'static str { "deepseek" }
    fn capabilities(&self) -> &'static [ProviderCapability];
    fn models(&self) -> &[ModelInfo];
    
    // Core functionality
    async fn chat_completion(&self, request: ChatRequest, context: RequestContext) -> Result<ChatResponse, Self::Error>;
    async fn health_check(&self) -> HealthStatus;
    async fn calculate_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> Result<f64, Self::Error>;
}
```

**Key Design Elements**:
- Shared HTTP client with connection pooling
- Request/response transformation
- Health monitoring integration
- Cost calculation with pricing database

### 6. **Streaming Support (`streaming.rs`)**

```rust
/// Streaming response handler
pub struct DeepSeekStream {
    inner: Pin<Box<dyn Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send>>,
    parser: DeepSeekStreamParser,
}

/// Server-Sent Events parser
pub struct DeepSeekStreamParser {
    buffer: String,
    finished: bool,
}

impl DeepSeekStreamParser {
    /// Parse SSE chunks to ChatChunk
    pub fn parse_chunk(&mut self, data: &str) -> Result<Option<ChatChunk>, DeepSeekError>;
    
    /// Handle completion signals
    fn handle_completion(&mut self) -> bool;
}

impl Stream for DeepSeekStream {
    type Item = Result<ChatChunk, DeepSeekError>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
}
```

**Streaming Architecture**:
- Server-Sent Events (SSE) protocol
- Async stream implementation
- Chunk parsing and buffering
- Error handling and recovery

## Integration Patterns

### 1. **Provider Registration**

```rust
// In DefaultRouter or completion.rs
if let Ok(_api_key) = std::env::var("DEEPSEEK_API_KEY") {
    use crate::core::providers::deepseek::{DeepSeekProvider, DeepSeekConfig};
    let config = DeepSeekConfig::from_env();
    if let Ok(deepseek_provider) = DeepSeekProvider::new(config) {
        provider_registry.register(Provider::DeepSeek(deepseek_provider));
    }
}
```

### 2. **Unified Provider Dispatch**

```rust
// Macro-driven dispatch for zero-cost abstractions
dispatch_provider_method!(provider, chat_completion, request, context)

// Expands to:
match provider {
    Provider::DeepSeek(p) => p.chat_completion(request, context).await?,
    Provider::OpenAI(p) => p.chat_completion(request, context).await?,
    // ... other providers
}
```

### 3. **Configuration Integration**

```yaml
# config/gateway.yaml
providers:
  deepseek:
    api_key: "${DEEPSEEK_API_KEY}"
    api_base: "https://api.deepseek.com"
    timeout_seconds: 30
    max_retries: 3
    extra_params:
      reasoning_effort: "medium"
```

## Testing Architecture

### 1. **Unit Tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let config = DeepSeekConfig::default();
        assert!(config.validate().is_err()); // No API key
    }
    
    #[test]
    fn test_model_registry() {
        let registry = get_deepseek_registry();
        assert!(registry.supports_feature("deepseek-reasoner", &ModelFeature::ReasoningMode));
    }
    
    #[tokio::test]
    async fn test_provider_creation() {
        let config = DeepSeekConfig::from_env();
        let provider = DeepSeekProvider::new(config).await;
        assert!(provider.is_ok());
    }
}
```

### 2. **Integration Tests**

```rust
#[tokio::test]
#[ignore] // Requires API key
async fn test_chat_completion_integration() {
    let provider = setup_test_provider().await;
    let request = ChatRequest::new("deepseek-chat")
        .add_user_message("Hello, world!");
    
    let response = provider.chat_completion(request, default_context()).await;
    assert!(response.is_ok());
}
```

## Best Practices

### 1. **Configuration Management**
- Use environment variables with fallbacks
- Validate configuration at startup
- Support hot reloading for non-sensitive config
- Encrypt sensitive data (API keys)

### 2. **Error Handling**
- Map provider errors to unified types
- Provide detailed error context
- Implement retry logic for transient failures
- Log errors with correlation IDs

### 3. **Performance Optimization**
- Reuse HTTP connections
- Implement connection pooling
- Use async/await throughout
- Minimize memory allocations

### 4. **Observability**
- Add metrics for key operations
- Implement distributed tracing
- Log request/response for debugging
- Monitor provider health

## Provider Implementation Checklist

When implementing a new provider, ensure:

- [ ] **Configuration**: Environment integration & validation
- [ ] **Error Types**: Comprehensive error coverage
- [ ] **Model Registry**: Dynamic model discovery
- [ ] **HTTP Client**: Connection pooling & retry logic
- [ ] **Request Transform**: OpenAI format conversion
- [ ] **Response Transform**: Unified response format
- [ ] **Streaming**: Real-time response support
- [ ] **Cost Calculation**: Integration with pricing DB
- [ ] **Health Checks**: Provider availability monitoring
- [ ] **Tests**: Unit & integration test coverage
- [ ] **Documentation**: Usage examples & API docs
- [ ] **Observability**: Metrics & logging integration

## Example Usage

```rust
// examples/deepseek_completion.rs
use litellm_rs::{completion, user_message, system_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple completion
    let response = completion(
        "deepseek-chat",
        vec![
            system_message("You are a helpful assistant."),
            user_message("Explain quantum computing in simple terms."),
        ],
        None,
    ).await?;
    
    println!("Response: {}", response.choices[0].message.content);
    
    // Advanced reasoning with deepseek-reasoner
    let reasoning_response = completion(
        "deepseek-reasoner", 
        vec![user_message("Solve this logic puzzle: ...")],
        None,
    ).await?;
    
    Ok(())
}
```

This provider architecture ensures consistency across all providers while allowing for provider-specific optimizations and features. The modular design makes it easy to add new providers, maintain existing ones, and extend functionality as needed.

## Comparison with Python LiteLLM

| Aspect | Python LiteLLM | Rust LiteLLM |
|--------|----------------|--------------|
| **File Organization** | `/llms/provider/endpoint/` | `/providers/provider/component.rs` |
| **Error Handling** | Exception hierarchy | Result types with thiserror |
| **Configuration** | Dict-based config | Type-safe structs with serde |
| **Model Registry** | Static definitions | Dynamic discovery with features |
| **Streaming** | Generator functions | Async streams with futures |
| **Type Safety** | Runtime validation | Compile-time guarantees |
| **Performance** | Interpreted execution | Zero-cost abstractions |

The Rust implementation provides stronger type safety, better performance, and more maintainable code structure while maintaining the ease of use that makes Python LiteLLM popular.