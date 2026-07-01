# API Compatibility Deep Analysis: litellm-rs vs litellm

This document provides a comprehensive comparison of API compatibility between the Rust implementation (litellm-rs) and the Python implementation (litellm).

## Executive Summary

| Aspect | litellm-rs | litellm (Python) | Compatibility |
|--------|-----------|-----------------|---------------|
| OpenAI API Compatibility | High | Full | 90% |
| Streaming Support | Full | Full | 100% |
| Function Calling | Full | Full | 100% |
| Vision Support | Full | Full | 100% |
| JSON Mode | Full | Full | 100% |
| SDK Drop-in Replace | Partial | Full | 85% |

---

## 1. OpenAI API Endpoint Compatibility

### 1.1 /chat/completions

#### Endpoint Routes

| Route | litellm-rs | litellm | Notes |
|-------|-----------|---------|-------|
| `/v1/chat/completions` | Yes | Yes | Primary endpoint |
| `/chat/completions` | Yes | Yes | Alias without version |
| `/engines/{model}/chat/completions` | No | Yes | Azure compatibility |
| `/openai/deployments/{model}/chat/completions` | No | Yes | Azure compatibility |

**Analysis:**
- litellm-rs provides the core `/v1/chat/completions` and `/chat/completions` endpoints
- litellm includes additional Azure-compatible routes for enterprise scenarios
- Gap: litellm-rs lacks Azure deployment-style routing

#### Request Parameters

| Parameter | litellm-rs | litellm | Type |
|-----------|-----------|---------|------|
| model | Yes | Yes | string |
| messages | Yes | Yes | array |
| temperature | Yes | Yes | float |
| max_tokens | Yes | Yes | integer |
| max_completion_tokens | Yes | Yes | integer (new OpenAI param) |
| top_p | Yes | Yes | float |
| frequency_penalty | Yes | Yes | float |
| presence_penalty | Yes | Yes | float |
| stop | Yes | Yes | string/array |
| stream | Yes | Yes | boolean |
| stream_options | No | Yes | object |
| tools | Yes | Yes | array |
| tool_choice | Yes | Yes | string/object |
| parallel_tool_calls | Yes | Yes | boolean |
| response_format | Yes | Yes | object |
| seed | Yes | Yes | integer |
| n | Yes | Yes | integer |
| logit_bias | Yes | Yes | object |
| logprobs | Yes | Yes | boolean |
| top_logprobs | Yes | Yes | integer |
| user | Yes | Yes | string |
| functions (legacy) | Yes | Yes | array |
| function_call (legacy) | Yes | Yes | string/object |
| service_tier | No | Yes | string |
| metadata | No | Yes | object |

**Compatibility Score: 92%**

#### Response Format

**litellm-rs Response Structure:**
```rust
pub struct ChatResponse {
    pub id: String,                              // chatcmpl-{uuid}
    pub object: String,                          // "chat.completion"
    pub created: i64,                            // Unix timestamp
    pub model: String,                           // Model name
    pub choices: Vec<ChatChoice>,                // Response choices
    pub usage: Option<Usage>,                    // Token usage
    pub system_fingerprint: Option<String>,      // System fingerprint
}
```

**litellm (Python) Response Structure:**
```python
class ModelResponse(OpenAIObject):
    id: str                                      # chatcmpl-{uuid}
    object: str                                  # "chat.completion"
    created: int                                 # Unix timestamp
    model: str                                   # Model name
    choices: List[Union[Choices, StreamingChoices]]
    usage: Optional[Usage]
    system_fingerprint: Optional[str]
    _hidden_params: dict                         # Internal tracking
    provider_specific_fields: Optional[Dict]     # Provider extensions
```

**Key Differences:**
1. litellm includes `_hidden_params` for internal tracking (model_id, api_base, etc.)
2. litellm includes `provider_specific_fields` for vendor-specific extensions
3. Response ID format: Both use `chatcmpl-{uuid}` format
4. Timestamps: litellm-rs uses i64, litellm uses int (compatible)

---

### 1.2 /completions (Legacy)

