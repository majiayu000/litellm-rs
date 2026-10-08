//! Shared deployment circuit-breaker state.
//!
//! Local atomics remain the default. When a live Redis pool is attached,
//! open/half-open/recovery run as single-key Lua so replicas share cooldown.
//! Redis errors follow `allow_degraded` (strict = fail closed, degraded =
//! last local snapshot).

use super::config::RouterConfig;
use super::deployment::Deployment;
#[cfg(feature = "gateway")]
use super::deployment::HealthStatus;
use super::error::CooldownReason;
#[cfg(feature = "gateway")]
use dashmap::DashMap;
#[cfg(feature = "gateway")]
use std::sync::atomic::Ordering;
#[cfg(feature = "gateway")]
use std::time::{Duration, Instant};
#[cfg(any(feature = "gateway", test))]
use tracing::warn;

#[cfg(feature = "gateway")]
const CACHE_TTL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Default)]
pub(crate) enum CircuitBackend {
    #[default]
    InProcess,
    #[cfg(feature = "gateway")]
    Redis {
        pool: std::sync::Arc<crate::storage::redis::RedisPool>,
        probe_token: String,
        allow_degraded: bool,
        cache: std::sync::Arc<
            DashMap<String, (crate::storage::redis::circuit::CircuitState, Instant)>,
        >,
    },
    #[cfg(test)]
    Unavailable { allow_degraded: bool },
}

pub(crate) enum CircuitObserve {
    UseLocal,
    #[cfg(feature = "gateway")]
    Shared(crate::storage::redis::circuit::CircuitState),
    #[cfg(any(feature = "gateway", test))]
    Blocked,
}

pub(crate) enum CircuitWrite {
    Local,
    #[cfg(feature = "gateway")]
    Applied(crate::storage::redis::circuit::CircuitState),
    #[cfg(any(feature = "gateway", test))]
    StrictUnavailable,
}

impl CircuitBackend {
    #[cfg(feature = "gateway")]
    pub(crate) fn redis(pool: std::sync::Arc<crate::storage::redis::RedisPool>) -> Self {
        if pool.is_noop() {
            Self::InProcess
        } else {
            Self::Redis {
                allow_degraded: pool.config.allow_degraded,
                pool,
                probe_token: uuid::Uuid::new_v4().to_string(),
                cache: std::sync::Arc::new(DashMap::new()),
            }
        }
    }

    pub(crate) async fn observe_async(
        &self,
        deployment: &Deployment,
        config: &RouterConfig,
    ) -> CircuitObserve {
        #[cfg(not(feature = "gateway"))]
        let _ = (deployment, config);
        match self {
            Self::InProcess => CircuitObserve::UseLocal,
            #[cfg(test)]
            Self::Unavailable { allow_degraded } => {
                if *allow_degraded {
                    warn!(
                        deployment_id = %deployment.id,
                        "circuit backend unavailable; using last local snapshot"
                    );
                    CircuitObserve::UseLocal
                } else {
                    warn!(
                        deployment_id = %deployment.id,
                        "circuit backend unavailable; failing closed"
                    );
                    CircuitObserve::Blocked
                }
            }
            #[cfg(feature = "gateway")]
            Self::Redis {
                pool,
                probe_token,
                allow_degraded,
                cache,
            } => {
                let shared_id = deployment.shared_state_id();
                if let Some(entry) = cache.get(shared_id.as_str())
                    && entry.1.elapsed() < CACHE_TTL
                {
                    return CircuitObserve::Shared(entry.0);
                }
                match invoke(pool, &shared_id, probe_token, config, "observe", 0).await {
                    Ok(state) => {
                        cache.insert(shared_id.clone(), (state, Instant::now()));
                        CircuitObserve::Shared(state)
                    }
                    Err(_) => redis_loss_observe(&deployment.id, *allow_degraded),
                }
            }
        }
    }

    pub(crate) async fn record_failure_async(
        &self,
        deployment: &Deployment,
        config: &RouterConfig,
        reason: CooldownReason,
    ) -> CircuitWrite {
        self.write(deployment, config, "fail", reason_code(reason))
            .await
    }

