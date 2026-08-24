---
name: config-architecture
description: LiteLLM-RS Configuration Architecture. Covers YAML loading with ${VAR} environment substitution, strict serde config models, startup validation via the Validate trait, and the LITELLM_* env-only fallback. Use when changing config models or serde defaults, debugging env variable substitution failures, updating validation rules, altering config loading behavior, or troubleshooting gateway.yaml.
---

# Configuration Architecture Guide

## Overview

LiteLLM-RS loads a single YAML file into type-safe Rust models (`GatewayConfig` in `src/config/models/gateway.rs` and its children), substitutes `${VAR}` / `$VAR` tokens from the process environment during load, fills missing fields from serde defaults, then validates the whole tree once at startup.

There is **no hot reload**. Configuration is read once at startup; `src/utils/config/optimized.rs` states this explicitly ("Hot reload is not supported"). Restart the process to pick up changes. `AppState.config` (`src/server/state.rs`) is atomically swappable by explicit code paths, but nothing watches the file.

### Load Order

```
┌─────────────────────────────────────────────────────────────────┐
│  1. YAML file                                                   │
│     --config PATH (default: "config/gateway.yaml", src/main.rs) │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  2. Environment substitution (substitute_env_vars)              │
│     ${VAR} = required (hard error if unset)                     │
│     $VAR   = best-effort (left literal if unset)                │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  3. serde parse + defaults                                      │
│     #[serde(default ...)] fills omitted fields;                 │
│     deny_unknown_fields rejects keys on structs that opt in     │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  4. Validation (Validate trait)                                 │
│     runs inside Config::from_file / Config::from_env            │
│     failure aborts startup                                      │
└─────────────────────────────────────────────────────────────────┘
```

After loading, explicit `--host` / `--port` CLI values overwrite `server.host` / `server.port`, then the config is re-validated (`load_config_with_overrides` in `src/main.rs`).

Fallback rule: when the gateway starts **without** an explicit `--config` and the default file fails to load, `run_server_with_default_config_overrides` retries with `Config::from_env()`, which builds the config from `LITELLM_*` environment variables only (`src/config/models/gateway.rs::GatewayConfig::from_env`). An explicit `--config` path that fails is a hard error — no env fallback (`load_explicit_config` in `src/server/builder.rs`).

There is no search-path chain: no `/etc/litellm/...`, no `gateway.yaml` in cwd. The only default is the single constant `DEFAULT_CONFIG_PATH = "config/gateway.yaml"` relative to the process working directory (`src/main.rs`).

---

## YAML Configuration Structure

The canonical example is `config/gateway.yaml.example`; unit tests assert it parses against the current schema. Top-level sections of `GatewayConfig`:

```yaml
schema_version: "1.0"        # only "1.0" is accepted by validation

server:                      # host/port/workers/timeout/max_body_size/tls/cors/features/...
providers:                   # LIST of provider entries, not a map
  - name: "openai-primary"
    provider_type: "openai"
    api_key: "${OPENAI_API_KEY}"
    base_url: "https://api.openai.com/v1"
    models: ["gpt-4o"]
    priority: 0              # lower wins under priority_based routing

model_aliases:               # public alias -> canonical model or another alias
  production-chat: "gpt-4o"

router:                      # strategy, circuit_breaker, load_balancer
storage:                     # database, redis, vector_db
auth:                        # enable_jwt, enable_api_key, jwt_secret, rbac, allow_anonymous
monitoring:                  # metrics, tracing, health, callbacks
cache:                       # deterministic response cache (+ semantic_cache flag)
rate_limit:                  # token bucket strategy, redis_failure_mode
guardrails:                  # content safety; prompt-injection protection on by default
ip_access:                   # allowlist/blocklist; empty/default rules allow all
enterprise:                  # sso, audit_logging
pricing:                     # source, unpriced_model_policy
```

Unknown top-level fields are rejected at parse time
(`#[serde(deny_unknown_fields)]` on `GatewayConfig`), as are unknown keys in
the nested YAML models that carry the same attribute. This is not a universal
serde guarantee: `providers[].settings` is intentionally open-ended, and some
provider/callback backend payload structs do not deny unknown fields. There are
no top-level `logging`, `routing`, or `observability` sections — logging lives
under `monitoring.logging`, routing under `router`.

---

## Type-Safe Configuration Models

### Root Configuration

`src/config/models/gateway.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfig {
    pub schema_version: String,                    // default "1.0"
    pub server: ServerConfig,
    pub providers: Vec<ProviderConfig>,            // Vec, not a map
    pub model_aliases: HashMap<String, String>,    // default empty
    pub router: GatewayRouterConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
    pub monitoring: MonitoringConfig,
    pub cache: CacheConfig,                        // default
    pub rate_limit: RateLimitConfig,               // default
    pub guardrails: GuardrailConfig,               // default = enabled
    pub ip_access: IpAccessConfig,                 // default
    pub enterprise: EnterpriseConfig,              // default
    pub pricing: GatewayPricingConfig,             // default
}
```

### Server Configuration

`src/config/models/server.rs`:

```rust
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub host: String,                      // default "0.0.0.0"
    pub port: u16,                         // default 8000
    pub workers: Option<usize>,            // None -> CPU count at runtime
    pub max_connections: Option<usize>,
    pub timeout: u64,                      // seconds, default 30
    pub max_body_size: usize,              // bytes, default 10 MiB
    pub dev_mode: bool,                    // default false
    pub tls: Option<TlsConfig>,
    pub cors: CorsConfig,
    pub features: Vec<String>,
    pub trusted_proxies: Vec<String>,      // X-Forwarded-For trust list
    pub stream_idle_timeout: u64,          // SSE idle timeout, default 300s; 0 disables
}
```