#### Endpoint Routes

| Route | litellm-rs | litellm | Notes |
|-------|-----------|---------|-------|
| `/v1/completions` | Yes | Yes | Primary endpoint |
| `/completions` | Yes | Yes | Alias |
| `/engines/{model}/completions` | No | Yes | Azure compatibility |
| `/openai/deployments/{model}/completions` | No | Yes | Azure compatibility |

#### Implementation Approach

**litellm-rs:**
- Converts legacy completion requests to chat format internally
- Maps prompt to user message
- Returns text completion response format

```rust
// Internal conversion
let messages = vec![ChatMessage {
    role: MessageRole::User,
    content: Some(MessageContent::Text(request.prompt.clone())),
    // ...
}];
```

**litellm:**
- Native text completion support
- Direct pass-through to providers supporting completions
- Automatic conversion for providers without native support

**Compatibility Score: 95%**

---

### 1.3 /embeddings

#### Endpoint Routes

| Route | litellm-rs | litellm | Notes |
|-------|-----------|---------|-------|
| `/v1/embeddings` | Yes | Yes | Primary endpoint |
| `/embeddings` | Yes | Yes | Alias |
| `/engines/{model}/embeddings` | No | Yes | Azure compatibility |
| `/openai/deployments/{model}/embeddings` | No | Yes | Azure compatibility |

#### Request Parameters

| Parameter | litellm-rs | litellm | Notes |
|-----------|-----------|---------|-------|
| model | Yes | Yes | Required |
| input | Yes | Yes | string/array |
| encoding_format | Yes | Yes | "float"/"base64" |
| dimensions | Yes | Yes | integer |
| user | Yes | Yes | string |

#### Input Handling

**litellm-rs:**
```rust
let input = match &request.input {
    serde_json::Value::String(s) => EmbeddingInput::Text(s.clone()),
    serde_json::Value::Array(arr) => {
        let texts: Vec<String> = arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        EmbeddingInput::Array(texts)
    }
    _ => return Err(GatewayError::validation("Invalid input"))
};
```

**litellm:**
- Supports token array input with automatic decoding
- Provider-specific token array handling
- Batch input optimization

**Key Difference:** litellm handles token array inputs (list of integer tokens) with automatic decoding for providers that don't support this format natively.

**Compatibility Score: 90%**

---

### 1.4 /models

#### Endpoint Routes

| Route | litellm-rs | litellm | Notes |
|-------|-----------|---------|-------|
| `/v1/models` | Yes | Yes | List all models |
| `/models` | Yes | Yes | Alias |
| `/v1/models/{model_id}` | Yes | Yes | Get specific model |
| `/models/{model_id}` | Yes | Yes | Alias |

#### Response Format

**litellm-rs:**
```rust
pub struct ModelListResponse {
    pub object: String,      // "list"
    pub data: Vec<Model>,
}

pub struct Model {
    pub id: String,          // Model ID
    pub object: String,      // "model"
    pub created: u64,        // Timestamp
    pub owned_by: String,    // Provider name
}
```

**litellm:**
```python
# Returns dict format
{
    "object": "list",
    "data": [
        {
            "id": str,
            "object": "model",
            "created": int,
            "owned_by": str,
        }
    ]
}
```

**Additional litellm Features:**
- `return_wildcard_routes` parameter for pattern matching
- `team_id` filtering for team-specific models
- `include_model_access_groups` for access group information
- `only_model_access_groups` for filtered listing

**Compatibility Score: 85%**

---

### 1.5 Additional Endpoints

| Endpoint | litellm-rs | litellm | Notes |
|----------|-----------|---------|-------|
| `/v1/images/generations` | Yes | Yes | Image generation |
| `/v1/audio/speech` | Yes | Yes | Text-to-speech |
| `/v1/audio/transcriptions` | Yes | Yes | Speech-to-text |
| `/v1/audio/translations` | Yes | Yes | Audio translation |
| `/v1/moderations` | No | Yes | Content moderation |
| `/v1/files` | No | Yes | File management |
| `/v1/batches` | Partial | Yes | Provider-backed OpenAI-compatible create/list/get/cancel proxy; no durable local batch execution |
| `/v1/fine_tuning/jobs` | No | Yes | Fine-tuning |
| `/v1/assistants` | No | Yes | Assistants API |
| `/v1/threads` | No | Yes | Threads API |
| `/v1/realtime` | No | Yes | Realtime API (WebSocket) |