    pub(crate) async fn record_success_async(
        &self,
        deployment: &Deployment,
        config: &RouterConfig,
    ) -> CircuitWrite {
        self.write(deployment, config, "ok", 0).await
    }

    async fn write(
        &self,
        deployment: &Deployment,
        config: &RouterConfig,
        op: &'static str,
        reason: i64,
    ) -> CircuitWrite {
        #[cfg(not(feature = "gateway"))]
        let _ = (deployment, config, op, reason);
        match self {
            Self::InProcess => CircuitWrite::Local,
            #[cfg(test)]
            Self::Unavailable { allow_degraded } => {
                if *allow_degraded {
                    warn!(
                        deployment_id = %deployment.id,
                        operation = op,
                        "circuit backend unavailable; using last local snapshot"
                    );
                    CircuitWrite::Local
                } else {
                    warn!(
                        deployment_id = %deployment.id,
                        operation = op,
                        "circuit backend unavailable; failing closed"
                    );
                    CircuitWrite::StrictUnavailable
                }
            }
            #[cfg(feature = "gateway")]
            Self::Redis {
                pool,
                probe_token,
                allow_degraded,
                cache,
            } => {
                let shared_id = deployment.shared_state_id();
                match invoke(pool, &shared_id, probe_token, config, op, reason).await {
                    Ok(state) => {
                        cache.insert(shared_id, (state, Instant::now()));
                        CircuitWrite::Applied(state)
                    }
                    Err(_) if *allow_degraded => {
                        warn!(
                            deployment_id = %deployment.id,
                            operation = op,
                            "circuit redis operation failed; using last local snapshot"
                        );
                        CircuitWrite::Local
                    }
                    Err(_) => {
                        warn!(
                            deployment_id = %deployment.id,
                            operation = op,
                            "circuit redis operation failed; failing closed"
                        );
                        CircuitWrite::StrictUnavailable
                    }
                }
            }
        }
    }
}

#[cfg(feature = "gateway")]
pub(crate) fn apply_circuit_snapshot(
    deployment: &Deployment,
    state: &crate::storage::redis::circuit::CircuitState,
) {
    deployment
        .state
        .cooldown_until
        .store(state.opened_until.max(0) as u64, Ordering::Relaxed);
    deployment
        .state
        .consecutive_successes
        .store(state.consecutive_successes.max(0) as u32, Ordering::Relaxed);
    deployment
        .state
        .fails_this_minute
        .store(state.fails.max(0) as u32, Ordering::Relaxed);
    let health = if state.status == 1 {
        HealthStatus::Cooldown
    } else if deployment.state.probe_unhealthy.load(Ordering::Relaxed) {
        HealthStatus::Unhealthy
    } else {
        HealthStatus::from(state.health.max(0) as u8)
    };
    deployment
        .state
        .health
        .store(health as u8, Ordering::Relaxed);
}

fn reason_code(reason: CooldownReason) -> i64 {
    match reason {
        CooldownReason::ConsecutiveFailures => 0,
        CooldownReason::HighFailureRate => 2,
        CooldownReason::RateLimit
        | CooldownReason::AuthError
        | CooldownReason::NotFound
        | CooldownReason::Timeout
        | CooldownReason::Manual => 1,
    }
}

#[cfg(feature = "gateway")]
fn redis_loss_observe(deployment_id: &str, allow_degraded: bool) -> CircuitObserve {
    if allow_degraded {
        warn!(
            deployment_id,
            "circuit redis operation failed; using last local snapshot"
        );
        CircuitObserve::UseLocal
    } else {
        warn!(
            deployment_id,
            "circuit redis operation failed; failing closed"
        );
        CircuitObserve::Blocked
    }
}

