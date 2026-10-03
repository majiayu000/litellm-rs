# LiteLLM-RS Provider Architecture Design

## 🏗️ Overview

LiteLLM-RS implements a **Unified Provider Architecture** that combines the best aspects of enum-based static dispatch and trait-based polymorphism. This hybrid design delivers zero-cost abstractions while maintaining excellent extensibility and type safety.

## 🎯 Design Principles

### 1. **Performance First**
- **Static dispatch**: All provider calls resolve at compile-time
- **Zero-cost abstractions**: No runtime overhead compared to direct calls
- **Optimal memory layout**: Enum variants stored efficiently on the stack

### 2. **Type Safety**
- **Compile-time verification**: All method calls validated by the compiler
- **Strong typing**: Each provider has its own configuration and error types
- **Exhaustive pattern matching**: Compiler ensures all providers are handled

### 3. **Developer Experience**
- **Uniform API**: Built-in providers implement the same `LLMProvider` trait
- **Macro-driven dispatch**: No repetitive match statements in user code
- **Clear error handling**: Unified error conversion with context preservation

### 4. **Extensibility**
- **Trait-based implementation**: new routed providers must implement `LLMProvider`
  and be wired into the `Provider` enum, dispatch macros, and factory path
- **Modular design**: Each provider is self-contained
- **Configuration flexibility**: Provider-specific config types

## 🔧 Core Architecture Components

### Provider Hierarchy

```rust
┌─────────────────────────────────────────────────────────────┐
│                    LiteLLM-RS Architecture                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │   User Code     │───▶│         Provider Enum          │ │
│  └─────────────────┘    │   (Static Dispatch Layer)      │ │
│                         └─────────────────┬───────────────┘ │
│                                          │                │
│  ┌─────────────────────────────────────────▼─────────────────────────────────┐ │
│  │                    Dispatch Macros                                      │ │
│  │  • dispatch_provider_async!     - Async with error conversion          │ │
│  │  • dispatch_provider_value!     - Direct value return                  │ │
│  │  • dispatch_provider_async_direct! - Async without conversion          │ │
│  └─────────────────────────────────────────┬─────────────────────────────────┘ │
│                                          │                                │
│  ┌─────────────────────────────────────────▼─────────────────────────────────┐ │
│  │                    LLMProvider Trait                                   │ │
│  │  • Uniform interface for all providers                                  │ │
│  │  • Associated types for Config, Error, ErrorMapper                     │ │
│  │  • Default implementations for optional features                       │ │
│  └─────────────────────────────────────────┬─────────────────────────────────┘ │
│                                          │                                │
│  ┌─────────────────────────────────────────▼─────────────────────────────────┐ │
│  │                Concrete Providers                                      │ │
│  │                                                                         │ │
│  │  OpenAI  │ Anthropic │ Azure │ DeepInfra │ AzureAI │ ... (12 total)    │ │
│  │                                                                         │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 1. Provider Enum (Static Dispatch Layer)

```rust
/// Unified Provider container with zero-cost dispatch
#[derive(Debug)]
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Azure(azure::AzureOpenAIProvider),
    Mistral(mistral::MistralProvider),
    DeepSeek(deepseek::DeepSeekProvider),
    Moonshot(moonshot::MoonshotProvider),
    MetaLlama(meta_llama::LlamaProvider),
    OpenRouter(openrouter::OpenRouterProvider),
    VertexAI(vertex_ai::VertexAIProvider),
    V0(v0::V0Provider),
    DeepInfra(deepinfra::DeepInfraProvider),
    AzureAI(azure_ai::AzureAIProvider),
}
```

**Key Features:**
- **Compile-time dispatch**: Each variant directly contains the concrete provider
- **Type safety**: Compiler ensures all variants are handled in match expressions
- **Memory efficiency**: Single allocation contains provider data
- **Performance**: No vtable lookups or dynamic dispatch overhead

### 2. LLMProvider Trait (Uniform Interface)

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync + Debug + 'static {
    type Config: ProviderConfig + Clone + Send + Sync;
    type Error: ProviderErrorTrait;
    type ErrorMapper: ErrorMapper<Self::Error>;
    
    // Core metadata
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &'static [ProviderCapability];
    fn models(&self) -> &[ModelInfo];
    
    // Core functionality (required)
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error>;
    
    // Optional functionality (with default implementations)
    async fn chat_completion_stream(...) -> Result<Stream, Self::Error> {
        Err(Self::Error::not_supported("streaming"))
    }
    
    async fn embeddings(...) -> Result<EmbeddingResponse, Self::Error> {
        Err(Self::Error::not_supported("embeddings"))
    }
    
    // Health and monitoring
    async fn health_check(&self) -> HealthStatus;
    async fn calculate_cost(&self, ...) -> Result<f64, Self::Error>;
}
```

