## Security Best Practices

### 1. JWT secret policy

Generate secrets with `generate_jwt_secret()` (`src/utils/auth/crypto/keys.rs`):
64 random alphanumeric characters. `AuthConfig::validate()` enforces the rest:
at least 32 bytes (256-bit for HS256), uppercase required when lowercase is
present, and no placeholder values (`your-secret-key`, `change-me`, or values
containing `yoursecretkey` / `changeme` / `replacewith` / `placeholder`).
Secrets are loaded from environment variables via `${LITELLM_JWT_SECRET}`
interpolation in `config/gateway.yaml` — never committed in plaintext; an
unresolved reference fails validation by length.

### 2. API key hashing

API keys are hashed, not encrypted and never stored raw:

- With `auth.api_key_hmac_secret` set: HMAC-SHA256(secret, key) — recommended,
  since plain hashes of high-entropy-looking keys are brute-forceable if the
  database leaks.
- Without it: plain SHA-256 fallback, and `warn_insecure_config()` logs a
  warning at startup.

Verification re-hashes the presented key and looks up by hash; the raw key is
returned exactly once by `create_key` / `create_key_with_options`. The stored
`key_prefix` (`extract_api_key_prefix`, e.g. `gw-a...mnop`) is display metadata
only — never used for lookup.

User passwords use a different primitive: Argon2 (`hash_password` /
`verify_password` in `src/utils/auth/crypto/password.rs`). Do not swap the two.

### 3. Fail closed

Both auth methods disabled is rejected at validation unless
`allow_anonymous: true` is set explicitly, and `AuthMiddleware` re-checks the
combination per request as defense in depth. Keep `allow_anonymous`
development-only.

### 4. Credential redaction

Credential-bearing types implement manual `Debug`: `AuthMethod` prints
`Jwt("[REDACTED]")` / `ApiKey("[REDACTED]")` / `Session("[REDACTED]")`;
`JwtHandler` redacts its encoding/decoding keys; `AuthConfig` redacts
`jwt_secret` and `api_key_hmac_secret`. Preserve these impls when touching
those types. Request-context building also strips `authorization` and
`x-api-key` headers before storing header maps.

### 5. Audit logging

Audit events flow through `src/core/audit/`: `AuditMiddleware` records request
lifecycles into `AuditEvent`s via `AuditLogger`; user actions include
`UserAction::Login`, `Logout`, `AuthFailed`, `ApiKeyCreated`, `ApiKeyRevoked`.
After successful authentication, `record_authenticated_principal`
(`src/core/audit/middleware.rs`) attaches user/api-key/team identity to the
in-flight audit record so 403 responses are attributable.

### 6. Key lifecycle

There is no in-place rotation with delayed deletion. Use
`src/auth/api_key/management.rs`:

- `revoke_key(key_id)` — deactivate immediately.
- `regenerate_key(key_id)` — issue a new raw key for the same key record
  (closest analog to rotation); returns `(ApiKey, String)` where the string is
  the only time the raw key is visible.
- `update_expiration`, `cleanup_expired_keys` — expiry hygiene.

### 7. Brute-force resistance

Failed attempts are tracked per client (IP + credential hash) by
`AuthRateLimiter` with exponential lockout, and rejected auth attempts also
consume gateway rate-limit capacity so lockout evasion cannot bypass limiting.
Lockout responses expose only "try again in N seconds", never which
credential failed.
