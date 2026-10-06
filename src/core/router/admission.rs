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
    inner: std::sync::Arc<AdmissionHoldInner>,
}

impl AdmissionHold {
    /// Preserve a known cancellation before another accounting await.
    #[cfg(feature = "gateway")]
    pub(crate) fn prepare_cancellation(&self) {
        *self.inner.completion.lock() = Some(AdmissionCompletion::Cancel);
    }

    /// Preserve known usage before yielding it to a caller that may drop the
    /// stream. The lease's existing RAII cleanup owns the eventual Redis write.
    pub(crate) fn prepare_settlement(&self, actual_tokens: u64) {
        #[cfg(feature = "gateway")]
        {
            *self.inner.completion.lock() =
                Some(AdmissionCompletion::Settle(to_i64(actual_tokens)));
        }
        #[cfg(not(feature = "gateway"))]
        let _ = actual_tokens;
    }
}

#[cfg(feature = "gateway")]
#[derive(Debug)]
struct AdmissionHoldInner {
    pool: std::sync::Arc<crate::storage::redis::RedisPool>,
    lease_id: String,
    deployment_id: String,
    // A cancelled waiter must retain the desired settlement, not turn known
    // usage into a cancellation while waiting for an I/O permit.
    completion: parking_lot::Mutex<Option<AdmissionCompletion>>,
}

#[cfg(feature = "gateway")]
#[derive(Clone, Copy, Debug)]
enum AdmissionCompletion {
    Cancel,
    Settle(i64),
}

#[cfg(feature = "gateway")]
impl AdmissionHold {
    pub(crate) fn detach(&self) {
        *self.inner.completion.lock() = None;
    }

    fn new(
        pool: std::sync::Arc<crate::storage::redis::RedisPool>,
        lease_id: String,
        deployment_id: String,
    ) -> Self {
        Self {
            inner: std::sync::Arc::new(AdmissionHoldInner {
                pool,
                lease_id,
                deployment_id,
                completion: parking_lot::Mutex::new(Some(AdmissionCompletion::Cancel)),
            }),
        }
    }
}

#[cfg(feature = "gateway")]
impl Drop for AdmissionHoldInner {
    fn drop(&mut self) {
        if let Some(completion) = self.completion.get_mut().take() {
            enqueue_cleanup(AdmissionCleanup {
                pool: self.pool.clone(),
                lease_id: self.lease_id.clone(),
                deployment_id: self.deployment_id.clone(),
                completion,
            });
        }
    }
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
    #[cfg(feature = "gateway")]
    Denied,
    #[cfg(any(feature = "gateway", test))]
    Unavailable,
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