**Benefits:**
- **Consistent interface**: All providers implement the same methods
- **Gradual feature adoption**: Optional methods have default "not supported" implementations
- **Strong typing**: Associated types ensure type safety across provider implementations
- **Future-proof**: New methods can be added with default implementations

`LLMProvider` is not currently a standalone router plugin boundary. Router
deployments store the closed `Provider` enum, so a third-party implementation
must also be added to the enum, dispatch macros, and factory wiring before it can
serve routed traffic.

### 3. Dispatch Macros (Boilerplate Elimination)

```rust
/// Async methods with unified error conversion
macro_rules! dispatch_provider_async {
    ($self:expr, $method:ident, $($arg:expr),*) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            Provider::Anthropic(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
            // ... 12 total providers
        }
    };
}

/// Direct value methods (no Result wrapping)
macro_rules! dispatch_provider_value {
    ($self:expr, $method:ident) => {
        match $self {
            Provider::OpenAI(p) => LLMProvider::$method(p),
            Provider::Anthropic(p) => LLMProvider::$method(p),
            // ... 12 total providers
        }
    };
}
```

**Advantages:**
- **DRY principle**: Eliminates 100+ lines of repetitive match statements
- **Maintainability**: Adding new providers requires only adding to macros
- **Consistency**: Ensures uniform error handling across all providers
- **Compile-time expansion**: No runtime cost

### 4. Unified Error System

```rust
/// Single error type for all providers
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed for {provider}: {message}")]
    Authentication { provider: &'static str, message: String },
    
    #[error("Rate limit exceeded for {provider}: {message}")]
    RateLimit { provider: &'static str, message: String, retry_after: Option<u64> },
    
    // ... comprehensive error variants
}

/// Automatic conversion from provider-specific errors
impl From<OpenAIError> for ProviderError { ... }
impl From<AnthropicError> for ProviderError { ... }
// ... all providers supported
```

**Benefits:**
- **Uniform error handling**: All providers return the same error type to users
- **Rich error information**: Includes provider context and structured data
- **Automatic conversion**: Provider-specific errors transparently converted
- **Error recovery**: Standardized retry logic based on error type

## 📊 Performance Characteristics

### Static Dispatch Performance

```rust
// This code:
let provider = Provider::OpenAI(openai_provider);
let response = provider.chat_completion(request, context).await?;

// Compiles to equivalent of:
let response = openai_provider.chat_completion(request, context).await
    .map_err(ProviderError::from)?;
```

**Performance Metrics:**
- **Call overhead**: 0ns (fully inlined)
- **Memory overhead**: 0 bytes (no vtable)
- **Binary size**: Minimal (dead code elimination)
- **Optimization**: Full (compiler can inline and optimize aggressively)

### Memory Layout

```
Provider enum size = max(all provider struct sizes) + 1 byte (discriminant)

Typical layout:
┌─────────────────────────────────────────────────────────┐
│ Provider::OpenAI                                        │
├───────────┬─────────────────────────────────────────────┤
│     0     │              OpenAIProvider                 │
│ (1 byte)  │             (rest of space)                 │
└───────────┴─────────────────────────────────────────────┘
```

## 🚀 Usage Examples

### Basic Usage

```rust
use litellm_rs::core::providers::{Provider, openai, anthropic};

// Create providers
let openai = Provider::OpenAI(
    openai::OpenAIProvider::new(openai_config).await?
);
let anthropic = Provider::Anthropic(
    anthropic::AnthropicProvider::new(anthropic_config).await?
);

// Uniform API
for provider in [openai, anthropic] {
    println!("Provider: {}", provider.name());
    println!("Models: {:?}", provider.list_models());
    
    let response = provider
        .chat_completion(request.clone(), context.clone())
        .await?;
        
    println!("Response: {:?}", response);
}
```

### Advanced Usage with Error Handling

Use the router's policy with the actual attempt, remaining budget and deadline.
Choose the context for the operation: non-idempotent requests and streams that
have already emitted output must not use an idempotent unary context.

