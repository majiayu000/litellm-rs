# Provider Support Comparison: litellm-rs vs litellm

## Executive Summary

| Dimension | litellm-rs (Rust) | litellm (Python) |
|-----------|-------------------|------------------|
| **Total Providers** | 16 | 100+ |
| **Architecture** | Trait-based (compile-time) | Class-based (runtime) |
| **Type Safety** | Compile-time verification | Runtime checks |
| **Extensibility** | Implement `LLMProvider` and wire into `Provider` enum/factory | Inherit `BaseConfig` class |
| **Maturity** | Early stage | Production-ready |

---

## 1. Supported Provider List Comparison

### 1.1 litellm-rs Providers (16 total)

```rust
pub enum Provider {
    OpenAI,      // Full implementation: chat, streaming, embeddings, images
    Anthropic,   // Full implementation: chat, streaming, multimodal, tool calling
    Azure,       // Azure OpenAI: chat, assistants, batches, images
    AzureAI,     // Azure AI Studio: chat, embeddings, rerank, images
    Bedrock,     // AWS Bedrock: chat, streaming, embeddings, images
    Cloudflare,  // Workers AI: chat, streaming
    DeepInfra,   // Chat, streaming, rerank
    DeepSeek,    // Chat, streaming, thinking/reasoning
    Gemini,      // Google Gemini: chat, streaming, multimodal
    Groq,        // Chat, streaming, STT
    MetaLlama,   // Meta Llama API: chat, streaming
    Mistral,     // Chat, streaming
    Moonshot,    // Chat, streaming
    OpenRouter,  // Multi-provider gateway: chat, streaming
    VertexAI,    // Google Vertex AI: chat, streaming, embeddings, multimodal
    XAI,         // Grok: chat, streaming
    V0,          // Chat, streaming (custom/internal provider)
}
```

### 1.2 litellm (Python) Providers (100+ total)

#### Tier 1 - Major Providers (Full Support)
- **OpenAI** - Complete: chat, streaming, embeddings, images, audio, fine-tuning, assistants, realtime
- **Anthropic** - Complete: chat, streaming, batches, files, skills, multimodal
- **Azure OpenAI** - Complete: all OpenAI features + Azure-specific
- **Azure AI** - Complete: chat, embeddings, rerank, images, agents, OCR
- **Bedrock** - Complete: chat, streaming, embeddings, images, guardrails, knowledge bases
- **Vertex AI** - Complete: chat, streaming, embeddings, batches, multimodal
- **Gemini** - Complete: chat, streaming, multimodal, files, caching

#### Tier 2 - Well-Supported Providers
- **Cohere** - Chat, streaming, embeddings, rerank
- **Mistral** - Chat, streaming, embeddings
- **Groq** - Chat, streaming
- **Together AI** - Chat, streaming, embeddings
- **Fireworks AI** - Chat, streaming, embeddings
- **DeepSeek** - Chat, streaming
- **OpenRouter** - Multi-provider gateway
- **Perplexity** - Chat, streaming, search
- **Replicate** - Chat, images
- **HuggingFace** - Chat, embeddings

#### Tier 3 - Specialty Providers
| Category | Providers |
|----------|-----------|
| **Cloud Platforms** | AWS Sagemaker, OCI, Snowflake, Databricks, WatsonX |
| **Image/Video** | RunwayML, Fal AI, Stability, Recraft, Topaz |
| **Audio/Speech** | ElevenLabs, Deepgram, AWS Polly |
| **Search** | Tavily, Exa AI, Google PSE, Linkup, Searxng |
| **Self-Hosted** | Ollama, LM Studio, vLLM, Llamafile, Triton |
| **China Providers** | DashScope, Moonshot, Minimax, Volcengine, GigaChat |
| **Embeddings** | Voyage, Jina AI, Infinity, Milvus, PG Vector |
| **Others** | 60+ more specialized providers |

### 1.3 Provider Count Summary