    pub(crate) async fn reserve_async(
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
                AdmissionReserve::Unavailable
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
                let deployment_id = deployment.id.clone();
                let log_deployment_id = deployment_id.clone();
                let result = run_redis(&log_deployment_id, "reserve", async move {
                    let state = pool
                        .admission_reserve(crate::storage::redis::admission::AdmissionReserveArgs {
                            key: &key,
                            max_parallel,
                            max_rpm,
                            max_tpm,
                            rpm_inc,
                            tpm_inc,
                            lease_id: &lease_id,
                            ttl_ms,
                        })
                        .await?;
                    Ok(if state.allowed {
                        // Construct the RAII hold on the I/O task. Dropping an
                        // unreceived result therefore releases the Redis slot.
                        AdmissionReserve::Granted(AdmissionHold::new(pool, lease_id, deployment_id))
                    } else {
                        AdmissionReserve::Denied
                    })
                })
                .await;
                result.unwrap_or(AdmissionReserve::Unavailable)
            }
        }
    }

    #[cfg(all(test, feature = "gateway"))]
    pub(crate) fn settle(&self, hold: &AdmissionHold, actual_tokens: u64) {
        super::sync_compat::wait(self.settle_async(hold, actual_tokens));
    }

    pub(crate) fn cancel(&self, hold: &AdmissionHold) {
        super::sync_compat::wait(self.cancel_async(hold));
    }

    pub(crate) async fn settle_async(&self, hold: &AdmissionHold, actual_tokens: u64) {
        self.finish(hold, "settle", to_i64(actual_tokens)).await;
    }

    pub(crate) async fn cancel_async(&self, hold: &AdmissionHold) {
        self.finish(hold, "cancel", 0).await;
    }

    async fn finish(&self, hold: &AdmissionHold, op: &'static str, actual_tpm: i64) {
        #[cfg(not(feature = "gateway"))]
        let _ = (hold, op, actual_tpm);
        match self {
            Self::InProcess => {}
            #[cfg(test)]
            Self::Unavailable => {}
            #[cfg(feature = "gateway")]
            Self::Redis { pool, .. } => {
                let pool = std::sync::Arc::clone(pool);
                let key =
                    crate::storage::redis::RedisPool::admission_key(&hold.inner.deployment_id);
                let hold = hold.clone();
                let deployment_id = hold.inner.deployment_id.clone();
                *hold.inner.completion.lock() = Some(if op == "settle" {
                    AdmissionCompletion::Settle(actual_tpm)
                } else {
                    AdmissionCompletion::Cancel
                });
                let _ = run_redis(&deployment_id, op, async move {
                    let result = if op == "settle" {
                        pool.admission_settle(&key, &hold.inner.lease_id, actual_tpm)
                            .await
                    } else {
                        pool.admission_cancel(&key, &hold.inner.lease_id).await
                    };
                    if result.is_ok() {
                        *hold.inner.completion.lock() = None;
                    }
                    result
                })
                .await;
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
fn admission_io_slots() -> &'static tokio::sync::Semaphore {
    static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(64);
    &SLOTS
}

#[cfg(all(test, feature = "gateway"))]
pub(crate) async fn pause_admission_io() -> tokio::sync::SemaphorePermit<'static> {
    admission_io_slots().acquire_many(64).await.unwrap()
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
    // Erase the Redis connection/Lua future before constructing the caller's
    // async frame, including its initial unpolled state. This keeps deep SDK
    // and gateway call chains within downstream crates' default type limits.
    let fut: futures::future::BoxFuture<'static, crate::utils::error::gateway_error::Result<T>> =
        Box::pin(fut);
    async move {
        // Waiters stay on the caller's runtime. Only a bounded number of
        // operations are spawned onto the connection-owning I/O runtime.
        let permit = admission_io_slots().acquire().await.map_err(|_| ())?;
        let task = admission_io_handle().spawn(async move {
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
}

#[cfg(feature = "gateway")]
struct AdmissionCleanup {
    pool: std::sync::Arc<crate::storage::redis::RedisPool>,
    lease_id: String,
    deployment_id: String,
    completion: AdmissionCompletion,
}

#[cfg(feature = "gateway")]
fn enqueue_cleanup(cleanup: AdmissionCleanup) {
    static SENDER: std::sync::OnceLock<tokio::sync::mpsc::Sender<AdmissionCleanup>> =
        std::sync::OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<AdmissionCleanup>(1_024);
        admission_io_handle().spawn(async move {
            while let Some(cleanup) = receiver.recv().await {
                let key = crate::storage::redis::RedisPool::admission_key(&cleanup.deployment_id);
                let result = match cleanup.completion {
                    AdmissionCompletion::Cancel => {
                        cleanup.pool.admission_cancel(&key, &cleanup.lease_id).await
                    }
                    AdmissionCompletion::Settle(tokens) => {
                        cleanup
                            .pool
                            .admission_settle(&key, &cleanup.lease_id, tokens)
                            .await
                    }
                };
                if let Err(error) = result {
                    warn!(
                        deployment_id = %cleanup.deployment_id,
                        %error,
                        "admission cleanup failed; relying on lease expiry"
                    );
                }
            }
        });
        sender
    });
    if let Err(error) = sender.try_send(cleanup) {
        warn!(
            deployment_id = %error.into_inner().deployment_id,
            "admission cleanup queue unavailable; relying on lease expiry"
        );
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
            let hold = AdmissionHold::new(pool.clone(), "old".into(), deployment_id);
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
            backend.finish(&hold, op, 4).await;
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
            // A synchronous recv on the caller would prevent the peer above
            // from sending until this timeout failed on the I/O runtime.
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

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_settlement_waiter_keeps_known_usage() {
        use crate::config::models::storage::RedisConfig;
        use crate::storage::redis::{RedisPool, admission::AdmissionReserveArgs};

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
        let deployment = uuid::Uuid::new_v4().to_string();
        let key = RedisPool::admission_key(&deployment);
        let worker_pool = pool.clone();
        let worker_key = key.clone();
        // The cached admission connection must be created on its long-lived
        // owner runtime, not on this test's temporary current-thread runtime.
        let state = run_redis(&deployment, "reserve", async move {
            worker_pool
                .admission_reserve(AdmissionReserveArgs {
                    key: &worker_key,
                    max_parallel: 1,
                    max_rpm: -1,
                    max_tpm: 10,
                    rpm_inc: 0,
                    tpm_inc: 10,
                    lease_id: "settling",
                    ttl_ms: DEFAULT_LEASE_TTL_MS,
                })
                .await
        })
        .await
        .unwrap();
        assert!(state.allowed);
        let backend = AdmissionBackend::redis(pool.clone());
        let hold = AdmissionHold::new(pool.clone(), "settling".into(), deployment);

        // Saturate the bridge so settlement can be cancelled specifically
        // while awaiting an I/O permit, before a command has been spawned.
        let slots = admission_io_slots().acquire_many(64).await.unwrap();
        let mut settlement = Box::pin(backend.settle_async(&hold, 4));
        assert!(futures::poll!(settlement.as_mut()).is_pending());
        assert!(matches!(
            *hold.inner.completion.lock(),
            Some(AdmissionCompletion::Settle(4))
        ));
        drop(settlement);
        drop(hold);

        // Drop cleanup must settle actual usage even while the request's
        // permit remains unavailable; replacing it with cancel would store 0.
        let mut conn = pool.open_live_connection().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let (parallel, tokens): (i64, i64) = redis::cmd("HMGET")
                    .arg(&key)
                    .arg(&["p", "t"])
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                if parallel == 0 {
                    assert_eq!(
                        tokens, 4,
                        "known usage must not be refunded on cancellation"
                    );
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("cancelled settlement did not finish through drop cleanup");
        drop(slots);
        pool.delete(&key).await.unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn abandoned_reserve_result_releases_the_shared_slot() {
        use crate::config::models::storage::RedisConfig;
        use crate::storage::redis::{RedisPool, admission::AdmissionReserveArgs};
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
        let deployment = uuid::Uuid::new_v4().to_string();
        let key = RedisPool::admission_key(&deployment);
        let (reserved_tx, reserved_rx) = tokio::sync::oneshot::channel();
        let (deliver_tx, deliver_rx) = tokio::sync::oneshot::channel();
        let worker_pool = pool.clone();
        let worker_key = key.clone();
        let task = tokio::spawn(async move {
            run_redis("cancelled-request", "reserve", async move {
                let state = worker_pool
                    .admission_reserve(AdmissionReserveArgs {
                        key: &worker_key,
                        max_parallel: 1,
                        max_rpm: -1,
                        max_tpm: -1,
                        rpm_inc: 0,
                        tpm_inc: 0,
                        lease_id: "abandoned",
                        ttl_ms: DEFAULT_LEASE_TTL_MS,
                    })
                    .await?;
                assert!(state.allowed);
                let hold = AdmissionHold::new(worker_pool, "abandoned".into(), deployment);
                let _ = reserved_tx.send(());
                let _ = deliver_rx.await;
                Ok(hold)
            })
            .await
        });
        reserved_rx.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        deliver_tx.send(()).unwrap();

        // The result is delivered after its request was cancelled. The hold
        // must be cleaned up promptly, not wait for its ten-minute lease TTL.
        let mut conn = pool.open_live_connection().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let active: i64 = redis::cmd("HGET")
                    .arg(&key)
                    .arg("p")
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                if active == 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("abandoned admission hold was not released");
        pool.delete(&key).await.unwrap();
    }
}
