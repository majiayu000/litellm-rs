## Contents

- The Validate Trait
- Validation Module Layout
- When Validation Runs
- Key Rules by Section

---

## The Validate Trait

Defined in `src/config/validation/trait_def.rs`:

```rust
pub trait Validate {
    fn validate(&self) -> Result<(), String>;
}
```

Implementations return `Result<(), String>` — plain error strings, not `ConfigError`. `Config::validate` in `src/config/mod.rs` wraps any failure into `GatewayError::Config("Gateway config error: ...")`.

## Validation Module Layout

All rules live in `src/config/validation/`, split by config section:

| File | Covers |
|------|--------|
| `trait_def.rs` | the `Validate` trait |
| `config_validators.rs` | `GatewayConfig`, `ServerConfig`, `ProviderConfig`, model aliases, pricing |
| `router_validators.rs` | routing strategy, circuit breaker, load balancer |
| `storage_validators.rs` | database / Redis / vector DB |
| `auth_validators.rs` | JWT, API key, RBAC |
| `monitoring_validators.rs` | metrics, tracing, health |
| `cache_validators.rs` | cache and rate limit |
| `enterprise_validators.rs` | enterprise/SSO/audit settings |
| `ssrf.rs` | SSRF checks for provider endpoints (`validate_url_against_ssrf`) |

Add new rules here rather than scattering checks through handler code.

## When Validation Runs

Validation runs at startup only; there is no reload or periodic revalidation.

- End of `Config::from_file` and `Config::from_env` (`src/config/mod.rs`).
- Again after CLI `--host` / `--port` overrides are applied in `src/main.rs`.
- After validation of a file-loaded config, `warn_insecure_config(&config.auth)` (`src/config/models/auth.rs`) logs warnings for insecure-but-valid setups.
- `gateway validate-config` loads and validates without starting the server.

Note: `Config::default()` intentionally fails validation (no providers), so an all-defaults gateway refuses to start.

## Key Rules by Section

### GatewayConfig

`impl Validate for GatewayConfig` in `src/config/validation/config_validators.rs`:

- `schema_version` non-empty and one of the supported versions (`["1.0"]`).
- Server and CORS validate.
- At least one provider entry must be configured; names must be unique.
- Every provider passes its own `Validate`.
- Model aliases: non-empty trimmed keys/targets, no self-targeting, and no cycles — checked graph-wide by `validate_model_alias_map`, independent of provider model lists.
- Router, storage, auth, monitoring, cache, rate_limit, guardrails, ip_access, enterprise, and pricing validators all run; any failure aborts.

### ServerConfig

- Host must be non-empty.
- Port must be non-zero and >= 1024 outside tests (non-root binding).
- `workers`, when set, must be 1..=1000 (`None` means auto-detect CPU count at runtime).
- Timeout must be > 0 and <= 3600 seconds.
- `max_body_size` must be > 0 and <= 100 MiB.
- TLS, when present, requires non-empty cert and key paths.

### ProviderConfig

- Name and type non-empty; the type must resolve via the provider factory/catalog (`is_provider_selector_supported`).
- API key required unless the registry marks the selector as keyless.
- Weight in (0, 100]; timeout in (0, 300] seconds; rpm > 0; tpm > 0.
- `endpoint_access` may only appear as a top-level field, never inside `settings`; `private_network` access generally requires an explicit base URL and a policy-wired provider type.
- Base URL, when configured, must parse as http/https and pass endpoint-policy and SSRF checks.

### AuthConfig

`AuthConfig::validate` in `src/config/models/auth.rs`:

- Both auth methods disabled is rejected unless `allow_anonymous: true` is set explicitly.
- With `enable_jwt: true`: `jwt_secret` must be at least 32 bytes with mixed-case letters, and known placeholders ("change-me", "your-secret-key", ...) are rejected.
- `jwt_expiration` must be between 300 seconds and 30 days.
- With API-key auth enabled, `api_key_header` must be non-empty.

### Strict Deserialization

`GatewayConfig` and many nested gateway-facing models carry
`#[serde(deny_unknown_fields)]`. Unknown keys on those concrete structs fail at
parse time — before validation ever runs — so typos surface as parse errors
naming the field. The guarantee is per struct, not recursive by definition:
`providers[].settings` accepts arbitrary keys, and some provider/callback
backend payload structs omit `deny_unknown_fields` and may ignore unknown keys.
Keep `config/gateway.yaml.example` in sync when adding fields; tests assert the
example parses against the current schema.
