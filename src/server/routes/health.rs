//! Health check and status endpoints
//!
//! This module exposes three distinct health signals:
//!
//! * `GET /health` — basic **liveness**. Always returns 200 as long as the
//!   process is serving HTTP. Suitable for load-balancer liveness probes.
//! * `GET /health/ready` — **readiness**. Returns 200 only when the gateway
//!   has a working storage backend AND at least one configured, enabled
//!   provider has a successful probe. Returns 503 otherwise. Suitable for
//!   Kubernetes readiness probes and traffic gating.
//! * `GET /health/detailed` — diagnostic snapshot. Mirrors the readiness
//!   verdict in its `status` field (`healthy` / `degraded`) and includes
//!   per-component detail (storage, providers, host metrics).
//!
//! ## Aggregate rule (used by both `/health/ready` and `/health/detailed`)
//!
//! The gateway is considered **ready / healthy** only when ALL of:
//!
//! 1. Storage `overall` is true (when storage is configured).
//! 2. At least one provider is configured and enabled.
//! 3. Every enabled provider's status is `healthy`. Any enabled provider with
//!    status `unhealthy` OR `unknown` blocks readiness — per VibeGuard U-29,
//!    an unknown probe must not be reported as a healthy aggregate.
//!
//! Per-provider status values:
//!
//! | Value            | Meaning                                                 |
//! |------------------|---------------------------------------------------------|
//! | `healthy`        | Live probe succeeded.                                   |
//! | `unhealthy`      | Live probe failed.                                      |
//! | `unknown`        | Provider is enabled but no successful probe yet wired.  |
//! | `disabled`       | `enabled = false` in config; excluded from readiness.   |
//!
//! When zero providers are configured the aggregate reports `not_configured`
//! and readiness fails (a gateway with no upstreams cannot serve traffic).

#![allow(dead_code)]

use crate::server::routes::ApiResponse;
use crate::server::state::AppState;
use actix_web::{HttpResponse, Result as ActixResult, web};
use std::borrow::Cow;
#[cfg(feature = "metrics")]
use std::sync::LazyLock;
#[cfg(feature = "metrics")]
use sysinfo::System;

use tracing::{debug, error};

#[cfg(feature = "metrics")]
static HEALTH_SYSTEM: LazyLock<parking_lot::Mutex<System>> =
    LazyLock::new(|| parking_lot::Mutex::new(System::new_all()));

/// Configure health check routes
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

/// Basic liveness endpoint.
///
/// Always returns 200 while the process is up. Does **not** probe storage or
/// providers. Use `/health/ready` for traffic gating.
pub async fn health_check(_state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("Liveness check requested");

    let health_status = HealthStatus {
        status: Cow::Borrowed("alive"),
        timestamp: chrono::Utc::now(),
        version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(health_status)))
}

/// Readiness endpoint.
///
/// Returns 200 with `ready: true` only when the gateway can serve traffic
/// per the aggregate rule documented at the module top. Returns 503 with
/// `ready: false` and a short reason otherwise.
async fn readiness_check(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("Readiness check requested");

    let (storage_health, provider_health) = collect_component_health(&state).await;
    let verdict = aggregate_readiness(&storage_health, &provider_health);

    let body = ReadinessStatus {
        ready: verdict.ready,
        reason: verdict.reason,
        timestamp: chrono::Utc::now(),
        version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
        storage: storage_health,
        providers: provider_health,
    };

    let response = ApiResponse::success(body);
    if verdict.ready {
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(response))
    }
}

/// Detailed health check endpoint.
///
/// Returns comprehensive health information including storage, authentication,
/// and provider status. The top-level `status` field mirrors readiness:
/// `healthy` when the aggregate rule passes, `degraded` otherwise.
async fn detailed_health_check(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("Detailed health check requested");

    let (storage_health, provider_health) = collect_component_health(&state).await;
    let verdict = aggregate_readiness(&storage_health, &provider_health);

    let detailed_status = DetailedHealthStatus {
        status: if verdict.ready {
            Cow::Borrowed("healthy")
        } else {
            Cow::Borrowed("degraded")
        },
        reason: verdict.reason,
        timestamp: chrono::Utc::now(),
        version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
        uptime_seconds: get_uptime_seconds(),
        storage: storage_health,
        providers: provider_health,
        memory_usage: get_memory_usage(),
        cpu_usage: get_cpu_usage(),
    };

    let response = ApiResponse::success(detailed_status);
    if verdict.ready {
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(response))
    }
}

