//! Shared atomic backend for deployment admission (parallel / RPM / TPM).
//!
//! Local CAS remains the default. When a live Redis pool is attached,
//! reserve/settle/cancel run as single-key Lua so replica counts cannot
//! multiply limits. Redis errors fail closed.

use super::deployment::Deployment;
#[cfg(any(feature = "gateway", test))]
use tracing::warn;

#[cfg(feature = "gateway")]
pub(crate) const DEFAULT_LEASE_TTL_MS: i64 = 600_000;

#[derive(Clone, Debug)]
pub(crate) struct AdmissionHold {
    #[cfg(feature = "gateway")]
    pub lease_id: String,
    #[cfg(feature = "gateway")]
    pub deployment_id: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) enum AdmissionBackend {
    #[default]
    InProcess,
    #[cfg(feature = "gateway")]
    Redis {
        pool: std::sync::Arc<crate::storage::redis::RedisPool>,
        lease_ttl_ms: i64,
    },
    #[cfg(test)]
    Unavailable,
}

pub(crate) enum AdmissionReserve {
    Skipped,
    #[cfg(any(feature = "gateway", test))]
    Denied,
    #[cfg(feature = "gateway")]
    Granted(AdmissionHold),
}

impl AdmissionBackend {
    #[cfg(feature = "gateway")]
    pub(crate) fn redis(pool: std::sync::Arc<crate::storage::redis::RedisPool>) -> Self {
        if pool.is_noop() {
            Self::InProcess
        } else {
            Self::Redis {
                pool,
                lease_ttl_ms: DEFAULT_LEASE_TTL_MS,
            }
        }
    }

    pub(crate) fn reserve(
        &self,
        deployment: &Deployment,
        estimated_tokens: u64,
    ) -> AdmissionReserve {
        #[cfg(not(feature = "gateway"))]
        let _ = (estimated_tokens,);
        let max_parallel = option_limit(deployment.config.max_parallel_requests.map(i64::from));
        let max_rpm = option_limit(deployment.config.rpm_limit.map(to_i64));
        let max_tpm = option_limit(deployment.config.tpm_limit.map(to_i64));
        if max_parallel < 0 && max_rpm < 0 && max_tpm < 0 {
            return AdmissionReserve::Skipped;
        }

        match self {
            Self::InProcess => AdmissionReserve::Skipped,
            #[cfg(test)]
            Self::Unavailable => {
                warn!(
                    deployment_id = %deployment.id,
                    "deployment admission backend unavailable; failing closed"
                );
                AdmissionReserve::Denied
            }
            #[cfg(feature = "gateway")]
            Self::Redis { pool, lease_ttl_ms } => {
                let rpm_inc = i64::from(max_rpm >= 0);
                let tpm_inc = if max_tpm >= 0 {
                    to_i64(estimated_tokens.max(1))
                } else {
                    0
                };
                let lease_id = uuid::Uuid::new_v4().to_string();
                let pool = std::sync::Arc::clone(pool);
                let key = crate::storage::redis::RedisPool::admission_key(&deployment.id);
                let ttl_ms = *lease_ttl_ms;
                let lease_id_for_fut = lease_id.clone();
                let deployment_id = deployment.id.clone();
                let state = match run_redis(&deployment_id, "reserve", async move {
                    pool.admission_reserve(crate::storage::redis::admission::AdmissionReserveArgs {
                        key: &key,
                        max_parallel,
                        max_rpm,
                        max_tpm,
                        rpm_inc,
                        tpm_inc,
                        lease_id: &lease_id_for_fut,
                        ttl_ms,
                    })
                    .await
                }) {
                    Ok(state) => state,
                    Err(_) => return AdmissionReserve::Denied,
                };
                if !state.allowed {
                    return AdmissionReserve::Denied;
                }
                AdmissionReserve::Granted(AdmissionHold {
                    lease_id,
                    deployment_id,
                })
            }
        }
    }

    pub(crate) fn settle(&self, hold: &AdmissionHold, actual_tokens: u64) {
        self.finish(hold, "settle", to_i64(actual_tokens));
    }

    pub(crate) fn cancel(&self, hold: &AdmissionHold) {
        self.finish(hold, "cancel", 0);
    }

