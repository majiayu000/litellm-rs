use super::router_tests::create_test_deployment;
use crate::core::router::config::RouterConfig;
use crate::core::router::error::{CooldownReason, RouterError};
use crate::core::router::unified::Router;

#[tokio::test]
async fn unavailable_backend_fails_closed_without_selection() {
    let a = Router::new(RouterConfig::default()).with_unavailable_circuit(false);
    let b = Router::new(RouterConfig::default()).with_unavailable_circuit(false);
    a.add_deployment(create_test_deployment("closed-1", "gpt-4").await);
    b.add_deployment(create_test_deployment("closed-1", "gpt-4").await);
    for router in [&a, &b] {
        let err = router
            .select_deployment_lease("gpt-4")
            .expect_err("strict circuit loss must fail closed");
        assert!(matches!(err, RouterError::NoAvailableDeployment(_)));
    }
}

#[tokio::test]
async fn unavailable_backend_degraded_uses_local_snapshot() {
    let router = Router::new(RouterConfig::default()).with_unavailable_circuit(true);
    router.add_deployment(create_test_deployment("degraded-open", "gpt-4").await);
    router
        .select_deployment_lease("gpt-4")
        .expect("degraded circuit loss must use a closed local snapshot");

    let blocked = Router::new(RouterConfig::default()).with_unavailable_circuit(true);
    let deployment = create_test_deployment("degraded-open", "gpt-4").await;
    deployment.enter_cooldown(3600);
    blocked.add_deployment(deployment);
    assert!(
        blocked.select_deployment_lease("gpt-4").is_err(),
        "degraded circuit loss must honor the last local cooldown snapshot"
    );
}

#[cfg(feature = "gateway")]
mod redis {
    use super::*;
    use crate::config::models::storage::RedisConfig;
    use crate::storage::redis::RedisPool;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn node_a_open_stops_selection_on_node_b() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("open");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id).await;
        seed(&b, &id).await;

        a.record_failure_with_reason(&id, CooldownReason::RateLimit);
        assert!(
            b.select_deployment_lease("gpt-4").is_err(),
            "node B must skip a circuit opened on node A"
        );
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shared_failure_threshold_is_atomic() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("threshold");
        let config = RouterConfig {
            allowed_fails: 3,
            min_requests: 1,
            cooldown_time_secs: 60,
            ..Default::default()
        };
        let a = Router::new(config.clone()).with_circuit_redis(pool.clone());
        let b = Router::new(config).with_circuit_redis(pool.clone());
        seed(&a, &id).await;
        seed(&b, &id).await;

        a.record_failure(&id);
        a.record_failure(&id);
        assert!(a.select_deployment_lease("gpt-4").is_ok());
        b.record_failure(&id);
        assert!(
            a.select_deployment_lease("gpt-4").is_err(),
            "third shared failure must open the circuit on both nodes"
        );
        assert!(b.select_deployment_lease("gpt-4").is_err());
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cooldown_half_open_is_exclusive_then_recovers() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("half-open");
        let config = RouterConfig {
            cooldown_time_secs: 1,
            success_threshold: 1,
            ..Default::default()
        };
        let a = Router::new(config.clone()).with_circuit_redis(pool.clone());
        let b = Router::new(config).with_circuit_redis(pool.clone());
        seed(&a, &id).await;
        seed(&b, &id).await;

        a.record_failure_with_reason(&id, CooldownReason::Timeout);
        tokio::time::sleep(Duration::from_millis(1_100)).await;

        let a_lease = a.select_deployment_lease("gpt-4");
        let b_lease = b.select_deployment_lease("gpt-4");
        let successes = [&a_lease, &b_lease]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(successes, 1, "only the half-open owner may probe");

        let (owner, other) = if a_lease.is_ok() { (&a, &b) } else { (&b, &a) };
        drop(a_lease);
        drop(b_lease);
        owner.record_success(&id, 10, 1_000);
        tokio::time::sleep(Duration::from_millis(80)).await;
        other
            .select_deployment_lease("gpt-4")
            .expect("probe success must close the circuit for the other replica");
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn half_open_failure_reopens_cooldown() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("reopen");
        let config = RouterConfig {
            cooldown_time_secs: 1,
            success_threshold: 3,
            ..Default::default()
        };
        let a = Router::new(config.clone()).with_circuit_redis(pool.clone());
        let b = Router::new(config).with_circuit_redis(pool.clone());
        seed(&a, &id).await;
        seed(&b, &id).await;

        a.record_failure_with_reason(&id, CooldownReason::Manual);
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        let _probe = a
            .select_deployment_lease("gpt-4")
            .expect("cooldown expiry must allow one probe");
        a.record_failure_with_reason(&id, CooldownReason::ConsecutiveFailures);
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            b.select_deployment_lease("gpt-4").is_err(),
            "half-open failure must re-open cooldown on the other replica"
        );
        cleanup(&pool, &id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn current_thread_runtime_can_observe_without_panic() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("current-thread");
        let router = router_with(pool.clone());
        seed(&router, &id).await;
        router
            .select_deployment_lease("gpt-4")
            .expect("Actix workers use current_thread; circuit observe must not panic");
        cleanup(&pool, &id).await;
    }

    fn router_with(pool: Arc<RedisPool>) -> Router {
        Router::new(RouterConfig::default()).with_circuit_redis(pool)
    }

    async fn seed(router: &Router, id: &str) {
        router.add_deployment(create_test_deployment(id, "gpt-4").await);
    }

    fn unique(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn cleanup(pool: &RedisPool, deployment_id: &str) {
        let _ = pool.delete(&RedisPool::circuit_key(deployment_id)).await;
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
                    eprintln!("Skipping distributed circuit Redis integration test: {err}");
                    None
                }
            },
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should be reachable in CI at {redis_url}: {err}");
                }
                eprintln!("Skipping distributed circuit Redis integration test: {err}");
                None
            }
        }
    }
}