/// System status endpoint
///
/// Returns general system information and statistics.
async fn system_status(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("System status requested");

    let cfg = state.config.load();
    let system_status = SystemStatus {
        service_name: Cow::Borrowed("Rust LiteLLM Gateway"),
        version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
        build_time: Cow::Borrowed(env!("BUILD_TIME")),
        git_hash: Cow::Borrowed(env!("GIT_HASH")),
        rust_version: Cow::Borrowed(env!("RUST_VERSION")),
        uptime_seconds: get_uptime_seconds(),
        timestamp: chrono::Utc::now(),
        environment: std::env::var("ENVIRONMENT")
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed("development")),
        config: SystemConfig {
            server_host: cfg.server().host.clone(),
            server_port: cfg.server().port,
            auth_enabled: cfg.auth().enable_jwt || cfg.auth().enable_api_key,
            rate_limiting_enabled: cfg.gateway.rate_limit.enabled,
            caching_enabled: cfg.gateway.cache.enabled,
            providers_count: cfg.providers().len(),
        },
    };

    Ok(HttpResponse::Ok().json(ApiResponse::success(system_status)))
}

/// Version information endpoint
///
/// Returns version and build information.
async fn version_info() -> HttpResponse {
    debug!("Version info requested");

    let version_info = VersionInfo {
        version: Cow::Borrowed(env!("CARGO_PKG_VERSION")),
        build_time: Cow::Borrowed(env!("BUILD_TIME")),
        git_hash: Cow::Borrowed(env!("GIT_HASH")),
        rust_version: Cow::Borrowed(env!("RUST_VERSION")),
        features: get_enabled_features(),
    };

    HttpResponse::Ok().json(ApiResponse::success(version_info))
}

/// Metrics endpoint (Prometheus format)
///
/// Returns metrics in Prometheus format for monitoring systems.
async fn metrics(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    debug!("Metrics requested");

    // NOTE: Proper Prometheus metrics not yet implemented.
    // For now, return basic metrics in Prometheus format
    let metrics = format!(
        r#"# HELP gateway_uptime_seconds Total uptime of the gateway in seconds
# TYPE gateway_uptime_seconds counter
gateway_uptime_seconds {}

# HELP gateway_memory_usage_bytes Current memory usage in bytes
# TYPE gateway_memory_usage_bytes gauge
gateway_memory_usage_bytes {}

# HELP gateway_cpu_usage_percent Current CPU usage percentage
# TYPE gateway_cpu_usage_percent gauge
gateway_cpu_usage_percent {}

# HELP gateway_providers_total Total number of configured providers
# TYPE gateway_providers_total gauge
gateway_providers_total {}
"#,
        get_uptime_seconds(),
        get_memory_usage(),
        get_cpu_usage(),
        state.config.load().providers().len()
    );

    Ok(HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(metrics))
}

/// Basic liveness response body.
#[derive(Debug, Clone, serde::Serialize)]
struct HealthStatus {
    status: Cow<'static, str>,
    timestamp: chrono::DateTime<chrono::Utc>,
    version: Cow<'static, str>,
}

/// Readiness response body.
#[derive(Debug, Clone, serde::Serialize)]
struct ReadinessStatus {
    ready: bool,
    reason: Cow<'static, str>,
    timestamp: chrono::DateTime<chrono::Utc>,
    version: Cow<'static, str>,
    storage: crate::storage::StorageHealthStatus,
    providers: ProviderHealthStatus,
}

/// Detailed health status
#[derive(Debug, Clone, serde::Serialize)]
struct DetailedHealthStatus {
    status: Cow<'static, str>,
    reason: Cow<'static, str>,
    timestamp: chrono::DateTime<chrono::Utc>,
    version: Cow<'static, str>,
    uptime_seconds: u64,
    storage: crate::storage::StorageHealthStatus,
    providers: ProviderHealthStatus,
    memory_usage: u64,
    cpu_usage: f64,
}