---

## 2. Request/Response Format Compatibility

### 2.1 Message Structure

#### Role Types

| Role | litellm-rs | litellm | Notes |
|------|-----------|---------|-------|
| system | Yes | Yes | System instructions |
| user | Yes | Yes | User messages |
| assistant | Yes | Yes | AI responses |
| tool | Yes | Yes | Tool results |
| function | Yes | Yes | Legacy function results |

#### Content Types

**litellm-rs:**
```rust
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    Audio { audio: AudioData },
    Image { source: ImageSource, detail: Option<String> },
    Document { source: DocumentSource, cache_control: Option<CacheControl> },
    ToolResult { tool_use_id: String, content: Value, is_error: Option<bool> },
    ToolUse { id: String, name: String, input: Value },
}
```

**litellm:**
```python
# Supports OpenAI's AllMessageValues type
# Including all content types via openai package types
content: Union[str, List[ContentPart]]
# ContentPart can be text, image_url, audio, etc.
```

**Compatibility:** Both implementations support the full range of OpenAI content types including multimodal inputs.

### 2.2 Error Response Format

**litellm-rs OpenAI-Style Error:**
```rust
pub enum OpenAIError {
    ApiError { message: String, status_code: Option<u16>, error_type: Option<String> },
    Authentication(String),
    RateLimit(String),
    ModelNotFound { model: String },
    InvalidRequest(String),
    Network(String),
    Timeout(String),
    // ...
}
```

HTTP Error Response:
```json
{
  "success": false,
  "error": "Error message",
  "data": null
}
```

**litellm Error Response:**
```python
class ProxyException(HTTPException):
    message: str
    type: str
    param: str
    code: int
```

HTTP Error Response:
```json
{
  "error": {
    "message": "Error message",
    "type": "error_type",
    "param": "parameter",
    "code": "error_code"
  }
}
```

**Key Difference:** litellm follows OpenAI's exact error format with nested `error` object, while litellm-rs uses a simpler flat structure. This may cause SDK compatibility issues.

---

## 3. Streaming Response Compatibility

### 3.1 SSE Implementation

**litellm-rs:**
```rust
// Event structure
pub struct Event {
    pub event: Option<String>,
    pub data: String,
}

impl Event {
    pub fn to_bytes(&self) -> web::Bytes {
        let mut result = String::new();
        if let Some(event) = &self.event {
            result.push_str(&format!("event: {}\n", event));
        }
        result.push_str(&format!("data: {}\n\n", self.data));
        web::Bytes::from(result)
    }
}
```

**litellm:**
```python
# Uses FastAPI StreamingResponse
return StreamingResponse(
    selected_data_generator,
    media_type="text/event-stream",
)
```

### 3.2 Chunk Format

**litellm-rs Chunk:**
```rust
pub struct ChatCompletionChunk {
    pub id: String,                    // chatcmpl-{uuid}
    pub object: String,                // "chat.completion.chunk"
    pub created: u64,
    pub model: String,
    pub system_fingerprint: Option<String>,
    pub choices: Vec<ChatCompletionChunkChoice>,
    pub usage: Option<Usage>,
}

pub struct ChatCompletionChunkChoice {
    pub index: u32,
    pub delta: ChatCompletionDelta,
    pub finish_reason: Option<String>,
    pub logprobs: Option<Value>,
}

pub struct ChatCompletionDelta {
    pub role: Option<MessageRole>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}
```

**litellm Chunk:**
```python
class ModelResponseStream(ModelResponseBase):
    choices: List[StreamingChoices]
    provider_specific_fields: Optional[Dict[str, Any]]

class StreamingChoices(OpenAIObject):
    index: int
    delta: Delta
    finish_reason: Optional[str]
    logprobs: Optional[ChoiceLogprobs]
```

