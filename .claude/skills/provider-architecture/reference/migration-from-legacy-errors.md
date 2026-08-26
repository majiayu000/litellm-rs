## Migration from Legacy Errors

If migrating from provider-specific error types:

```rust
// Before
use super::error::FireworksError;
impl LLMProvider for FireworksProvider {
    type Error = FireworksError;
}
Err(FireworksError::AuthenticationError("Invalid key".into()))

// After
use crate::core::providers::unified_provider::ProviderError;
impl LLMProvider for FireworksProvider {
    type Error = ProviderError;
}
Err(ProviderError::authentication("fireworks", "Invalid key"))
```

### Error Mapping Reference

| Legacy Pattern | Unified Pattern |
|----------------|-----------------|
| `XxxError::AuthenticationError(msg)` | `ProviderError::authentication("xxx", msg)` |
| `XxxError::RateLimitError { retry_after }` | `ProviderError::rate_limit("xxx", retry_after)` |
| `XxxError::ModelNotFoundError(model)` | `ProviderError::model_not_found("xxx", model)` |
| `XxxError::InvalidRequestError(msg)` | `ProviderError::invalid_request("xxx", msg)` |
| `XxxError::NetworkError(msg)` | `ProviderError::network("xxx", msg)` |
| `XxxError::TimeoutError(msg)` | `ProviderError::timeout("xxx", msg)` |
| `XxxError::ApiError(status, msg)` | `ProviderError::api_error("xxx", status, msg)` |