/// Provider health status
#[derive(Debug, Clone, serde::Serialize)]
struct ProviderHealthStatus {
    /// Aggregate label across all configured providers. One of `healthy`,
    /// `degraded`, `unknown`, `not_configured`.
    aggregate: Cow<'static, str>,
    healthy_providers: usize,
    total_providers: usize,
    enabled_providers: usize,
    provider_details: Vec<ProviderHealth>,
}

/// Individual provider health
#[derive(Debug, Clone, serde::Serialize)]
struct ProviderHealth {
    name: String,
    /// One of `healthy`, `unhealthy`, `unknown`, `disabled`.
    status: Cow<'static, str>,
    response_time_ms: Option<u64>,
    last_check: chrono::DateTime<chrono::Utc>,
    error_message: Option<String>,
}

/// System status information
#[derive(Debug, Clone, serde::Serialize)]
struct SystemStatus {
    service_name: Cow<'static, str>,
    version: Cow<'static, str>,
    build_time: Cow<'static, str>,
    git_hash: Cow<'static, str>,
    rust_version: Cow<'static, str>,
    uptime_seconds: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
    environment: Cow<'static, str>,
    config: SystemConfig,
}

/// System configuration summary
#[derive(Debug, Clone, serde::Serialize)]
struct SystemConfig {
    server_host: String,
    server_port: u16,
    auth_enabled: bool,
    rate_limiting_enabled: bool,
    caching_enabled: bool,
    providers_count: usize,
}

/// Version information
#[derive(Debug, Clone, serde::Serialize)]
struct VersionInfo {
    version: Cow<'static, str>,
    build_time: Cow<'static, str>,
    git_hash: Cow<'static, str>,
    rust_version: Cow<'static, str>,
    features: Vec<Cow<'static, str>>,
}

/// Outcome of the readiness aggregate.
struct ReadinessVerdict {
    ready: bool,
    reason: Cow<'static, str>,
}

/// Collect both storage and provider health snapshots used by readiness and
/// detailed endpoints.
async fn collect_component_health(
    state: &AppState,
) -> (crate::storage::StorageHealthStatus, ProviderHealthStatus) {
    let cfg = state.config.load();

    let storage_health = if cfg.storage().database.url.is_empty() {
        crate::storage::StorageHealthStatus {
            overall: false,
            database: false,
            redis: false,
            files: false,
            vector: false,
        }
    } else {
        match state.storage.health_check().await {
            Ok(status) => status,
            Err(_) => crate::storage::StorageHealthStatus {
                overall: false,
                database: false,
                redis: false,
                files: false,
                vector: false,
            },
        }
    };

    let provider_health = match check_provider_health(state).await {
        Ok(status) => status,
        Err(e) => {
            error!("Provider health check failed: {}", e);
            ProviderHealthStatus {
                aggregate: Cow::Borrowed("not_configured"),
                healthy_providers: 0,
                total_providers: 0,
                enabled_providers: 0,
                provider_details: vec![],
            }
        }
    };

    (storage_health, provider_health)
}

/// Apply the aggregate rule defined at the module top.
fn aggregate_readiness(
    storage_health: &crate::storage::StorageHealthStatus,
    provider_health: &ProviderHealthStatus,
) -> ReadinessVerdict {
    if !storage_health.overall {
        return ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("storage unhealthy"),
        };
    }

    if provider_health.total_providers == 0 {
        return ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("no providers configured"),
        };
    }

    if provider_health.enabled_providers == 0 {
        return ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("no providers enabled"),
        };
    }

    // Among enabled providers, any non-healthy status (unhealthy OR unknown)
    // blocks readiness. Disabled providers are filtered out upstream.
    let mut has_unhealthy = false;
    let mut has_unknown = false;
    for p in &provider_health.provider_details {
        match p.status.as_ref() {
            "unhealthy" => has_unhealthy = true,
            "unknown" => has_unknown = true,
            _ => {}
        }
    }

    if has_unhealthy {
        return ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("one or more providers unhealthy"),
        };
    }
    if has_unknown {
        return ReadinessVerdict {
            ready: false,
            reason: Cow::Borrowed("one or more providers have unknown status"),
        };
    }

    ReadinessVerdict {
        ready: true,
        reason: Cow::Borrowed("ok"),
    }
}

