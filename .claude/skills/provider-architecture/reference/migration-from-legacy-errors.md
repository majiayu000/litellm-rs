## Migration from Legacy Error Types

Per-provider error enums have been removed from the codebase. `LLMProvider` has no
`Error` associated type — every trait method returns the unified
`crate::core::providers::unified_provider::ProviderError` directly.

Historical type names survive as plain aliases, so existing call sites keep compiling:

```rust
// src/core/providers/anthropic/error.rs
pub type AnthropicError = ProviderError;

// Same pattern elsewhere:
// GeminiError, CohereError, OllamaError, MistralError, VertexAIError,
// GitHubError, GitHubCopilotError, LlamaError
```

### Old Variant Shapes -> Unified Factory Methods

Legacy enums are gone, so the left column below describes their historical shapes
(illustrative), and the right column is what to write today:

| Legacy Pattern | Unified Pattern |
|----------------|-----------------|
| `XxxError::AuthenticationError(msg)` | `ProviderError::authentication("xxx", msg)` |
| `XxxError::RateLimitError { retry_after }` | `ProviderError::rate_limit("xxx", retry_after)` |
| `XxxError::ModelNotFoundError(model)` | `ProviderError::model_not_found("xxx", model)` |
| `XxxError::InvalidRequestError(msg)` | `ProviderError::invalid_request("xxx", msg)` |
| `XxxError::NetworkError(msg)` | `ProviderError::network("xxx", msg)` |
| `XxxError::TimeoutError(msg)` | `ProviderError::timeout("xxx", msg)` |
| `XxxError::ApiError(status, msg)` | `ProviderError::api_error("xxx", status, msg)` |

All factory methods take the static provider name first (`&'static str`; return
`error_provider_name()` from the provider for it).

### Where Provider-Specific Behavior Lives Now

Response/error-body parsing moved into error mappers rather than error enums:

- `OpenAIErrorMapper`, `AnthropicErrorMapper` — `core::traits::error_mapper::implementations`
  (e.g. `AnthropicErrorMapper::from_http_status(status, body)`,
  `from_api_response(&serde_json::Value)`)
- `GenericErrorMapper` (aliased `DefaultErrorMapper`) — `core::traits::error_mapper::types`,
  the generic HTTP-status fallback most providers return from `get_error_mapper()`