```
Category                    Rust    Python
----------------------------------------
Major Cloud Providers        7        7
OpenAI-Compatible           4       15+
Chinese Providers           1        8+
Self-Hosted                 0       10+
Specialty (Image/Audio)     0       15+
Embeddings Only             0        8+
Search                      0        5+
Other                       4       40+
----------------------------------------
TOTAL                      16      100+
```

---

## 2. Provider Implementation Architecture

### 2.1 Rust: Trait-Based Architecture

In `litellm-rs`, `LLMProvider` is the implementation trait for built-in
providers. Router deployments use the closed `Provider` enum, so implementing
the trait alone does not make a third-party provider routeable.

```rust
// Core LLMProvider trait definition
#[async_trait]
pub trait LLMProvider: Send + Sync + Debug + 'static {
    // Associated types for type safety
    type Config: ProviderConfig + Clone + Send + Sync;
    type Error: ProviderErrorTrait;
    type ErrorMapper: ErrorMapper<Self::Error>;

    // Required methods
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

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error>;

    async fn health_check(&self) -> HealthStatus;

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error>;

    // Optional with defaults
    async fn chat_completion_stream(...) -> Result<Stream<...>, Self::Error>;
    async fn embeddings(...) -> Result<EmbeddingResponse, Self::Error>;
    async fn image_generation(...) -> Result<ImageGenerationResponse, Self::Error>;
}
```

**Enum-based Dispatch Pattern:**
```rust
pub enum Provider {
    OpenAI(openai::OpenAIProvider),
    Anthropic(anthropic::AnthropicProvider),
    Azure(azure::AzureOpenAIProvider),
    // ... more variants
}

impl Provider {
    pub async fn chat_completion(&self, request: ChatRequest, context: RequestContext)
        -> Result<ChatResponse, ProviderError>
    {
        dispatch_provider_async!(self, chat_completion, request, context)
    }
}
```

### 2.2 Python: Class-Based Architecture

```python
# Base configuration class
class BaseConfig(ABC):
    @abstractmethod
    def get_supported_openai_params(self, model: str) -> list:
        pass

    @abstractmethod
    def map_openai_params(
        self,
        non_default_params: dict,
        optional_params: dict,
        model: str,
        drop_params: bool,
    ) -> dict:
        pass

    @abstractmethod
    def validate_environment(
        self,
        headers: dict,
        model: str,
        messages: List[AllMessageValues],
        optional_params: dict,
        litellm_params: dict,
        api_key: Optional[str] = None,
        api_base: Optional[str] = None,
    ) -> dict:
        pass

    @abstractmethod
    def transform_request(
        self,
        model: str,
        messages: List[AllMessageValues],
        optional_params: dict,
        litellm_params: dict,
        headers: dict,
    ) -> dict:
        pass

    @abstractmethod
    def transform_response(
        self,
        model: str,
        raw_response: httpx.Response,
        model_response: "ModelResponse",
        logging_obj: LiteLLMLoggingObj,
        request_data: dict,
        messages: List[AllMessageValues],
        optional_params: dict,
        litellm_params: dict,
        encoding: Any,
        api_key: Optional[str] = None,
        json_mode: Optional[bool] = None,
    ) -> "ModelResponse":
        pass

    @abstractmethod
    def get_error_class(
        self, error_message: str, status_code: int, headers: Union[dict, httpx.Headers]
    ) -> BaseLLMException:
        pass
```

### 2.3 Architecture Comparison

| Aspect | Rust (litellm-rs) | Python (litellm) |
|--------|-------------------|------------------|
| **Dispatch** | Enum pattern (zero-cost) | Dynamic dispatch |
| **Type Safety** | Compile-time | Runtime |
| **Error Handling** | Associated error types | Exception classes |
| **Extensibility** | Add enum variant + implement trait | Subclass BaseConfig |
| **Runtime Overhead** | Near-zero | Python overhead |
| **Capability Check** | Compile-time macros | Runtime introspection |