/// Check provider health.
///
/// Per-provider live probes are not yet wired (see issue #555). Enabled
/// providers therefore report `unknown` until a real probe is implemented;
/// `unknown` is treated as not-ready by [`aggregate_readiness`] so an
/// unprobed deployment cannot present a green readiness signal.
async fn check_provider_health(
    state: &AppState,
) -> Result<ProviderHealthStatus, crate::utils::error::gateway_error::GatewayError> {
    let cfg = state.config.load();
    let mut provider_details = Vec::new();
    let mut healthy_count = 0;
    let mut enabled_count = 0;

    for provider_config in cfg.providers() {
        let (status, error_message): (Cow<'static, str>, Option<String>) =
            if !provider_config.enabled {
                (Cow::Borrowed("disabled"), None)
            } else {
                enabled_count += 1;
                (
                    Cow::Borrowed("unknown"),
                    Some("Provider health check not implemented".to_string()),
                )
            };

        if status == "healthy" {
            healthy_count += 1;
        }

        provider_details.push(ProviderHealth {
            name: provider_config.name.clone(),
            status,
            response_time_ms: None,
            last_check: chrono::Utc::now(),
            error_message,
        });
    }

    let total = cfg.providers().len();
    let aggregate = if total == 0 {
        Cow::Borrowed("not_configured")
    } else if enabled_count == 0 {
        Cow::Borrowed("disabled")
    } else if provider_details.iter().any(|p| p.status == "unhealthy") {
        Cow::Borrowed("degraded")
    } else if provider_details.iter().any(|p| p.status == "unknown") {
        Cow::Borrowed("unknown")
    } else {
        Cow::Borrowed("healthy")
    };

    Ok(ProviderHealthStatus {
        aggregate,
        healthy_providers: healthy_count,
        total_providers: total,
        enabled_providers: enabled_count,
        provider_details,
    })
}

/// Get system uptime in seconds
fn get_uptime_seconds() -> u64 {
    // This is a simplified implementation
    // In a real application, you would track the actual start time
    static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs()
}

/// Get memory usage in bytes
#[cfg(feature = "metrics")]
fn get_memory_usage() -> u64 {
    let mut sys = HEALTH_SYSTEM.lock();
    sys.refresh_memory();
    sys.used_memory()
}

/// Get CPU usage percentage
#[cfg(not(feature = "metrics"))]
fn get_memory_usage() -> u64 {
    0
}

/// Get CPU usage percentage
#[cfg(feature = "metrics")]
fn get_cpu_usage() -> f64 {
    let mut sys = HEALTH_SYSTEM.lock();
    sys.refresh_cpu_usage();
    sys.global_cpu_usage() as f64
}

/// Get CPU usage percentage
#[cfg(not(feature = "metrics"))]
fn get_cpu_usage() -> f64 {
    0.0
}

/// Get enabled features
fn get_enabled_features() -> Vec<Cow<'static, str>> {
    let mut features = Vec::new();

    #[cfg(feature = "enterprise")]
    features.push(Cow::Borrowed("enterprise"));

    #[cfg(feature = "analytics")]
    features.push(Cow::Borrowed("analytics"));

    #[cfg(feature = "vector-db")]
    features.push(Cow::Borrowed("vector-db"));

    if features.is_empty() {
        features.push(Cow::Borrowed("standard"));
    }

    features
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_ok() -> crate::storage::StorageHealthStatus {
        crate::storage::StorageHealthStatus {
            overall: true,
            database: true,
            redis: true,
            files: true,
            vector: true,
        }
    }

    fn storage_bad() -> crate::storage::StorageHealthStatus {
        crate::storage::StorageHealthStatus {
            overall: false,
            database: false,
            redis: false,
            files: false,
            vector: false,
        }
    }

    fn provider(name: &str, status: &'static str) -> ProviderHealth {
        ProviderHealth {
            name: name.to_string(),
            status: Cow::Borrowed(status),
            response_time_ms: None,
            last_check: chrono::Utc::now(),
            error_message: None,
        }
    }

    fn provider_status(details: Vec<ProviderHealth>) -> ProviderHealthStatus {
        let total = details.len();
        let enabled = details.iter().filter(|p| p.status != "disabled").count();
        let healthy = details.iter().filter(|p| p.status == "healthy").count();
        let aggregate = if total == 0 {
            Cow::Borrowed("not_configured")
        } else if enabled == 0 {
            Cow::Borrowed("disabled")
        } else if details.iter().any(|p| p.status == "unhealthy") {
            Cow::Borrowed("degraded")
        } else if details.iter().any(|p| p.status == "unknown") {
            Cow::Borrowed("unknown")
        } else {
            Cow::Borrowed("healthy")
        };
        ProviderHealthStatus {
            aggregate,
            healthy_providers: healthy,
            total_providers: total,
            enabled_providers: enabled,
            provider_details: details,
        }
    }

    // Acceptance case 1: all-unknown providers must NOT be ready.
    #[test]
    fn readiness_fails_when_all_providers_unknown() {
        let providers = provider_status(vec![
            provider("openai", "unknown"),
            provider("anthropic", "unknown"),
        ]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(!verdict.ready);
        assert_eq!(verdict.reason, "one or more providers have unknown status");
        assert_eq!(providers.aggregate, "unknown");
    }

    // Acceptance case 2: one unhealthy provider must NOT be ready and the
    // reason must point at the unhealthy signal, not the unknown one.
    #[test]
    fn readiness_fails_when_one_provider_unhealthy() {
        let providers = provider_status(vec![
            provider("openai", "healthy"),
            provider("anthropic", "unhealthy"),
        ]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(!verdict.ready);
        assert_eq!(verdict.reason, "one or more providers unhealthy");
        assert_eq!(providers.aggregate, "degraded");
    }

    // Acceptance case 3: no providers configured at all -> not ready.
    #[test]
    fn readiness_fails_when_no_providers_configured() {
        let providers = provider_status(vec![]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(!verdict.ready);
        assert_eq!(verdict.reason, "no providers configured");
        assert_eq!(providers.aggregate, "not_configured");
    }

    // Acceptance case 4: all enabled providers report healthy -> ready.
    #[test]
    fn readiness_passes_when_all_providers_healthy() {
        let providers = provider_status(vec![
            provider("openai", "healthy"),
            provider("anthropic", "healthy"),
        ]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(verdict.ready);
        assert_eq!(verdict.reason, "ok");
        assert_eq!(providers.aggregate, "healthy");
    }

    // Storage down must always block readiness regardless of provider state.
    #[test]
    fn readiness_fails_when_storage_unhealthy() {
        let providers = provider_status(vec![provider("openai", "healthy")]);
        let verdict = aggregate_readiness(&storage_bad(), &providers);
        assert!(!verdict.ready);
        assert_eq!(verdict.reason, "storage unhealthy");
    }

    // Disabled providers must not block readiness on their own, but if every
    // configured provider is disabled the gateway cannot serve traffic.
    #[test]
    fn readiness_fails_when_all_providers_disabled() {
        let providers = provider_status(vec![
            provider("openai", "disabled"),
            provider("anthropic", "disabled"),
        ]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(!verdict.ready);
        assert_eq!(verdict.reason, "no providers enabled");
        assert_eq!(providers.aggregate, "disabled");
    }

    // A disabled provider next to a healthy one should still be ready.
    #[test]
    fn readiness_passes_when_disabled_provider_alongside_healthy() {
        let providers = provider_status(vec![
            provider("openai", "healthy"),
            provider("legacy", "disabled"),
        ]);
        let verdict = aggregate_readiness(&storage_ok(), &providers);
        assert!(verdict.ready);
        assert_eq!(providers.enabled_providers, 1);
    }

    #[test]
    fn version_info_serializes() {
        let version_info = VersionInfo {
            version: Cow::Borrowed("1.0.0"),
            build_time: Cow::Borrowed("2024-01-01T00:00:00Z"),
            git_hash: Cow::Borrowed("abc123"),
            rust_version: Cow::Borrowed("1.75.0"),
            features: vec![Cow::Borrowed("standard")],
        };

        assert_eq!(version_info.version, "1.0.0");
        assert!(!version_info.features.is_empty());
    }

    #[test]
    fn enabled_features_non_empty() {
        let features = get_enabled_features();
        assert!(!features.is_empty());
        let valid_features = ["standard", "enterprise", "analytics", "vector-db"];
        assert!(
            features
                .iter()
                .any(|f| valid_features.contains(&f.as_ref()))
        );
    }
}