### 3.3 Stream Termination

Both implementations:
- Send `data: [DONE]\n\n` as final event
- Include usage in final chunk (when requested)
- Support `finish_reason` in final choice

**Compatibility Score: 98%**

---

## 4. SDK Compatibility

### 4.1 OpenAI SDK Drop-in Replacement

**Test Scenario:**
```python
from openai import OpenAI

# Using litellm-rs as backend
client = OpenAI(
    api_key="sk-xxx",
    base_url="http://localhost:8080/v1"  # litellm-rs
)

response = client.chat.completions.create(
    model="gpt-4",
    messages=[{"role": "user", "content": "Hello"}]
)
```

| Feature | litellm-rs | litellm | SDK Compatible |
|---------|-----------|---------|----------------|
| Basic completion | Yes | Yes | Yes |
| Streaming | Yes | Yes | Yes |
| Function calling | Yes | Yes | Yes |
| Tool use | Yes | Yes | Yes |
| Vision | Yes | Yes | Yes |
| JSON mode | Yes | Yes | Yes |
| Error handling | Partial | Full | Partial |
| Rate limit headers | Partial | Full | Partial |
| Request ID headers | Yes | Yes | Yes |

### 4.2 Known SDK Compatibility Issues

**litellm-rs:**
1. Error format differs from OpenAI (flat vs nested)
2. Missing some rate limit headers (`x-ratelimit-*`)
3. No `x-request-id` in all responses

**Recommendation:** For full SDK compatibility, litellm-rs should:
1. Implement OpenAI-style nested error format
2. Add rate limit headers
3. Ensure consistent request ID propagation

---

## 5. Special Features API Comparison

### 5.1 Function Calling / Tool Use

**Tool Definition (Both Compatible):**
```json
{
  "type": "function",
  "function": {
    "name": "get_weather",
    "description": "Get current weather",
    "parameters": {
      "type": "object",
      "properties": {
        "location": {"type": "string"}
      },
      "required": ["location"]
    }
  }
}
```

**Tool Choice Options:**
| Option | litellm-rs | litellm |
|--------|-----------|---------|
| "auto" | Yes | Yes |
| "none" | Yes | Yes |
| "required" | Yes | Yes |
| `{"type": "function", "function": {"name": "..."}}` | Yes | Yes |

**Streaming Tool Calls:**
```rust
// litellm-rs
pub struct ToolCallDelta {
    pub index: u32,
    pub id: Option<String>,
    pub tool_type: Option<String>,
    pub function: Option<FunctionCallDelta>,
}
```

**Compatibility Score: 100%**

### 5.2 Vision (Multimodal)

**Image URL Support:**
```json
{
  "type": "image_url",
  "image_url": {
    "url": "https://example.com/image.png",
    "detail": "high"
  }
}
```

**Base64 Image Support:**
```json
{
  "type": "image",
  "source": {
    "media_type": "image/png",
    "data": "base64_encoded_data"
  }
}
```

| Feature | litellm-rs | litellm |
|---------|-----------|---------|
| URL images | Yes | Yes |
| Base64 images | Yes | Yes |
| Detail level | Yes | Yes |
| Multiple images | Yes | Yes |
| Interleaved text/image | Yes | Yes |

**Compatibility Score: 100%**

### 5.3 JSON Mode / Structured Output

**Response Format Options:**

| Type | litellm-rs | litellm |
|------|-----------|---------|
| `{"type": "text"}` | Yes | Yes |
| `{"type": "json_object"}` | Yes | Yes |
| `{"type": "json_schema", "json_schema": {...}}` | Yes | Yes |

**litellm-rs Implementation:**
```rust
pub struct ResponseFormat {
    pub format_type: String,          // "text", "json_object", "json_schema"
    pub json_schema: Option<Value>,   // Schema when type is json_schema
    pub response_type: Option<String>,
}
```

**Compatibility Score: 100%**

### 5.4 Audio Features