---

## 3. Provider Functionality Coverage

### 3.1 Feature Matrix

| Feature | Rust Implementation | Python Implementation |
|---------|--------------------|-----------------------|
| **Chat Completion** | All 16 providers | All 100+ providers |
| **Streaming** | 5 providers (OpenAI, Anthropic, DeepInfra, AzureAI, Groq) | All major providers |
| **Embeddings** | 2 providers (OpenAI, Azure) | 30+ providers |
| **Image Generation** | 1 provider (OpenAI) | 15+ providers |
| **Tool Calling** | Full support in types | Full support |
| **Vision/Multimodal** | Types defined (ContentPart) | Full implementation |
| **Audio (TTS/STT)** | Groq STT only | 10+ providers |
| **Batches** | OpenAI-compatible provider proxy routes | Anthropic, Azure, Vertex AI |
| **Fine-tuning** | OpenAI module exists | OpenAI, Azure |
| **Assistants** | Azure assistants | OpenAI, Azure |
| **Realtime** | OpenAI module exists | OpenAI, Azure |
| **Rerank** | AzureAI, DeepInfra | 5+ providers |
| **Web Search** | Not implemented | Perplexity, Gemini, Anthropic |

### 3.2 Rust Implementation Status by Provider

| Provider | Chat | Stream | Tools | Vision | Embed | Images |
|----------|------|--------|-------|--------|-------|--------|
| OpenAI | Complete | Complete | Complete | Complete | Complete | Complete |
| Anthropic | Complete | Complete | Complete | Complete | N/A | N/A |
| Azure | Complete | Partial | Complete | Partial | Complete | Partial |
| AzureAI | Complete | Complete | Complete | Partial | Complete | Partial |
| Bedrock | Complete | Complete | Complete | Partial | Partial | Partial |
| Cloudflare | Complete | Partial | Partial | N/A | N/A | N/A |
| DeepInfra | Complete | Complete | N/A | N/A | N/A | N/A |
| DeepSeek | Complete | Complete | Complete | N/A | N/A | N/A |
| Gemini | Complete | Complete | Complete | Complete | N/A | N/A |
| Groq | Complete | Complete | Complete | Partial | N/A | N/A |
| MetaLlama | Complete | Partial | Partial | Partial | N/A | N/A |
| Mistral | Complete | Partial | Complete | N/A | N/A | N/A |
| Moonshot | Complete | Partial | N/A | N/A | N/A | N/A |
| OpenRouter | Complete | Partial | Complete | Via routing | N/A | N/A |
| VertexAI | Complete | Complete | Complete | Complete | Complete | N/A |
| XAI | Complete | Partial | Complete | N/A | N/A | N/A |

### 3.3 What's Missing in Rust

**Provider Coverage:**
- Audio providers (ElevenLabs, Deepgram, AWS Polly)
- Image-focused providers (Fal AI, Stability, RunwayML)
- Search providers (Tavily, Exa AI)
- Self-hosted (Ollama, vLLM, LM Studio)
- Many Chinese providers (DashScope, Minimax, Volcengine)
- Specialty providers (Cohere, Replicate, HuggingFace)

**Feature Coverage:**
- Audio transcription (only Groq partial)
- Text-to-speech
- Video generation
- OCR
- Web search integration
- Files API (upload/download)
- Vector stores

---

## 4. Provider Extension Mechanism

### 4.1 Adding New Provider in Rust

**Step 1: Create Provider Module Structure**
```
src/core/providers/new_provider/
    mod.rs          # Module exports
    config.rs       # Provider configuration
    client.rs       # HTTP client
    provider.rs     # LLMProvider implementation
    error.rs        # Error types
    streaming.rs    # Streaming support
    models.rs       # Model registry
```

**Step 2: Implement Config**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProviderConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub timeout: Duration,
}