    fn finish(&self, hold: &AdmissionHold, op: &'static str, actual_tpm: i64) {
        #[cfg(not(feature = "gateway"))]
        let _ = (hold, op, actual_tpm);
        match self {
            Self::InProcess => {}
            #[cfg(test)]
            Self::Unavailable => {}
            #[cfg(feature = "gateway")]
            Self::Redis { pool, .. } => {
                let pool = std::sync::Arc::clone(pool);
                let key = crate::storage::redis::RedisPool::admission_key(&hold.deployment_id);
                let lease_id = hold.lease_id.clone();
                let deployment_id = hold.deployment_id.clone();
                let _ = run_redis(&deployment_id, op, async move {
                    if op == "settle" {
                        pool.admission_settle(&key, &lease_id, actual_tpm).await
                    } else {
                        pool.admission_cancel(&key, &lease_id).await
                    }
                });
            }
        }
    }
}

fn option_limit(limit: Option<i64>) -> i64 {
    limit.filter(|value| *value >= 0).unwrap_or(-1)
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(feature = "gateway")]
fn admission_io_handle() -> tokio::runtime::Handle {
    static HANDLE: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::sync_channel(1);
            std::thread::Builder::new()
                .name("admission-redis-rt".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(1)
                        .enable_all()
                        .thread_name("admission-redis")
                        .build()
                        .expect("admission redis runtime");
                    let handle = runtime.handle().clone();
                    tx.send(handle).expect("send admission redis handle");
                    runtime.block_on(std::future::pending::<()>());
                })
                .expect("spawn admission redis runtime thread");
            rx.recv().expect("admission redis runtime handle")
        })
        .clone()
}

#[cfg(feature = "gateway")]
fn run_redis<T>(
    deployment_id: &str,
    operation: &'static str,
    fut: impl std::future::Future<Output = crate::utils::error::gateway_error::Result<T>>
    + Send
    + 'static,
) -> Result<T, ()>
where
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    admission_io_handle().spawn(async move {
        let _ = tx.send(fut.await);
    });
    match rx.recv() {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => {
            warn!(
                deployment_id,
                operation,
                error = %err,
                "admission redis operation failed; failing closed"
            );
            Err(())
        }
        Err(_) => {
            warn!(
                deployment_id,
                operation, "admission redis worker dropped; failing closed"
            );
            Err(())
        }
    }
}

#[cfg(all(test, feature = "gateway"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finishing_old_lease_keeps_the_current_window() {
        use crate::config::models::storage::RedisConfig;
        use crate::storage::redis::RedisPool;

        let Ok(url) = std::env::var("REDIS_URL") else {
            assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
            return;
        };
        let pool = std::sync::Arc::new(
            RedisPool::new(&RedisConfig {
                url,
                enabled: true,
                allow_degraded: false,
                ..RedisConfig::default()
            })
            .await
            .unwrap(),
        );
        let mut conn = pool.open_live_connection().await.unwrap();
        let backend = AdmissionBackend::redis(pool.clone());
        for op in ["settle", "cancel"] {
            let (seconds, micros): (i64, i64) =
                redis::cmd("TIME").query_async(&mut conn).await.unwrap();
            let epoch = seconds / 60;
            let deployment_id = uuid::Uuid::new_v4().to_string();
            let key = RedisPool::admission_key(&deployment_id);
            let hold = AdmissionHold {
                lease_id: "old".into(),
                deployment_id,
            };
            // Finish a lease from the previous window through the production backend.
            redis::cmd("HSET")
                .arg(&key)
                .arg("p")
                .arg(1)
                .arg("r")
                .arg(1)
                .arg("t")
                .arg(10)
                .arg("e")
                .arg(epoch - 1)
                .arg("l:old")
                .arg(format!(
                    "1:1:10:{}:{}",
                    epoch - 1,
                    seconds * 1_000 + micros / 1_000 + DEFAULT_LEASE_TTL_MS
                ))
                .query_async::<i64>(&mut conn)
                .await
                .unwrap();
            backend.finish(&hold, op, 4);
            let state: (i64, i64, i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["e", "p", "r", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            let tpm = if op == "settle" { 4 } else { 0 };
            let (after, _): (i64, i64) = redis::cmd("TIME").query_async(&mut conn).await.unwrap();
            assert!((epoch..=after / 60).contains(&state.0));
            assert_eq!(
                (state.1, state.2, state.3),
                (0, 0, tpm),
                "{op} must use the current window"
            );
            pool.delete(&key).await.unwrap();
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn run_redis_does_not_panic_on_current_thread_runtime() {
        let result = run_redis("probe", "probe", async {
            Ok::<i64, crate::utils::error::gateway_error::GatewayError>(7)
        });
        assert_eq!(result.expect("current_thread redis bridge"), 7);
    }
}
