## LiteLLMError (Gateway Level)

```rust
// src/core/types/errors/litellm.rs

#[derive(Debug, thiserror::Error)]
pub enum LiteLLMError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Routing error: {0}")]
    Routing(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authorization error: {0}")]
    Authorization(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Rate limit error: {0}")]
    RateLimit(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Bad gateway: {0}")]
    BadGateway(String),

    #[error("Cancelled: {0}")]
    Cancelled(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
```