impl ProviderConfig for NewProviderConfig {
    fn api_key(&self) -> Option<&str> { Some(&self.api_key) }
    fn api_base(&self) -> Option<&str> { self.api_base.as_deref() }
    fn timeout(&self) -> Duration { self.timeout }
    // ...
}
```

**Step 3: Implement LLMProvider Trait**
```rust
#[async_trait]
impl LLMProvider for NewProvider {
    type Config = NewProviderConfig;
    type Error = NewProviderError;
    type ErrorMapper = NewProviderErrorMapper;

    fn name(&self) -> &'static str { "new_provider" }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::ChatCompletion,
          ProviderCapability::ChatCompletionStream]
    }

    async fn chat_completion(&self, request: ChatRequest, context: RequestContext)
        -> Result<ChatResponse, Self::Error>
    {
        // Implementation
    }
    // ... implement other required methods
}
```

The provider must also be added to the `Provider` enum, dispatch arms, provider
type metadata, and factory wiring before router deployments can use it.

**Step 4: Add to Provider Enum**
```rust
// In mod.rs
pub enum Provider {
    // ... existing
    NewProvider(new_provider::NewProvider),
}

// Update all dispatch macros
```

**Estimated Code: ~500-1000 lines** for a basic provider

### 4.2 Adding New Provider in Python

**Step 1: Create Provider Directory**
```
litellm/llms/new_provider/
    __init__.py
    chat/
        transformation.py    # Request/response transformation
        handler.py          # API calls
```

**Step 2: Implement Config Class**
```python
class NewProviderConfig(BaseConfig):
    def get_supported_openai_params(self, model: str) -> list:
        return ["temperature", "max_tokens", "stream", "tools", ...]

    def map_openai_params(self, non_default_params, optional_params, model, drop_params):
        # Map OpenAI params to provider format
        return optional_params

    def validate_environment(self, headers, model, messages, optional_params,
                            litellm_params, api_key=None, api_base=None):
        # Validate and set auth headers
        return headers

    def transform_request(self, model, messages, optional_params,
                         litellm_params, headers):
        # Convert to provider format
        return request_body

    def transform_response(self, model, raw_response, model_response,
                          logging_obj, request_data, messages,
                          optional_params, litellm_params, encoding,
                          api_key=None, json_mode=None):
        # Convert response to standard format
        return model_response

    def get_error_class(self, error_message, status_code, headers):
        return BaseLLMException(status_code, error_message, headers)
```

**Step 3: Register Provider**
```python
# Add to model mappings and routing logic
```

**Estimated Code: ~200-500 lines** for a basic provider

### 4.3 Extension Comparison

| Aspect | Rust | Python |
|--------|------|--------|
| **Minimum Files** | 6-8 | 2-3 |
| **Code Lines** | 500-1000 | 200-500 |
| **Type Safety** | Compile-time errors | Runtime errors |
| **Testing** | Unit + integration | Unit + integration |
| **Maintenance** | Lower (compile checks) | Higher (runtime checks) |

---

## 5. Special Provider Handling

### 5.1 Streaming Implementation

**Rust SSE Parser (Unified):**
```rust
pub type OpenAIStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    OpenAICompatibleTransformer,
>;

pub fn create_openai_stream(
    stream: impl Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
) -> OpenAIStream {
    let transformer = OpenAICompatibleTransformer::new("openai");
    UnifiedSSEStream::new(Box::pin(stream), transformer)
}
```

**Python Stream Handler:**
```python
def get_model_response_iterator(
    self,
    streaming_response: Union[Iterator[str], AsyncIterator[str], "ModelResponse"],
    sync_stream: bool,
    json_mode: Optional[bool] = False,
) -> Any:
    # Provider-specific stream iteration
```

### 5.2 Tool Calling

**Rust Tool Types:**
```rust
pub struct Tool {
    pub tool_type: ToolType,
    pub function: FunctionDefinition,
}