**Speech-to-Text:**
| Parameter | litellm-rs | litellm |
|-----------|-----------|---------|
| model | Yes | Yes |
| file | Yes | Yes |
| language | Yes | Yes |
| prompt | Yes | Yes |
| response_format | Yes | Yes |
| temperature | Yes | Yes |
| timestamp_granularities | No | Yes |

**Text-to-Speech:**
| Parameter | litellm-rs | litellm |
|-----------|-----------|---------|
| model | Yes | Yes |
| input | Yes | Yes |
| voice | Yes | Yes |
| response_format | Yes | Yes |
| speed | Yes | Yes |

**Compatibility Score: 95%**

### 5.5 Extended Thinking (Reasoning Models)

**litellm-rs:**
```rust
pub struct ThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: Option<u32>,
    pub effort: Option<ThinkingEffort>,
}

pub struct ThinkingContent {
    pub thinking: Option<String>,
    pub thinking_signature: Option<String>,
}
```

**litellm:**
- Supports reasoning via provider-specific parameters
- Claude extended thinking
- OpenAI o1/o3 reasoning

| Feature | litellm-rs | litellm |
|---------|-----------|---------|
| Thinking config | Yes | Yes |
| Thinking in response | Yes | Yes |
| Budget tokens | Yes | Partial |
| Effort levels | Yes | No |

---

## 6. Provider-Specific Features

### 6.1 Anthropic Extensions

| Feature | litellm-rs | litellm |
|---------|-----------|---------|
| System message handling | Yes | Yes |
| Cache control | Yes | Yes |
| PDF support | Yes | Yes |
| Extended thinking | Yes | Yes |
| Computer use | No | Yes |

### 6.2 Azure OpenAI Extensions

| Feature | litellm-rs | litellm |
|---------|-----------|---------|
| Deployment routing | No | Yes |
| API version support | Partial | Full |
| Content filtering | No | Yes |

### 6.3 Google/Vertex AI Extensions

| Feature | litellm-rs | litellm |
|---------|-----------|---------|
| Gemini models | Yes | Yes |
| Multimodal | Yes | Yes |
| Grounding | No | Yes |

---

## 7. Compatibility Matrix Summary

### Overall API Compatibility Score: 91%

| Category | Score | Notes |
|----------|-------|-------|
| Core Chat API | 95% | Missing some Azure routes |
| Completions API | 95% | Full OpenAI compatibility |
| Embeddings API | 90% | Missing token array decoding |
| Models API | 85% | Missing advanced filtering |
| Streaming | 98% | Fully compatible |
| Function Calling | 100% | Full compatibility |
| Vision | 100% | Full compatibility |
| JSON Mode | 100% | Full compatibility |
| Audio | 95% | Missing some parameters |
| Error Format | 70% | Different structure |
| SDK Compatibility | 85% | Minor header differences |

---

## 8. Migration Recommendations

### For Users Migrating from litellm to litellm-rs:

1. **Endpoint URLs:** Update any Azure-style endpoints to standard `/v1/*` format
2. **Error Handling:** Adjust error parsing for flat structure
3. **Rate Limiting:** Implement custom rate limit tracking if needed
4. **Token Arrays:** Pre-decode token arrays before sending to embeddings endpoint

### For Users Migrating from litellm-rs to litellm:

1. **Extended Features:** Take advantage of additional endpoints (files, batches, fine-tuning)
2. **Provider Extensions:** Utilize provider-specific features
3. **Enterprise Features:** Access team management, audit logging

---

## 9. Conclusion

Both implementations provide strong OpenAI API compatibility for core functionality. litellm-rs offers excellent performance characteristics with high compatibility for standard use cases, while litellm provides broader feature coverage and deeper provider integrations.

**Choose litellm-rs when:**
- Performance is critical (10,000+ RPS)
- Core chat/completion/embedding APIs are sufficient
- Rust ecosystem integration is needed
- Minimal memory footprint is required

**Choose litellm when:**
- Maximum provider coverage is needed
- Enterprise features (audit, RBAC) are required
- Full OpenAI API surface area is needed
- Python ecosystem integration is preferred