#[cfg(feature = "gateway")]
async fn invoke(
    pool: &std::sync::Arc<crate::storage::redis::RedisPool>,
    deployment_id: &str,
    token: &str,
    config: &RouterConfig,
    op: &'static str,
    reason: i64,
) -> Result<crate::storage::redis::circuit::CircuitState, ()> {
    let pool = std::sync::Arc::clone(pool);
    let key = crate::storage::redis::RedisPool::circuit_key(deployment_id);
    let token = token.to_string();
    let now_secs = now_secs();
    let window_epoch = now_secs / 60;
    let allowed_fails = i64::from(config.allowed_fails);
    let min_requests = i64::from(config.min_requests);
    let cooldown_secs = i64::try_from(config.cooldown_time_secs).unwrap_or(i64::MAX);
    let success_threshold = i64::from(config.success_threshold);
    run_redis(deployment_id, op, async move {
        pool.circuit_invoke(
            &key,
            crate::storage::redis::circuit::CircuitArgs {
                op,
                now_secs,
                window_epoch,
                token: &token,
                allowed_fails,
                min_requests,
                cooldown_secs,
                success_threshold,
                reason,
            },
        )
        .await
    })
    .await
}

#[cfg(feature = "gateway")]
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(feature = "gateway")]
fn circuit_io_handle() -> tokio::runtime::Handle {
    static HANDLE: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::Builder::new()
                .name("circuit-redis-rt".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(1)
                        .enable_all()
                        .thread_name("circuit-redis")
                        .build()
                        .expect("circuit redis runtime");
                    let handle = runtime.handle().clone();
                    tx.send(handle).expect("send circuit redis handle");
                    runtime.block_on(std::future::pending::<()>());
                })
                .expect("spawn circuit redis runtime thread");
            rx.recv().expect("circuit redis runtime handle")
        })
        .clone()
}

#[cfg(feature = "gateway")]
fn circuit_io_slots() -> &'static tokio::sync::Semaphore {
    static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(64);
    &SLOTS
}

#[cfg(all(test, feature = "gateway"))]
pub(crate) async fn pause_circuit_io() -> tokio::sync::SemaphorePermit<'static> {
    circuit_io_slots().acquire_many(64).await.unwrap()
}

#[cfg(feature = "gateway")]
fn run_redis<'a, T>(
    deployment_id: &'a str,
    operation: &'static str,
    fut: impl std::future::Future<Output = crate::utils::error::gateway_error::Result<T>>
    + Send
    + 'static,
) -> impl std::future::Future<Output = Result<T, ()>> + Send + 'a
where
    T: Send + 'static,
{
    // Keep the connection/Lua implementation out of the unpolled async frame
    // exposed to selection and request completion callers.
    let fut: futures::future::BoxFuture<'static, crate::utils::error::gateway_error::Result<T>> =
        Box::pin(fut);
    async move {
        let permit = circuit_io_slots().acquire().await.map_err(|_| ())?;
        let task = circuit_io_handle().spawn(async move {
            let _permit = permit;
            fut.await
        });
        match task.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(err)) => {
                warn!(
                    deployment_id,
                    operation,
                    error = %err,
                    "circuit redis operation failed"
                );
                Err(())
            }
            Err(_) => {
                warn!(deployment_id, operation, "circuit redis worker dropped");
                Err(())
            }
        }
    }
}

#[cfg(all(test, feature = "gateway"))]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn run_redis_does_not_panic_on_current_thread_runtime() {
        let result = run_redis("probe", "probe", async {
            Ok::<i64, crate::utils::error::gateway_error::GatewayError>(7)
        })
        .await;
        assert_eq!(result.expect("current_thread redis bridge"), 7);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn redis_wait_keeps_the_request_worker_polling() {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            tokio::task::yield_now().await;
            let _ = sender.send(7);
        });
        let result = run_redis("worker-progress", "probe", async move {
            tokio::time::timeout(std::time::Duration::from_secs(2), receiver)
                .await
                .map_err(|_| {
                    crate::utils::error::gateway_error::GatewayError::Internal(
                        "request worker stopped polling".into(),
                    )
                })?
                .map_err(|_| {
                    crate::utils::error::gateway_error::GatewayError::Internal(
                        "peer dropped".into(),
                    )
                })
        })
        .await;
        assert_eq!(result.unwrap(), 7);
        peer.await.unwrap();
    }
}