pub struct ToolCall {
    pub id: String,
    pub tool_type: String,
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String,  // JSON string
}
```

**Python Tool Handling:**
```python
def convert_tool_use_to_openai_format(
    anthropic_tool_content: Dict[str, Any],
    index: int,
) -> ChatCompletionToolCallChunk:
    # Convert between provider formats
```

### 5.3 Vision/Multimodal

**Rust Content Types:**
```rust
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    Audio { audio: AudioData },
    Image { source: ImageSource, detail: Option<String>, ... },
    Document { source: DocumentSource, cache_control: Option<CacheControl> },
    ToolResult { tool_use_id: String, content: Value, is_error: Option<bool> },
    ToolUse { id: String, name: String, input: Value },
}
```

**Python Message Handling:**
```python
# Complex message content types
AllMessageValues = Union[
    ChatCompletionSystemMessage,
    ChatCompletionUserMessage,
    ChatCompletionAssistantMessage,
    ChatCompletionToolMessage,
    ...
]
```

### 5.4 Thinking/Reasoning Mode

**Rust Thinking Support:**
```rust
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u32>,
    // Provider-specific options
}

pub struct ThinkingContent {
    pub text: Option<String>,
    pub tokens: Option<u32>,
}
```

**Python Extended Thinking:**
```python
def is_thinking_enabled(self, non_default_params: dict) -> bool:
    return (
        non_default_params.get("thinking", {}).get("type") == "enabled"
        or non_default_params.get("reasoning_effort") is not None
    )
```

---

## 6. Performance Considerations

### 6.1 Rust Advantages
- **Zero-cost abstractions**: Enum dispatch compiles to jump tables
- **Compile-time verification**: No runtime capability checks needed
- **Memory efficiency**: No Python object overhead
- **Async/await**: Native Tokio runtime

### 6.2 Python Advantages
- **Rapid development**: Faster to add new providers
- **Dynamic features**: Runtime provider loading
- **Community ecosystem**: More integrations available
- **Production-tested**: Battle-tested at scale

---

## 7. Recommendations

### For litellm-rs Development

**Priority 1 - Complete Core Providers:**
1. Complete streaming for all existing providers
2. Add embeddings support to more providers
3. Implement full vision/multimodal handling

**Priority 2 - Add High-Demand Providers:**
1. Ollama (self-hosted essential)
2. Cohere (embeddings + rerank)
3. Together AI (popular)
4. HuggingFace (community)

**Priority 3 - Feature Parity:**
1. Audio transcription
2. Web search integration
3. Files API

### Migration Path

For users migrating from Python litellm:
1. Core providers (OpenAI, Anthropic, Azure) have good coverage
2. Streaming requires checking provider support
3. Self-hosted options not yet available
4. Specialty features may need Python fallback

---

## Appendix: Complete Provider Lists

### A. litellm-rs Provider Modules

```
src/core/providers/
    anthropic/     # 9 files - Full implementation
    azure/         # 13 files - Azure OpenAI
    azure_ai/      # 12 files - Azure AI Studio
    bedrock/       # 21 files - AWS Bedrock
    cloudflare/    # 7 files
    deepinfra/     # 7 files
    deepseek/      # 9 files
    gemini/        # 9 files
    groq/          # 10 files
    meta_llama/    # 5 files
    mistral/       # 5 files
    moonshot/      # 4 files
    openai/        # 22 files - Full implementation
    openrouter/    # 10 files
    v0/            # 4 files
    vertex_ai/     # 26 files
    xai/           # 7 files
```

### B. litellm (Python) Provider Directories

```
108 total provider directories including:
- 7 major cloud providers
- 15+ OpenAI-compatible providers
- 8+ Chinese providers
- 10+ self-hosted solutions
- 15+ specialty providers (image/audio/video)
- 8+ embedding-specific providers
- 5+ search providers
- 40+ other providers
```

---

*Document generated: 2026-01-09*
*Analysis based on: litellm-rs commit 161d9f9, litellm latest*
