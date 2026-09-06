use super::router_tests::create_test_deployment;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::{Deployment, DeploymentConfig};
use crate::core::router::error::RouterError;
use crate::core::router::unified::Router;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn unavailable_backend_fails_closed_without_reservation() {
    let router = Router::new(RouterConfig::default()).with_unavailable_admission();
    let deployment = limited_deployment("closed-1", Some(1), None, None).await;
    router.add_deployment(deployment);
    let err = router
        .select_deployment_lease("gpt-4")
        .expect_err("unavailable admission must fail closed");
    assert!(matches!(err, RouterError::NoAvailableDeployment(_)));
    let deployment = router.get_deployment("closed-1").unwrap();
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
    assert!(!deployment.is_in_cooldown());
}

#[cfg(feature = "gateway")]
mod redis {
    use super::*;
    use crate::config::models::storage::RedisConfig;
    use crate::storage::redis::RedisPool;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_routers_share_max_parallel_limit() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("parallel");
        let a = Arc::new(router_with(pool.clone()));
        let b = Arc::new(router_with(pool.clone()));
        seed(&a, &id, Some(1), None, None).await;
        seed(&b, &id, Some(1), None, None).await;

        let a_task = {
            let a = Arc::clone(&a);
            tokio::spawn(async move { a.select_deployment_lease("gpt-4") })
        };
        let b_task = {
            let b = Arc::clone(&b);
            tokio::spawn(async move { b.select_deployment_lease("gpt-4") })
        };
        let a_result = a_task.await.expect("join a");
        let b_result = b_task.await.expect("join b");
        let successes = [&a_result, &b_result]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(successes, 1, "replica count must not multiply max_parallel");

        for result in [&a_result, &b_result] {
            if let Err(err) = result {
                assert!(matches!(err, RouterError::NoAvailableDeployment(_)));
            }
        }
        let denied = a.get_deployment(&id).unwrap();
        assert_eq!(denied.state.fail_requests.load(Ordering::Relaxed), 0);
        assert!(!denied.is_in_cooldown());
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_routers_share_rpm_limit() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("rpm");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id, None, Some(1), None).await;
        seed(&b, &id, None, Some(1), None).await;

        let mut first = a
            .select_deployment_lease("gpt-4")
            .expect("first rpm reserve");
        assert!(
            b.select_deployment_lease("gpt-4").is_err(),
            "outstanding rpm must block the second replica"
        );
        first.commit_admission(0);
        drop(first);
        assert!(
            b.select_deployment_lease("gpt-4").is_err(),
            "settled rpm must still occupy the minute window"
        );
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_refunds_rpm_for_the_other_replica() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("rpm-cancel");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id, None, Some(1), None).await;
        seed(&b, &id, None, Some(1), None).await;

        drop(a.select_deployment_lease("gpt-4").expect("reserve"));
        b.select_deployment_lease("gpt-4")
            .expect("cancel must refund rpm so the other replica can admit");
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tpm_estimate_is_shared_and_settled() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("tpm");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id, None, None, Some(10)).await;
        seed(&b, &id, None, None, Some(10)).await;

        let mut hold = a
            .select_deployment_lease_with_tokens("gpt-4", 10)
            .expect("first tpm reserve");
        assert!(b.select_deployment_lease_with_tokens("gpt-4", 1).is_err());
        hold.commit_admission(4);
        drop(hold);
        b.select_deployment_lease_with_tokens("gpt-4", 6)
            .expect("settled 4 of 10 should leave 6");
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn current_thread_runtime_can_admit_without_panic() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("current-thread");
        let router = router_with(pool.clone());
        seed(&router, &id, Some(1), None, None).await;
        let lease = router
            .select_deployment_lease("gpt-4")
            .expect("Actix workers use current_thread; admit must not panic");
        drop(lease);
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stream_disconnect_releases_parallel_slot() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("stream-drop");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id, Some(1), None, None).await;
        seed(&b, &id, Some(1), None, None).await;

        let lease = a.select_deployment_lease("gpt-4").expect("reserve");
        assert!(b.select_deployment_lease("gpt-4").is_err());
        drop(lease);
        b.select_deployment_lease("gpt-4")
            .expect("drop must release the shared parallel slot");
        cleanup(&pool, &id).await;
    }

    fn router_with(pool: Arc<RedisPool>) -> Router {
        Router::new(RouterConfig::default()).with_admission_redis(pool)
    }

    async fn seed(
        router: &Router,
        id: &str,
        max_parallel: Option<u32>,
        rpm: Option<u64>,
        tpm: Option<u64>,
    ) {
        router.add_deployment(limited_deployment(id, max_parallel, rpm, tpm).await);
    }

    fn unique(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn cleanup(pool: &RedisPool, deployment_id: &str) {
        let _ = pool.delete(&RedisPool::admission_key(deployment_id)).await;
    }

    async fn live_redis_pool() -> Option<Arc<RedisPool>> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let config = RedisConfig {
            url: redis_url.clone(),
            enabled: true,
            max_connections: 10,
            connection_timeout: 1,
            cluster: false,
            allow_degraded: false,
        };

        match RedisPool::new(&config).await {
            Ok(pool) => match pool.health_check().await {
                Ok(()) => Some(Arc::new(pool)),
                Err(err) => {
                    if std::env::var("CI").is_ok() {
                        panic!("Redis should pass health check in CI at {redis_url}: {err}");
                    }
                    eprintln!("Skipping distributed admission Redis integration test: {err}");
                    None
                }
            },
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should be reachable in CI at {redis_url}: {err}");
                }
                eprintln!("Skipping distributed admission Redis integration test: {err}");
                None
            }
        }
    }
}

async fn limited_deployment(
    id: &str,
    max_parallel: Option<u32>,
    rpm: Option<u64>,
    tpm: Option<u64>,
) -> Deployment {
    create_test_deployment(id, "gpt-4")
        .await
        .with_config(DeploymentConfig {
            max_parallel_requests: max_parallel,
            rpm_limit: rpm,
            tpm_limit: tpm,
            ..Default::default()
        })
}
