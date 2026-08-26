## Error Types

There is no `AuthError` enum. All auth errors flow through the shared
`GatewayError` enum (`src/utils/error/gateway_error/types.rs`) and its
constructor helpers (`helpers.rs`). The variants the auth stack uses:

```rust
pub enum GatewayError {
    Auth(String),        // 401 AUTH_ERROR / authentication_error
    Forbidden(String),   // 403 FORBIDDEN / permission_error
    RateLimit {          // 429 RATE_LIMIT_EXCEEDED
        message: String,
        retry_after: Option<u64>,
        rpm_limit: Option<u32>,
        tpm_limit: Option<u32>,
    },
    Validation(String),  // 400 — e.g. invalid API-key name or permission
    Storage(String),     // 503 — database/cache failures behind auth
    Internal(String),    // 500 — infrastructure failures
    // ... plus Config, Serialization, Timeout, NotFound, Conflict, etc.
}
```

Constructors: `GatewayError::auth("...")`, `GatewayError::forbidden("...")`,
`GatewayError::rate_limit("...")`, `GatewayError::validation("...")`.
`From<jsonwebtoken::errors::Error>` maps any JWT failure to
`GatewayError::Auth("JWT error: ...")`, preserving the error kind
(e.g. `ExpiredSignature`) in the message.

### HTTP mapping

`gateway_http_error_facts` (`src/utils/error/gateway_error/http_mapping.rs`)
assigns status, code, and OpenAI error type: `Auth` → 401
`authentication_error`, `Forbidden` → 403 `permission_error`, `RateLimit` → 429
`rate_limit_exceeded`. Response rendering (`response.rs`) emits `Retry-After`,
`X-RateLimit-Limit-Requests`, and `X-RateLimit-Limit-Tokens` headers when the
corresponding fields are set.

### Rejection vs failure

Auth code distinguishes two channels:

- **Semantic rejection** — `AuthResult { success: false, error: Some(msg) }`
  from `AuthSystem::authenticate` for bad tokens, unknown users, inactive
  accounts, disabled sessions. The middleware turns these into 401 with the
  message. `ApiKeyHandler::verify_key` similarly returns `Ok(None)` for
  unknown/inactive/expired keys, and `verify_key_detailed` returns
  `ApiKeyVerification { is_valid: false, invalid_reason: Some(...) }`.
- **Infrastructure failure** — `Err(GatewayError)` (storage down, config
  broken). The middleware responds with a generic 500
  (`AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE` = "Authentication service
  temporarily unavailable") that never leaks internal details.
