## LiteLLMError (Gateway Level)

`LiteLLMError` is not an enum. It is a type alias for the canonical gateway
error type:

```rust
// src/core/types/errors/litellm.rs
pub type LiteLLMError = crate::utils::error::gateway_error::GatewayError;
pub type LiteLLMResult<T> = Result<T, LiteLLMError>;
```

The canonical enum lives in `src/utils/error/gateway_error/types.rs`
and has 18 variants:

```rust
#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Provider error: {0}")]
    Provider(ProviderError),

    #[error("Rate limit exceeded: {message}")]
    RateLimit {
        message: String,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
    },

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Timeout error: {0}")]
    Timeout(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}
```

Additional `From` conversions are implemented manually in the same file:
`serde_json::Error` and `serde_yml::Error` become `Serialization`;
`jsonwebtoken::errors::Error` becomes `Auth`; `redis::RedisError` (feature
`redis`) and `sea_orm::DbErr` (feature `storage`) become `Storage`.
