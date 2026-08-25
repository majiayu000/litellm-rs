## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing credentials")]
    MissingCredentials,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Expired credentials")]
    ExpiredCredentials,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Token creation failed: {0}")]
    TokenCreation(String),

    #[error("Token validation failed: {0}")]
    TokenValidation(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Key not found")]
    KeyNotFound,

    #[error("Missing auth context")]
    MissingContext,

    #[error("Internal error: {0}")]
    Internal(String),
}
```
