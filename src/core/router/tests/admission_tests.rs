use super::router_tests::create_test_deployment;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::{Deployment, DeploymentConfig};
use crate::core::router::error::RouterError;
use crate::core::router::unified::Router;
#[cfg(feature = "gateway")]
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
    use crate::core::router::selection::DeploymentLease;
    use crate::core::types::model::ProviderCapability;
    use crate::storage::redis::RedisPool;

    #[tokio::test(flavor = "current_thread")]
    async fn legacy_id_selectors_release_shared_admission_before_returning() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        for capability in [false, true] {
            let id = unique("legacy-id");
            let a = router_with(pool.clone());
            let b = router_with(pool.clone());
            seed(&a, &id, Some(1), None, None).await;
            seed(&b, &id, Some(1), None, None).await;
            let selected = if capability {
                a.select_deployment_lease_for_capability(
                    "gpt-4",
                    &ProviderCapability::ChatCompletion,
                )
            } else {
                a.select_deployment_lease("gpt-4")
            }
            .map(DeploymentLease::into_deployment_id)
            .expect("legacy selector can reserve inside a current-thread runtime");
            assert_eq!(
                a.get_deployment(&id)
                    .unwrap()
                    .state
                    .active_requests
                    .load(Ordering::Relaxed),
                1
            );
            Router::release_selected_deployment(&a.get_deployment(&selected).unwrap());
            assert_eq!(
                a.get_deployment(&id)
                    .unwrap()
                    .state
                    .active_requests
                    .load(Ordering::Relaxed),
                0
            );
            let second = if capability {
                b.select_deployment_lease_for_capability(
                    "gpt-4",
                    &ProviderCapability::ChatCompletion,
                )
            } else {
                b.select_deployment_lease("gpt-4")
            }
            .map(DeploymentLease::into_deployment_id)
            .expect("legacy release must leave the shared parallel slot reusable immediately");
            Router::release_selected_deployment(&b.get_deployment(&second).unwrap());
            drop(
                a.select_deployment_lease("gpt-4")
                    .expect("owned admission remains reusable"),
            );
            cleanup(&pool, &id).await;
        }
    }

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

    #[tokio::test(flavor = "current_thread")]
    async fn synchronous_admission_works_inside_a_futures_executor() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("nested-executor");
        let a = router_with(pool.clone());
        let b = router_with(pool.clone());
        seed(&a, &id, Some(1), None, Some(10)).await;
        seed(&b, &id, Some(1), None, Some(10)).await;

        futures::executor::block_on(async {
            let mut lease = a
                .select_deployment_lease_with_tokens("gpt-4", 10)
                .expect("nested executor can reserve shared admission");
            assert!(b.select_deployment_lease_with_tokens("gpt-4", 1).is_err());
            lease.commit_admission(4);
            drop(lease);
            drop(
                b.select_deployment_lease_with_tokens("gpt-4", 6)
                    .expect("sync settlement records actual tokens"),
            );
            drop(
                a.select_deployment_lease_with_tokens("gpt-4", 6)
                    .expect("sync drop refunds the unused reservation"),
            );
        });
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

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_provider_failure_keeps_local_outcome_before_circuit_dispatch() {
        use crate::core::providers::unified_provider::ProviderError;
        const CHILD: &str = "LITELLM_PROVIDER_FAILURE_CIRCUIT_TEST_CHILD";
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{module}::cancelled_provider_failure_keeps_local_outcome_before_circuit_dispatch"
        );
        if std::env::var(CHILD).as_deref() != Ok(name.as_str()) {
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", &name, "--nocapture", "--test-threads=1"])
                    .env(CHILD, &name)
                    .output()
                    .unwrap()
            })
            .await
            .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed; 0 failed"),
                "isolated circuit cancellation regression: {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        for (execution, retryable) in [
            ("once", false),
            ("retry", false),
            ("retry", true),
            ("stream", false),
            ("stream", true),
        ] {
            let id = unique("cancel-failure");
            let router = Arc::new(router_with(pool.clone()).with_circuit_redis(pool.clone()));
            seed(&router, &id, Some(1), Some(10), Some(100)).await;
            let (started, ready) = tokio::sync::oneshot::channel();
            let started = Arc::new(std::sync::Mutex::new(Some(started)));
            let worker = router.clone();
            let task = tokio::spawn(async move {
                let operation = move |_| {
                    let started = started.clone();
                    async move {
                        let slots = crate::core::router::circuit::pause_circuit_io().await;
                        started.lock().unwrap().take().unwrap().send(slots).unwrap();
                        let error = if retryable {
                            ProviderError::network("fixture", "connection lost")
                        } else {
                            ProviderError::authentication("fixture", "denied")
                        };
                        Err::<((), u64), _>(error)
                    }
                };
                if execution == "retry" {
                    let _ = worker
                        .execute_with_selected_deployment_retry("gpt-4", operation)
                        .await;
                } else if execution == "stream" {
                    let _ = crate::core::router::RuntimeBinding::new(worker.clone())
                        .bind()
                        .execute_stream_with_selected_deployment_capability_typed(
                            "gpt-4",
                            &ProviderCapability::ChatCompletionStream,
                            0,
                            move |deployment| {
                                let future = operation(deployment);
                                async move { future.await.map(|_| ()) }
                            },
                        )
                        .await;
                } else {
                    let _ = worker
                        .execute_once_with_selected_deployment("gpt-4", operation)
                        .await;
                }
            });
            let slots = tokio::time::timeout(std::time::Duration::from_secs(10), ready)
                .await
                .unwrap()
                .unwrap();
            let deployment = router.get_deployment(&id).unwrap();
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 0);
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            drop(slots);
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 1);
            let mut conn = pool.open_live_connection().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let state: (i64, i64, i64) = ::redis::cmd("HMGET")
                        .arg(RedisPool::admission_key(&id))
                        .arg(&["p", "r", "t"])
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    if state == (0, 0, 0) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("cancelled provider failure must refund routing admission");
            assert!(
                !pool.exists(&RedisPool::circuit_key(&id)).await.unwrap(),
                "cancelled wait before dispatch must not invent a shared failure ACK"
            );
            cleanup(&pool, &id).await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_success_settlement_retains_local_counts_for_all_execution_paths() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        for execution in ["once", "retry", "stream"] {
            let id = unique("cancel-success");
            let router = Arc::new(router_with(pool.clone()).with_circuit_redis(pool.clone()));
            seed(&router, &id, Some(1), Some(10), Some(100)).await;
            let (started, ready) = tokio::sync::oneshot::channel();
            let started = Arc::new(std::sync::Mutex::new(Some(started)));
            let worker = router.clone();
            let task = tokio::spawn(async move {
                if execution == "stream" {
                    let lease = worker.select_deployment_lease_async("gpt-4").await.unwrap();
                    let handle = crate::core::router::RuntimeBinding::new(worker.clone()).bind();
                    let mut completion = handle.stream_completion(lease, std::time::Instant::now());
                    completion.observe_usage(42);
                    let _slots = crate::core::router::admission::pause_admission_io().await;
                    let _ = started.lock().unwrap().take().unwrap().send(());
                    completion.finish_success().await;
                    return Ok(());
                }
                let operation = move |_| {
                    let started = started.clone();
                    async move {
                        // Reserve has finished. Hold every I/O permit in the
                        // result so the real settlement suspends until abort.
                        let slots = crate::core::router::admission::pause_admission_io().await;
                        let _ = started.lock().unwrap().take().unwrap().send(());
                        Ok((slots, 42))
                    }
                };
                if execution == "retry" {
                    worker
                        .execute_with_selected_deployment_retry("gpt-4", operation)
                        .await
                        .map(|_| ())
                } else {
                    worker
                        .execute_once_with_selected_deployment("gpt-4", operation)
                        .await
                        .map(|_| ())
                        .map_err(crate::core::router::execution::router_error_to_provider_error)
                        .map_err(|e| (e, 1))
                }
            });
            tokio::time::timeout(std::time::Duration::from_secs(10), ready)
                .await
                .unwrap()
                .unwrap();
            let mut conn = pool.open_live_connection().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let successes: Option<i64> = ::redis::cmd("HGET")
                        .arg(RedisPool::circuit_key(&id))
                        .arg("r")
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    if successes == Some(1) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("successful circuit outcome must precede blocked admission settlement");
            let before: (i64, i64) = ::redis::cmd("HMGET")
                .arg(RedisPool::circuit_key(&id))
                .arg(&["tot", "r"])
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(before, (1, 1));
            let deployment = router.get_deployment(&id).unwrap();
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 42);
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
            // RAII must also retain actual shared usage after the waiter dies.
            let mut conn = pool.open_live_connection().await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let state: (i64, i64) = ::redis::cmd("HMGET")
                        .arg(RedisPool::admission_key(&id))
                        .arg(&["p", "t"])
                        .query_async(&mut conn)
                        .await
                        .unwrap();
                    if state == (0, 42) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            let after: (i64, i64) = ::redis::cmd("HMGET")
                .arg(RedisPool::circuit_key(&id))
                .arg(&["tot", "r"])
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(after, (1, 1), "abort must not record circuit success twice");
            cleanup(&pool, &id).await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn resource_rotation_isolates_shared_admission_and_cached_circuit_state() {
        use crate::config::models::provider::ProviderConfig;
        use crate::core::router::runtime_state::GatewayRuntimeIdentity;

        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("resource-rotation");
        let router = router_with(pool.clone()).with_circuit_redis(pool.clone());
        let mut config = ProviderConfig {
            api_key: "retired-test-credential".into(),
            ..ProviderConfig::default()
        };
        let old_identity = GatewayRuntimeIdentity::for_provider(&config).for_model("gpt-4");
        let mut old = limited_deployment(&id, Some(1), Some(1), Some(10)).await;
        old.state.runtime_identity = Some(old_identity);
        router.add_deployment(old);
        let mut lease = router.select_deployment_lease_async("gpt-4").await.unwrap();
        let old_shared_id = lease.deployment().shared_state_id();
        router.record_failure_with_reason(&id, crate::core::router::CooldownReason::AuthError);
        assert!(router.select_deployment_lease_async("gpt-4").await.is_err());

        config.api_key = "rotated-test-credential".into();
        let new_identity = GatewayRuntimeIdentity::for_provider(&config).for_model("gpt-4");
        let mut replacement = limited_deployment(&id, Some(1), Some(1), Some(10)).await;
        replacement.state.runtime_identity = Some(new_identity.clone());
        let mut replacement_router = router_with(pool.clone());
        // A real gateway revision inherits state by construction identity.
        // Retain the backend cache too, so cached old cooldown cannot hide a
        // correct Redis namespace behind an ID-only local lookup.
        replacement_router.circuit = router.circuit.clone();
        replacement_router.add_deployment(replacement);
        replacement_router.inherit_runtime_state(&router);
        let mut replacement_lease = replacement_router
            .select_deployment_lease_async("gpt-4")
            .await
            .expect("rotated resource must not inherit the old lease or cached auth cooldown");
        let peer = router_with(pool.clone()).with_circuit_redis(pool.clone());
        let mut same_resource = limited_deployment(&id, Some(1), Some(1), Some(10)).await;
        same_resource.state.runtime_identity = Some(new_identity);
        peer.add_deployment(same_resource);
        assert!(
            peer.select_deployment_lease_async("gpt-4").await.is_err(),
            "replicas of the same replacement still share the parallel limit"
        );
        let new_shared_id = replacement_lease.deployment().shared_state_id();
        lease.commit_admission_async(7).await;
        replacement_lease.commit_admission_async(4).await;
        let mut conn = pool.open_live_connection().await.unwrap();
        for (shared_id, tokens) in [(&old_shared_id, 7), (&new_shared_id, 4)] {
            let state: (i64, i64) = ::redis::cmd("HMGET")
                .arg(RedisPool::admission_key(shared_id))
                .arg(&["p", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            assert_eq!(
                state,
                (0, tokens),
                "each hold finishes in its original resource namespace"
            );
            cleanup(&pool, shared_id).await;
            pool.delete(&RedisPool::circuit_key(shared_id))
                .await
                .unwrap();
        }
        drop(replacement_lease);
        drop(lease);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cleanup_capacity_is_reserved_before_admission_and_known_usage_is_retained() {
        use crate::core::router::admission::{AdmissionBackend, AdmissionReserve};

        // Match the budget bridge's subprocess isolation: this test owns every
        // process-wide cleanup slot without rejecting unrelated parallel tests.
        const CHILD: &str = "LITELLM_ADMISSION_CLEANUP_TEST_CHILD";
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!(
            "{module}::cleanup_capacity_is_reserved_before_admission_and_known_usage_is_retained"
        );
        if std::env::var(CHILD).as_deref() != Ok(name.as_str()) {
            let output = tokio::task::spawn_blocking(move || {
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--exact", &name, "--nocapture", "--test-threads=1"])
                    .env(CHILD, &name)
                    .output()
                    .unwrap()
            })
            .await
            .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed; 0 failed"),
                "isolated cleanup regression must run once and pass:\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let id = unique("cleanup-capacity");
        let deployment = limited_deployment(&id, Some(2_000), None, Some(100_000)).await;
        let backend = AdmissionBackend::redis(pool.clone());
        let mut conn = pool.open_live_connection().await.unwrap();
        let (seconds, micros): (u64, u64) =
            ::redis::cmd("TIME").query_async(&mut conn).await.unwrap();
        // The exact TPM assertion belongs to one Redis minute. Completed
        // usage expires at rollover, unlike outstanding reservations.
        let remaining_ms = 60_000 - (seconds % 60 * 1_000 + micros / 1_000);
        if remaining_ms < 15_000 {
            tokio::time::sleep(std::time::Duration::from_millis(remaining_ms + 1)).await;
        }
        let mut holds = Vec::new();
        for _ in 0..1_024 {
            match backend.reserve_async(&deployment, 1).await {
                AdmissionReserve::Granted(hold) => holds.push(hold),
                _ => panic!("cleanup capacity must accommodate 1024 owned holds"),
            }
        }
        assert!(
            matches!(
                backend.reserve_async(&deployment, 1).await,
                AdmissionReserve::Unavailable
            ),
            "saturated cleanup capacity must fail closed before accepting more liabilities"
        );
        for hold in &holds {
            hold.prepare_settlement(2);
        }
        drop(holds);
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let (parallel, tokens): (i64, i64) = ::redis::cmd("HMGET")
                    .arg(RedisPool::admission_key(&id))
                    .arg(&["p", "t"])
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                if parallel == 0 {
                    assert_eq!(
                        tokens, 2_048,
                        "every queued completion must retain actual usage"
                    );
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("all owned cleanup events must finish");
        let AdmissionReserve::Granted(hold) = backend.reserve_async(&deployment, 1).await else {
            panic!("terminal cleanup releases process admission capacity");
        };
        backend.cancel_async(&hold).await;
        drop(hold);
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
