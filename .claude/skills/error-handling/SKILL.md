---
name: error-handling
description: LiteLLM-RS Error Handling Architecture. Covers two-tier error hierarchy, ProviderError factory methods, HTTP status mapping, retry logic, and error context preservation. Use when designing error types or enums, creating ProviderError instances via factory methods, mapping HTTP statuses to typed errors, deciding retryability and backoff, or preserving error context.
---

# Error Handling Architecture Guide

## Two-Tier Error Hierarchy

LiteLLM-RS uses a two-tier error architecture spanning its provider catalog:

```
┌────────────────────────────────────────────────────────┐
│                    Gateway Layer                        │
│  LiteLLMError (core/types/errors/litellm.rs)          │
│  - Type alias for GatewayError                          │
│    (src/utils/error/gateway_error/types.rs)             │
│  - 18 variants for gateway-level errors                 │
└────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────┐
│                   Provider Layer                        │
│  ProviderError                                          │
│  (src/core/providers/unified_provider_error.rs;        │
│   exported as core::providers::ProviderError)           │
│  - 24 variants for provider-specific errors            │
│  - Each variant includes provider: &'static str        │
│  - Rich factory methods for error creation             │
└────────────────────────────────────────────────────────┘
```

---

## HTTP Status Mapping

### Standard Mapping Pattern

Most providers share one canonical status-to-error mapping,
`default_http_error_mapper` in `src/core/providers/unified_provider_http_mapping.rs`:

```rust
pub fn default_http_error_mapper(
    provider: &'static str,
    status_code: u16,
    response_body: &str,
) -> ProviderError {
    match status_code {
        400 => {
            let message = parse_error_message_from_body(response_body)
                .unwrap_or_else(|| response_body.to_string());
            ProviderError::invalid_request(provider, message)
        }
        401 => ProviderError::authentication(provider, "Invalid API key"),
        403 => ProviderError::authentication(provider, "Permission denied"),
        404 => ProviderError::model_not_found(provider, "Model not found"),
        429 => {
            let retry_after =
                crate::core::providers::shared::parse_retry_after_from_body(response_body);
            ProviderError::rate_limit(provider, retry_after)
        }
        500..=599 => ProviderError::api_error(provider, status_code, response_body),
        _ => ProviderError::api_error(provider, status_code, response_body),
    }
}
```

Providers needing extra special cases call `extended_http_error_mapper`
(same file), which additionally maps 402 to `quota_exceeded`, 408/504 to
`timeout`, 413 to `context_length_exceeded`, and 502/503 to
`provider_unavailable`.

### ErrorMapper Trait

```rust
// src/core/traits/error_mapper/trait_def.rs

pub trait ErrorMapper<E>: Send + Sync + 'static
where
    E: ProviderErrorTrait,
{
    // Required: map HTTP status + body to the provider's error type
    fn map_http_error(&self, status_code: u16, response_body: &str) -> E;

    // Default implementations
    fn map_json_error(&self, error_response: &serde_json::Value) -> E;
    fn map_network_error(&self, error: &dyn std::error::Error) -> E;
    fn map_parsing_error(&self, error: &dyn std::error::Error) -> E;
    fn map_timeout_error(&self, timeout_duration: std::time::Duration) -> E;
}
```

`GenericErrorMapper` (`src/core/traits/error_mapper/types.rs`, re-exported as
`DefaultErrorMapper`) implements `ErrorMapper<E>` for any `E: ProviderErrorTrait`:

```rust
impl<E> ErrorMapper<E> for GenericErrorMapper
where
    E: ProviderErrorTrait,
{
    fn map_http_error(&self, status_code: u16, response_body: &str) -> E {
        match status_code {
            400 => E::network_error("Bad Request: Invalid parameters"),
            401 => E::authentication_failed("Authentication failed: Invalid credentials"),
            403 => E::authentication_failed("Permission denied: Insufficient permissions"),
            404 => E::not_supported("Resource not found"),
            408 => E::network_error("Request timeout"),
            429 => E::rate_limited(None),
            500 => E::network_error("Internal server error"),
            502 => E::network_error("Bad gateway: Upstream server error"),
            503 => E::network_error("Service unavailable: Server overloaded"),
            504 => E::network_error("Gateway timeout: Upstream timeout"),
            _ => E::network_error(/* "HTTP Error {status}: {body or default}" */),
        }
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
// Good - execute_request already returns a classified ProviderError
let response = self.pool_manager
    .execute_request(&url, method, headers, body)
    .await?;

// Bad - erases an existing typed error by reclassifying it as Network
self.pool_manager.execute_request(&url, method, headers, body)
    .await
    .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?
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
    e if RetryPolicy
        .decide(&router_config, e, retry_context)
        .should_retry =>
    {
        // Retry per decision.delay (see reference/retry-logic.md).
        // Note: error.is_retryable() is deprecated since 0.6.0.
    }
    _ => return Err(error),
}
```

---

## HTTP to ProviderError Mapping Reference

Canonical behavior of `default_http_error_mapper`:

| HTTP Status | Result | Legacy-retryable |
|-------------|--------|------------------|
| 400 | `invalid_request` (message parsed from body) | No |
| 401 | `authentication` ("Invalid API key") | No |
| 403 | `authentication` ("Permission denied") | No |
| 404 | `model_not_found` | No |
| 429 | `rate_limit` (retry-after parsed from body when present) | Yes |
| 500-599 | `api_error(status)` | Yes |
| other | `api_error(status)` | No |

`extended_http_error_mapper` adds: 402→`quota_exceeded`, 408/504→`timeout`,
413→`context_length_exceeded`, 502/503→`provider_unavailable`.

## References

- [reference/provider-error-variants.md](reference/provider-error-variants.md) — Full ProviderError enum: all 24 variants with display strings and fields.
- [reference/factory-methods.md](reference/factory-methods.md) — Basic and enhanced ProviderError factory constructors.
- [reference/retry-logic.md](reference/retry-logic.md) — Legacy retry helpers, RetryPolicy decisions, retry hints, and exponential backoff.
- [reference/litellm-error-gateway.md](reference/litellm-error-gateway.md) — Gateway-level LiteLLMError type alias and the GatewayError enum with its 18 variants.
- [reference/error-context-preservation.md](reference/error-context-preservation.md) — Preserving error chains with ContextualError and std::error::Error::source.
