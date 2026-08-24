## Contents

- Health Endpoints
- Aggregate Readiness Rule
- Response Models
- Shared HealthStatus Enum

## Health Endpoints

All routes live in `src/server/routes/health.rs` and are mounted on the main HTTP server.
There is **no** `/health/live` route.
`monitoring.health.path` and `monitoring.health.detailed` are parsed and validated but are
not read by route wiring: changing them neither relocates `/health` nor disables
`/health/detailed` today.

```rust
// src/server/routes/health.rs
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/health")
            .route("", web::get().to(health_check))
            .route("/ready", web::get().to(readiness_check))
            .route("/detailed", web::get().to(detailed_health_check)),
    )
    .route("/status", web::get().to(system_status))
    .route("/version", web::get().to(version_info))
    .route("/metrics", web::get().to(metrics));
}
```

| Route | Purpose | Status codes |
|---|---|---|
| `GET /health` | Liveness. Always 200 while the process serves HTTP; probes nothing (`status: "alive"`). | 200 |
| `GET /health/ready` | Readiness for traffic gating. Applies the aggregate rule below plus audit-log availability. | 200 or 503 |
| `GET /health/detailed` | Diagnostic snapshot. Top-level `status` is `healthy`/`degraded`, mirroring the readiness verdict, plus uptime, memory, CPU, and per-component detail. | 200 or 503 |
| `GET /status` | Build info, uptime, environment, and a config summary (auth/rate-limit/cache flags, provider count). | 200 |
| `GET /version` | Version, build time, git hash, rustc version, enabled features. | 200 |
| `GET /metrics` | Prometheus text format (see [metrics.md](metrics.md)). | 200 |

JSON bodies are wrapped in the common envelope `ApiResponse<T>`:
`{ "success": bool, "data": T?, "error": string?, "meta": any? }`
(`src/server/routes/mod.rs`).

Per-provider status values reported by `/health/ready` and `/health/detailed`:

| Value | Meaning |
|---|---|
| `healthy` | Live probe succeeded. |
| `unhealthy` | Live probe failed. |
| `unknown` | Provider is enabled but no successful probe yet wired. Blocks readiness. |
| `disabled` | `enabled = false` in config; excluded from readiness. |

The provider aggregate is one of `healthy`, `degraded` (any enabled provider unhealthy),
`unknown` (any enabled provider unprobed), `disabled` (none enabled), or `not_configured`.

Live per-provider probes are not yet wired (tracked in issue #555): every enabled provider
currently reports `unknown`, so a stock deployment fails readiness by design until probes land.

## Aggregate Readiness Rule

Used by both `/health/ready` and `/health/detailed` (`aggregate_readiness` +
`include_audit_readiness`). The gateway is ready only when ALL of:

1. Storage `overall` is true (checked via `state.storage.health_check()`,
   `crate::storage::StorageHealthStatus { overall, database, redis, files, vector }`).
2. At least one provider is configured.
3. At least one provider is enabled, and the provider aggregate is exactly `healthy`.
   Any `unhealthy` **or** `unknown` enabled provider blocks readiness — an unknown probe must
   never be reported as a healthy aggregate.
4. The audit logger is available (`state.audit_logger.is_available()`); otherwise readiness
   fails with reason `"audit logging unavailable"`.

```rust
// src/server/routes/health.rs (abridged)
fn aggregate_readiness(
    storage_health: &crate::storage::StorageHealthStatus,
    provider_health: &ProviderHealthStatus,
) -> ReadinessVerdict {
    if !storage_health.overall {
        return ReadinessVerdict { ready: false, reason: Cow::Borrowed("storage unhealthy") };
    }
    if provider_health.total_providers == 0 {
        return ReadinessVerdict { ready: false, reason: Cow::Borrowed("no providers configured") };
    }
    if provider_health.enabled_providers == 0 {
        return ReadinessVerdict { ready: false, reason: Cow::Borrowed("no providers enabled") };
    }
    match provider_health.aggregate.as_ref() {
        "healthy" => ReadinessVerdict { ready: true, reason: Cow::Borrowed("ok") },
        "degraded" => ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("one or more providers unhealthy"),
        },
        "unknown" => ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("one or more providers have unknown status"),
        },
        _ => ReadinessVerdict { ready: false, reason: Cow::Borrowed("provider health unavailable") },
    }
}
```

## Response Models

Serialized structs from `src/server/routes/health.rs`:

```rust
struct ReadinessStatus {
    ready: bool,
    reason: Cow<'static, str>,          // e.g. "ok", "storage unhealthy"
    timestamp: chrono::DateTime<chrono::Utc>,
    version: Cow<'static, str>,
    storage: crate::storage::StorageHealthStatus,
    providers: ProviderHealthStatus,
}

struct DetailedHealthStatus {
    status: Cow<'static, str>,          // "healthy" | "degraded" — mirrors readiness verdict
    reason: Cow<'static, str>,
    timestamp: chrono::DateTime<chrono::Utc>,
    version: Cow<'static, str>,
    uptime_seconds: u64,
    storage: crate::storage::StorageHealthStatus,
    providers: ProviderHealthStatus,
    memory_usage: u64,                  // sysinfo under the "metrics" feature, else 0
    cpu_usage: f64,                     // sysinfo under the "metrics" feature, else 0.0
}

struct ProviderHealthStatus {
    aggregate: Cow<'static, str>,       // healthy|degraded|unknown|disabled|not_configured
    healthy_providers: usize,
    total_providers: usize,
    enabled_providers: usize,
    provider_details: Vec<ProviderHealth>,
}

struct ProviderHealth {
    name: String,
    status: Cow<'static, str>,          // healthy|unhealthy|unknown|disabled
    response_time_ms: Option<u64>,      // None today; probes are not wired
    last_check: chrono::DateTime<chrono::Utc>,
    error_message: Option<String>,
}
```

The liveness body is `{ status: "alive", timestamp, version }`. Memory/CPU come from a shared
`sysinfo::System` behind the `metrics` feature; without that feature they render as `0`.

## Shared HealthStatus Enum

The library-level status type used by storage/provider health code lives at
`crate::core::types::health::HealthStatus` (`src/core/types/health.rs`) — there is no
`core::types::common` module. It has four variants and serializes lowercase:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    #[default]
    Unknown,     // the default variant — treat as not-ready, never as healthy
    Degraded,
}
```

Note this enum is distinct from the string-valued statuses on the HTTP endpoints above;
the HTTP handlers use plain `&'static str` values so `unknown` can be surfaced verbatim.
