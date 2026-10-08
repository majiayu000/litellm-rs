//! Native completion keeps authoritative optional usage across accounting hooks.

use super::{StreamingDeploymentLease, completion};
use crate::config::models::storage::RedisConfig;
use crate::core::providers::{Provider, ProviderError, openai::OpenAIProvider};
use crate::core::router::{Deployment, DeploymentConfig, RouterConfig, UnifiedRouter};
use crate::storage::redis::RedisPool;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

async fn native_lease(
    pool: Option<Arc<RedisPool>>,
) -> (
    Arc<UnifiedRouter>,
    Arc<Deployment>,
    StreamingDeploymentLease,
) {
    let router = UnifiedRouter::new(RouterConfig {
        allowed_fails: 100,
        min_requests: 100,
        ..Default::default()
    });
    let router = Arc::new(match pool {
        Some(pool) => router.with_admission_redis(pool),
        None => router,
    });
    let id = uuid::Uuid::new_v4().to_string();
    router.add_deployment(
        Deployment::new(
            id.clone(),
            Provider::OpenAI(
                OpenAIProvider::with_api_key("sk-native-test")
                    .await
                    .unwrap(),
            ),
            "gpt-4o-mini".into(),
            "native".into(),
        )
        .with_config(DeploymentConfig {
            rpm_limit: Some(100),
            max_parallel_requests: Some(100),
            tpm_limit: Some(32),
            ..Default::default()
        }),
    );
    let deployment = router.get_deployment(&id).unwrap();
    let mut selected = router
        .select_deployment_lease_with_tokens("native", 20)
        .unwrap();
    let (admission, hold) = selected.take_admission();
    let _ = selected.into_deployment_id();
    let lease = StreamingDeploymentLease::new(
        router.clone(),
        deployment.clone(),
        Instant::now(),
        admission,
        hold,
    );
    (router, deployment, lease)
}

async fn assert_shared_usage(pool: &RedisPool, id: &str, tokens: u64) {
    let key = RedisPool::admission_key(id);
    let mut conn = pool.open_live_connection().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let counts: (Option<i64>, Option<i64>, Option<i64>) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "r", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            let fields: Vec<String> = redis::cmd("HKEYS")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap();
            if counts == (Some(0), Some(1), Some(tokens as i64))
                && !fields.iter().any(|field| field.starts_with("l:"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("native completion must release concurrency and preserve known/unknown usage");
}

fn assert_local_outcome(deployment: &Deployment, mode: &str, usage: Option<u64>) {
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(
        deployment.state.tpm_current.load(Ordering::Relaxed),
        usage.unwrap_or(0)
    );
    assert_eq!(
        deployment.state.total_requests.load(Ordering::Relaxed),
        u64::from(mode != "disconnect")
    );
    assert_eq!(
        deployment.state.success_requests.load(Ordering::Relaxed),
        u64::from(mode == "success")
    );
    assert_eq!(
        deployment.state.fail_requests.load(Ordering::Relaxed),
        u64::from(mode == "failure")
    );
}

#[tokio::test]
async fn native_stream_accounting_preserves_optional_usage_and_outcome_once() {
    for shared in [false, true] {
        let pool = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                continue;
            };
            Some(Arc::new(
                RedisPool::new(&RedisConfig {
                    url,
                    enabled: true,
                    allow_degraded: false,
                    ..Default::default()
                })
                .await
                .unwrap(),
            ))
        } else {
            None
        };
        for usage in [None, Some(0), Some(3)] {
            for mode in ["success", "failure", "disconnect"] {
                for wait_mode in ["normal", "cancel", "budget_cancel"] {
                    let cancel = wait_mode != "normal";
                    let (router, deployment, mut lease) = native_lease(pool.clone()).await;
                    let error = ProviderError::api_error("responses", 502, "native failure");
                    let mut wait = Box::pin(lease.settle_native_stream(
                        usage,
                        mode != "disconnect",
                        (mode == "failure").then_some(&error),
                        async {
                            // Native settlement may run legacy budget hooks. Neither
                            // their zero nor their success hint replaces native facts.
                            completion::observe_usage(0);
                            let budget_wait = (wait_mode != "cancel").then(|| {
                                completion::provider_succeeded();
                                completion::BudgetSettlementWait::new()
                            });
                            if cancel {
                                std::future::pending::<()>().await;
                            }
                            if let Some(budget_wait) = budget_wait {
                                budget_wait.finish();
                            }
                            17
                        },
                    ));
                    if cancel {
                        assert!(futures::poll!(wait.as_mut()).is_pending());
                    } else {
                        assert_eq!(wait.as_mut().await, 17);
                    }
                    drop(wait);
                    assert!(lease.finalized);
                    assert_local_outcome(&deployment, mode, usage);

                    // A caller can keep its &mut lease after dropping the wait.
                    // Neither a second native finish nor a numeric one may recount.
                    lease
                        .settle_native_stream(Some(999), true, None, async {})
                        .await;
                    lease.complete_response(999, None).await;
                    drop(lease);
                    assert_local_outcome(&deployment, mode, usage);
                    if let Some(pool) = &pool {
                        assert_shared_usage(pool, &deployment.id, usage.unwrap_or(20)).await;
                    }

                    // Only TPM can reject this probe: RPM/parallel remain below
                    // 100 and the single failure is below cooldown thresholds.
                    let next = router.select_deployment_lease_with_tokens("native", 20);
                    assert_eq!(
                        next.is_ok(),
                        usage.is_some(),
                        "shared={shared}, usage={usage:?}, mode={mode}, wait_mode={wait_mode}"
                    );
                    drop(next);
                    assert_local_outcome(&deployment, mode, usage);
                    if let Some(pool) = &pool {
                        assert_shared_usage(pool, &deployment.id, usage.unwrap_or(20)).await;
                        pool.delete(&RedisPool::admission_key(&deployment.id))
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }
}

async fn large_borrowed_settlement(marker: &u8) {
    let payload = [0_u8; 16 * 1024];
    std::future::pending::<()>().await;
    std::hint::black_box((payload, marker));
}

#[tokio::test]
async fn native_stream_settlement_is_lazy_and_boxes_borrowed_futures() {
    let (router, deployment, mut lease) = native_lease(None).await;
    let marker = 7;
    let wait = lease.settle_native_stream(None, false, None, large_borrowed_settlement(&marker));
    assert!(std::mem::size_of_val(&wait) < 1024);
    drop(wait);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
    drop(lease);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    let next = router
        .select_deployment_lease_with_tokens("native", 20)
        .expect("an unpolled settlement must not invent a consumed response");
    drop(next);
    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
}