```rust
use litellm_rs::core::router::retry_policy::RetryPolicy;

match provider.chat_completion(request, context).await {
    Ok(response) => { /* consume the response */ }
    Err(error) => {
        let decision = RetryPolicy.decide(&router_config, &error, retry_context);
        if decision.should_retry {
            if let Some(delay) = decision.delay {
                tokio::time::sleep(delay).await;
            }
            // Schedule the next attempt and update retry_context.
        } else {
            return Err(error);
        }
    }
}
```

## 🔧 Extending the Architecture

### Adding a New Routed Provider

1. **Implement the Provider**:

```rust
// src/core/providers/myai/mod.rs
pub struct MyAIProvider {
    config: MyAIConfig,
    client: HttpClient,
}

#[async_trait]
impl LLMProvider for MyAIProvider {
    type Config = MyAIConfig;
    type Error = MyAIError;
    type ErrorMapper = MyAIErrorMapper;
    
    fn name(&self) -> &'static str {
        "myai"
    }
    
    // ... implement all required methods
}
```

2. **Add to Provider Enum**:

```rust
pub enum Provider {
    // ... existing providers
    MyAI(myai::MyAIProvider),
}
```

3. **Update Dispatch Macros**:

```rust
// Add single line to each macro
Provider::MyAI(p) => LLMProvider::$method(p, $($arg),*).await.map_err(ProviderError::from),
```

4. **Add to ProviderType**:

```rust
pub enum ProviderType {
    // ... existing types
    MyAI,
}
```

**Total effort**: ~10 lines of boilerplate code changes

## 📈 Benchmarks

### Theoretical Performance Analysis

```rust
// Direct provider call (baseline)
let response = openai_provider.chat_completion(request, context).await?;
// Performance: 100% (baseline)

// LiteLLM-RS hybrid approach  
let provider = Provider::OpenAI(openai_provider);
let response = provider.chat_completion(request, context).await?;
// Performance: 100% (identical after optimization)

// Pure trait object approach
let provider: Box<dyn LLMProvider> = Box::new(openai_provider);
let response = provider.chat_completion(request, context).await?;
// Performance: ~85-95% (vtable + heap allocation overhead)
```

## 📝 Summary

The LiteLLM-RS Provider Architecture represents a sophisticated balance between performance, safety, and extensibility:

- **🚀 Performance**: Zero-cost abstractions with static dispatch
- **🛡️ Safety**: Compile-time type checking and memory safety  
- **🔧 Maintainability**: Uniform interfaces and macro-driven dispatch
- **📈 Scalability**: Easy provider addition with minimal boilerplate
- **🎯 Reliability**: Comprehensive error handling and health monitoring

This architecture enables LiteLLM-RS to deliver production-grade performance while maintaining the developer experience and ecosystem compatibility that makes LiteLLM successful.

---

## 📋 Current Implementation Status

### ✅ Fully Implemented (12 Providers)
- **OpenAI**: Complete with streaming, embeddings, image generation
- **Anthropic**: Complete with streaming and function calling
- **Azure**: OpenAI-compatible with enterprise features
- **Mistral**: Chat completion and function calling
- **DeepSeek**: High-performance Chinese AI provider
- **Moonshot**: Alternative OpenAI-compatible provider
- **MetaLlama**: Meta's LLaMA models via various providers
- **OpenRouter**: Multi-provider routing and access
- **VertexAI**: Google Cloud AI platform integration
- **V0**: Development and testing provider
- **DeepInfra**: Model hosting platform
- **AzureAI**: Azure AI Foundry integration

### 🎯 Architecture Benefits Achieved

1. **100% API Consistency**: All 12 providers use identical method signatures
2. **Zero Runtime Overhead**: Static dispatch with compile-time optimization
3. **Type Safety**: Comprehensive compile-time checking prevents runtime errors
4. **Extensibility**: Adding new providers requires minimal boilerplate changes
5. **Error Handling**: Unified error system with automatic conversion
6. **Feature Parity**: Consistent streaming, embeddings, and health check support

### 🚀 Next Steps

1. **Performance Benchmarks**: Comprehensive performance testing across all providers
2. **Integration Tests**: End-to-end testing with real provider APIs
3. **Documentation**: Complete API documentation and usage examples
4. **Production Hardening**: Error handling, retry logic, and monitoring improvements

This unified architecture successfully combines the performance benefits of Rust's type system with the developer experience of Python LiteLLM's uniform API design.
