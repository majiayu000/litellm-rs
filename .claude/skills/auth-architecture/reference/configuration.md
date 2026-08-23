## Configuration

### Auth configuration

Flat `AuthConfig` under the top-level `auth:` key — there are no nested
`auth.jwt.*` / `auth.api_key.*` / `auth.rate_limiting.*` blocks, and unknown
fields are rejected (`#[serde(deny_unknown_fields)]`,
`src/config/models/auth.rs`):

```yaml
auth:
  enable_jwt: true            # default false
  enable_api_key: true        # default true
  jwt_secret: "${LITELLM_JWT_SECRET}"  # required when enable_jwt; see validation
  jwt_expiration: 86400       # seconds; default 86400 (24 h)
  api_key_header: "Authorization"      # custom header for API keys
  # api_key_hmac_secret: "..."         # enables HMAC-SHA256 key hashing
  allow_anonymous: false      # dev-only escape hatch
  rbac:
    enabled: false            # default false
    default_role: "user"
    admin_roles: ["admin", "superuser"]
```

Field semantics (`src/config/models/auth.rs`, defaults in
`src/config/models/defaults.rs`):

- `enable_jwt` / `enable_api_key` — method switches checked by `AuthMiddleware`.
- `jwt_secret` — HS256 signing secret.
- `api_key_header` — header scanned for API keys after the `Authorization`
  forms; defaults to `Authorization` (which the extractor ignores as a plain
  header, leaving `X-API-Key` as fallback transport).
- `api_key_hmac_secret` — when set, API keys hash with HMAC-SHA256 instead of
  plain SHA-256; strongly recommended in production.
- `allow_anonymous` — only meaningful when both methods are disabled;
  validation and middleware otherwise reject that combination (fail closed).

### Validation rules

`AuthConfig::validate()` rejects:

- both auth methods disabled without explicit `allow_anonymous: true`;
- empty `jwt_secret` or secrets shorter than 32 bytes (256-bit) when JWT is on
  (unresolved `${ENV}` references usually fail here);
- placeholder secrets: `your-secret-key`, `change-me`, or values containing
  `yoursecretkey`, `changeme`, `replacewith`, or `placeholder` (case/space
  insensitive);
- secrets with lowercase but no uppercase letters;
- `jwt_expiration` below 300 s or above 30 days;
- empty `api_key_header` while API-key auth is enabled.

`is_production_ready()` is true iff at least one method is enabled.
`warn_insecure_config()` logs warnings for disabled auth and for API-key auth
without `api_key_hmac_secret`.

### Rate limiting

Rate limiting is a separate top-level `rate_limit:` section
(`src/config/models/rate_limit.rs`) — see rate-limiting.md for the full YAML
surface. It is not part of `AuthConfig`.