Shared defaults live in `src/config/models/mod.rs`: `default_port()` returns `8000`, `default_timeout()` returns `30`, `default_max_body_size()` returns `10 * 1024 * 1024`. There is no `keep_alive`, `request_timeout`, or string-form `max_request_size` field.

### Provider Configuration

`src/config/models/provider.rs` — one entry per deployment, stored as `Vec<ProviderConfig>`:

```rust
#[serde(deny_unknown_fields)]
pub struct ProviderConfig {
    pub name: String,                      // must be unique across entries
    pub provider_type: String,             // e.g. "openai", "anthropic"
    pub api_key: String,                   // required unless registry skips key checks
    pub base_url: Option<String>,
    pub endpoint_access: ProviderEndpointAccess, // default PublicOnly; "private_network" opt-in
    pub api_version: Option<String>,
    pub organization: Option<String>,
    pub project: Option<String>,
    pub weight: f32,                       // default 1.0, valid range (0, 100]
    pub priority: u32,                     // default 0; lower wins in priority_based
    pub rpm: u32,                          // default 1000
    pub tpm: u32,                          // default 100_000
    pub max_concurrent_requests: u32,      // default 10
    pub timeout: u64,                      // seconds, default 30, max 300
    pub max_retries: u32,                  // default 3
    pub retry: RetryConfig,                // base_delay/max_delay/backoff_multiplier/jitter
    pub health_check: ProviderHealthCheckConfig,
    pub settings: HashMap<String, serde_json::Value>, // provider-specific keys
    pub models: Vec<String>,
    pub tags: Vec<String>,
    pub enabled: bool,                     // default true
}
```

This is the YAML-facing model. The separate `ProviderConfig` *trait* that provider implementations satisfy lives in `src/core/traits/provider/config.rs` — see [reference/provider-config-errors.md](reference/provider-config-errors.md).

---

## Environment Variable Substitution

`substitute_env_vars` in `src/config/mod.rs` processes the raw file text before parsing:

- `${VAR_NAME}` — substituted with the env value. If unset, loading **fails**: all missing braced variables are collected, deduplicated, sorted, and reported in one error (`Missing environment variables referenced by config: A, B`). Placeholders can never silently reach runtime as literal strings.
- `$VAR_NAME` — shell-style bare form, also substituted. If unset, the token is **left literal** so ordinary dollar-containing values pass through untouched.
- `${VAR:-default}` is **not supported** and is silently dangerous: neither recognized pattern matches a token containing `:-`, so the whole string stays literal in the parsed value — no substitution and no error.
- Substitution is line-based and quote/comment aware: text after an unquoted `#` (a YAML comment) is never substituted, and values pulled from the environment are not re-expanded.
- Errors surface as `GatewayError::Config` (the runtime loader does not use `ConfigError`; see [reference/provider-config-errors.md](reference/provider-config-errors.md)).

```yaml
# Works
api_key: "${OPENAI_API_KEY}"        # fails startup if OPENAI_API_KEY is unset
url: "$REDIS_URL"                   # left literal if REDIS_URL is unset

# Does NOT work - no fallback syntax; url becomes the literal
# string "${REDIS_URL:-redis://localhost:6379}"
url: "${REDIS_URL:-redis://localhost:6379}"
```

Separately, when the implicit-default serve path falls back to env-only configuration, `GatewayConfig::from_env` reads dedicated `LITELLM_*` variables (`LITELLM_HOST`, `LITELLM_PORT`, `LITELLM_DATABASE_URL`, `LITELLM_JWT_SECRET`, `LITELLM_PROVIDERS`, ... — see the constants atop `src/config/models/gateway.rs`). This is independent of `${VAR}` substitution inside YAML.

---

## Configuration Loading

Actual chain, `Config::from_file` in `src/config/mod.rs`:

```rust
pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    let content = tokio::fs::read_to_string(path).await
        .map_err(|e| GatewayError::Config(format!("Failed to read config file: {}", e)))?;
    let content = substitute_env_vars(&content)?;          // hard error on missing ${VAR}
    let gateway: GatewayConfig = serde_yml::from_str(&content)
        .map_err(|e| GatewayError::Config(format!("Failed to parse config: {}", e)))?;
    let config = Self { gateway };
    config.validate()?;                                    // Validate trait, single entry point
    Ok(config)
}
```

Entry points:

| Path | Behavior |
|------|----------|
| `gateway --config FILE ...` | loads exactly that file; failure is fatal |
| `gateway serve` (no `--config`) | tries `config/gateway.yaml`; on any load error retries via `Config::from_env()` |
| `gateway validate-config` | loads + validates only, prints result |

Secret redaction happens only on export: `to_json` / `to_yaml` replace known secrets (provider API keys, `jwt_secret`, HMAC secret, S3/vector/SSO credentials) with `[REDACTED]` before serializing.

## References

- [reference/validation.md](reference/validation.md) — Validate trait, validation module layout, and per-section validation rules.
- [reference/provider-config-errors.md](reference/provider-config-errors.md) — Provider configuration trait and ConfigError variants.
- [reference/best-practices.md](reference/best-practices.md) — Defaults, early validation, secret handling, type safety, and documentation practices.
