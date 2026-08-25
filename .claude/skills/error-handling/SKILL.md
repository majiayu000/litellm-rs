---
name: error-handling
description: LiteLLM-RS Error Handling Architecture. Covers two-tier error hierarchy, ProviderError factory methods, HTTP status mapping, retry logic, and error context preservation. Use when designing error types or enums, creating ProviderError instances via factory methods, mapping HTTP statuses to typed errors, deciding retryability and backoff, or preserving error context.
---

# Error Handling Architecture Guide

## Two-Tier Error Hierarchy

LiteLLM-RS uses a two-tier error architecture optimized for 66+ providers:

```
┌────────────────────────────────────────────────────────┐
│                    Gateway Layer                        │
│  LiteLLMError (core/types/errors/litellm.rs)          │
│  - 15 variants for gateway-level errors                │
│  - Routing, configuration, authentication errors       │
└────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────┐
│                   Provider Layer                        │
│  ProviderError (core/providers/unified_provider.rs)   │
│  - 24 variants for provider-specific errors            │
│  - Each variant includes provider: &'static str        │
│  - Rich factory methods for error creation             │
└────────────────────────────────────────────────────────┘
```

---

## HTTP Status Mapping

### Standard Mapping Pattern

```rust
impl MyProvider {
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        match status {
            // Authentication errors
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            403 => ProviderError::authentication(PROVIDER_NAME, "Access forbidden"),

            // Not found errors
            404 => ProviderError::model_not_found(PROVIDER_NAME, body),

            // Client errors
            400 => ProviderError::invalid_request(PROVIDER_NAME, body),
            422 => ProviderError::invalid_request(PROVIDER_NAME, "Unprocessable entity"),

            // Rate limiting
            429 => ProviderError::rate_limit(PROVIDER_NAME, self.parse_retry_after(body)),

            // Server errors
            500 => ProviderError::provider_unavailable(PROVIDER_NAME, "Internal server error"),
            502 => ProviderError::provider_unavailable(PROVIDER_NAME, "Bad gateway"),
            503 => ProviderError::provider_unavailable(PROVIDER_NAME, "Service unavailable"),
            504 => ProviderError::timeout(PROVIDER_NAME, "Gateway timeout"),

            // Default
            _ => ProviderError::api_error(PROVIDER_NAME, status, body),
        }
    }

    fn parse_retry_after(&self, body: &str) -> Option<u64> {
        // Parse retry-after from response headers or body
        serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| v.get("retry_after"))
            .and_then(|v| v.as_u64())
    }
}
```

### ErrorMapper Trait

```rust
// src/core/traits/error_mapper/trait_def.rs

pub trait ErrorMapper: Send + Sync {
    type Error;

    fn map_http_error(&self, status: u16, body: &str) -> Self::Error;
    fn map_network_error(&self, error: reqwest::Error) -> Self::Error;
    fn map_parse_error(&self, error: serde_json::Error) -> Self::Error;
}

// Generic implementation for ProviderError
pub struct GenericErrorMapper;

impl ErrorMapper for GenericErrorMapper {
    type Error = ProviderError;

    fn map_http_error(&self, status: u16, body: &str) -> Self::Error {
        match status {
            401 | 403 => ProviderError::authentication("generic", body),
            404 => ProviderError::model_not_found("generic", body),
            429 => ProviderError::rate_limit("generic", None),
            400 | 422 => ProviderError::invalid_request("generic", body),
            500..=599 => ProviderError::provider_unavailable("generic", body),
            _ => ProviderError::api_error("generic", status, body),
        }
    }

    fn map_network_error(&self, error: reqwest::Error) -> Self::Error {
        if error.is_timeout() {
            ProviderError::timeout("generic", error.to_string())
        } else if error.is_connect() {
            ProviderError::network("generic", error.to_string())
        } else {
            ProviderError::network("generic", error.to_string())
        }
    }

    fn map_parse_error(&self, error: serde_json::Error) -> Self::Error {
        ProviderError::response_parsing("generic", error.to_string())
    }
}
```

---

## Best Practices

### 1. Always Use Factory Methods

```rust
// Good
ProviderError::authentication(PROVIDER_NAME, "Invalid API key")

// Bad - verbose and error-prone
ProviderError::Authentication {
    provider: PROVIDER_NAME,
    message: "Invalid API key".to_string(),
}
```

### 2. Include Provider Name

```rust
// Good - error clearly identifies source
ProviderError::network("openai", "Connection refused")

// Bad - unclear which provider failed
ProviderError::network("unknown", "Connection refused")
```

### 3. Preserve Error Context

```rust
// Good - preserves original error
self.pool_manager.execute_request(&url, method, headers, body)
    .await
    .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?

// Bad - loses original error
self.pool_manager.execute_request(&url, method, headers, body)
    .await
    .map_err(|_| ProviderError::network(PROVIDER_NAME, "Request failed"))?
```

### 4. Use Specific Error Types

```rust
// Good - specific error type
if response.status() == 429 {
    return Err(ProviderError::rate_limit(PROVIDER_NAME, retry_after));
}

// Bad - generic error loses information
if !response.status().is_success() {
    return Err(ProviderError::api_error(PROVIDER_NAME, status, "Failed"));
}
```

### 5. Handle All Error Variants in Match

```rust
// Good - exhaustive handling
match error {
    ProviderError::RateLimit { retry_after, .. } => {
        if let Some(delay) = retry_after {
            tokio::time::sleep(Duration::from_secs(delay)).await;
        }
        // Retry...
    }
    ProviderError::Authentication { .. } => {
        // Don't retry, return immediately
        return Err(error);
    }
    e if e.is_retryable() => {
        // Retry with backoff
    }
    _ => return Err(error),
}
```

---

## HTTP to ProviderError Mapping Reference

| HTTP Status | ProviderError Variant | Retryable |
|-------------|----------------------|-----------|
| 400 | `InvalidRequest` | No |
| 401 | `Authentication` | No |
| 403 | `Authentication` | No |
| 404 | `ModelNotFound` | No |
| 408 | `Timeout` | Yes |
| 422 | `InvalidRequest` | No |
| 429 | `RateLimit` | Yes |
| 500 | `ProviderUnavailable` | Yes |
| 502 | `ProviderUnavailable` | Yes |
| 503 | `ProviderUnavailable` | Yes |
| 504 | `Timeout` | Yes |

## References

- [reference/provider-error-variants.md](reference/provider-error-variants.md) — Full ProviderError enum: all 24 variants with display strings and fields.
- [reference/factory-methods.md](reference/factory-methods.md) — Basic and enhanced ProviderError factory constructors.
- [reference/retry-logic.md](reference/retry-logic.md) — Retryable-error detection, retry_after hints, fallback decisions, and exponential backoff.
- [reference/litellm-error-gateway.md](reference/litellm-error-gateway.md) — Gateway-level LiteLLMError enum with its 15 variants.
- [reference/error-context-preservation.md](reference/error-context-preservation.md) — Attaching anyhow context to errors and displaying error chains.
