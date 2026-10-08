//! Cancelled circuit publication must not erase a newer local failure.
use super::router_tests::create_test_deployment;
use crate::config::models::storage::RedisConfig;
use crate::core::router::circuit::pause_circuit_io;
use crate::core::router::config::RouterConfig;
use crate::core::router::deployment::{Deployment, HealthStatus};
use crate::core::router::error::CooldownReason;
use crate::core::router::unified::Router;
use crate::storage::redis::RedisPool;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

#[tokio::test(flavor = "current_thread")]
async fn cancelled_failure_survives_old_shared_success_and_observation() {
    const CHILD: &str = "LITELLM_CIRCUIT_SNAPSHOT_FENCE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // Pausing all bridge slots is process-local: do not delay unrelated
        // tests that run concurrently in the normal Cargo test process.
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "core::router::tests::circuit_snapshot_tests::cancelled_failure_survives_old_shared_success_and_observation",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed; 0 failed"),
            "isolated snapshot regression must pass once:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let Some(pool) = live_redis_pool().await else {
        return;
    };
    for old_success in [false, true] {
        for reason in [CooldownReason::RateLimit, CooldownReason::AuthError] {
            for cancel in [false, true] {
                let check_expiry =
                    cancel && old_success && matches!(reason, CooldownReason::AuthError);
                let router = Router::new(RouterConfig {
                    cooldown_time_secs: if check_expiry { 5 } else { 60 },
                    allowed_fails: 2,
                    min_requests: 2,
                    success_threshold: 1,
                    ..Default::default()
                })
                .with_circuit_redis(pool.clone());
                let id = format!("snapshot-fence-{}", uuid::Uuid::new_v4());
                router.add_deployment(create_test_deployment(&id, "gpt-4").await);
                let deployment = router.get_deployment(&id).unwrap();
                wait_for_window(&pool).await;
                deployment.state.reset_minute();
                let pause = pause_circuit_io().await;
                let mut old = Box::pin(async {
                    if old_success {
                        router
                            .record_success_for_deployment_async(&deployment, 3, 1_000)
                            .await;
                    } else {
                        let _ = router.deployment_is_selectable_async(&deployment).await;
                    }
                });
                assert!(futures::poll!(old.as_mut()).is_pending());
                router.record_local_failure(&deployment, reason);
                let deadline = deployment.state.cooldown_until.load(Relaxed);
                let mut failure = Box::pin(
                    router.record_failure_circuit_for_deployment_async(&deployment, reason),
                );
                assert!(futures::poll!(failure.as_mut()).is_pending());
                let failure = if cancel {
                    // No Redis task has accepted the known local failure.
                    drop(failure);
                    None
                } else {
                    Some(failure)
                };
                drop(pause);
                old.await;
                if let Some(failure) = failure {
                    failure.await;
                }
                assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
                assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 1);
                assert!(deployment.state.cooldown_until.load(Relaxed) >= deadline);
                assert_eq!(deployment.state.health_status(), HealthStatus::Cooldown);
                assert!(router.select_deployment_lease_async("gpt-4").await.is_err());
                // Force a fresh Redis observation, then exercise the cache.
                tokio::time::sleep(Duration::from_millis(80)).await;
                assert!(!router.deployment_is_selectable_async(&deployment).await);
                assert!(!router.deployment_is_selectable_async(&deployment).await);
                assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 1);
                assert_eq!(deployment.state.consecutive_successes.load(Relaxed), 0);
                let mut reloaded = (*deployment).clone();
                reloaded.state = deployment.state.for_snapshot_insertion();
                assert!(!router.deployment_is_selectable_async(&reloaded).await);
                assert!(reloaded.state.cooldown_until.load(Relaxed) >= deadline);

                if check_expiry {
                    // A new success that starts after the failure also sees
                    // missing Redis history; it cannot shorten local cooldown.
                    router
                        .record_success_for_deployment_async(&reloaded, 3, 1_000)
                        .await;
                    assert!(!router.deployment_is_selectable_async(&deployment).await);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    drop(
                        router
                            .select_deployment_lease_async("gpt-4")
                            .await
                            .expect("the true local deadline restores real selection"),
                    );
                    assert_eq!(deployment.state.health_status(), HealthStatus::Degraded);
                    assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
                    assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
                    router
                        .record_success_for_deployment_async(&reloaded, 3, 1_000)
                        .await;
                    assert_eq!(deployment.state.health_status(), HealthStatus::Healthy);
                }
                // A closed observe does not create Redis history. If the
                // failure never reached Redis and no success preceded it,
                // HGET legitimately returns nil rather than the string "0".
                let mut connection = pool.open_live_connection().await.unwrap();
                let shared_fail: Option<u64> = redis::cmd("HGET")
                    .arg(RedisPool::circuit_key(&deployment.shared_state_id()))
                    .arg("fail")
                    .query_async(&mut connection)
                    .await
                    .unwrap();
                let expected_fail = if cancel && !old_success {
                    None
                } else {
                    Some(u64::from(!cancel))
                };
                assert_eq!(shared_fail, expected_fail);
                pool.delete(&RedisPool::circuit_key(&deployment.shared_state_id()))
                    .await
                    .unwrap();
            }
        }
    }

    // A cancelled subthreshold failure must remain available to the next
    // failure threshold check, even across a later fresh healthy observation.
    let router = Router::new(RouterConfig {
        allowed_fails: 2,
        min_requests: 2,
        cooldown_time_secs: 5,
        ..Default::default()
    })
    .with_circuit_redis(pool.clone());
    let id = format!("snapshot-threshold-{}", uuid::Uuid::new_v4());
    router.add_deployment(create_test_deployment(&id, "gpt-4").await);
    let deployment = router.get_deployment(&id).unwrap();
    wait_for_window(&pool).await;
    deployment.state.reset_minute();
    cancel_local_failure(&router, &deployment, CooldownReason::ConsecutiveFailures).await;
    assert!(router.deployment_is_selectable_async(&deployment).await);
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 1);
    assert_eq!(deployment.state.health_status(), HealthStatus::Degraded);
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(router.deployment_is_selectable_async(&deployment).await);
    cancel_local_failure(&router, &deployment, CooldownReason::ConsecutiveFailures).await;
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 2);
    assert!(router.select_deployment_lease_async("gpt-4").await.is_err());
    tokio::time::sleep(Duration::from_secs(5)).await;
    drop(router.select_deployment_lease_async("gpt-4").await.unwrap());
    // Two resets within the same wall-clock second must still invalidate the
    // old minute's lower bound, without clearing an independent cooldown early.
    deployment.state.reset_minute();
    assert!(router.deployment_is_selectable_async(&deployment).await);
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 0);
    cancel_local_failure(&router, &deployment, CooldownReason::ConsecutiveFailures).await;
    deployment.state.reset_minute();
    cancel_local_failure(&router, &deployment, CooldownReason::ConsecutiveFailures).await;
    assert!(router.deployment_is_selectable_async(&deployment).await);
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 1);
    assert_eq!(deployment.state.fail_requests.load(Relaxed), 4);
    assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
    pool.delete(&RedisPool::circuit_key(&deployment.shared_state_id()))
        .await
        .unwrap();
}

async fn cancel_local_failure(router: &Router, deployment: &Deployment, reason: CooldownReason) {
    let pause = pause_circuit_io().await;
    router.record_local_failure(deployment, reason);
    let mut publish =
        Box::pin(router.record_failure_circuit_for_deployment_async(deployment, reason));
    assert!(futures::poll!(publish.as_mut()).is_pending());
    drop(publish);
    drop(pause);
}

async fn wait_for_window(pool: &RedisPool) {
    let mut connection = redis::Client::open(pool.config.url.as_str())
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    loop {
        let (shared_seconds, _micros): (u64, u64) = redis::cmd("TIME")
            .query_async(&mut connection)
            .await
            .unwrap();
        let local_seconds = crate::core::router::deployment::current_timestamp();
        let delay = [shared_seconds % 60, local_seconds % 60]
            .into_iter()
            .filter(|second| *second >= 40)
            .map(|second| 61 - second)
            .max();
        match delay {
            Some(seconds) => tokio::time::sleep(Duration::from_secs(seconds)).await,
            None => break,
        }
    }
}

async fn live_redis_pool() -> Option<Arc<RedisPool>> {
    let redis_url = match std::env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) if std::env::var_os("CI").is_some() => panic!("CI requires actual REDIS_URL"),
        Err(_) => {
            eprintln!("Skipping shared snapshot regression: REDIS_URL is not set");
            return None;
        }
    };
    let pool = RedisPool::new(&RedisConfig {
        url: redis_url,
        enabled: true,
        max_connections: 10,
        connection_timeout: 1,
        cluster: false,
        allow_degraded: false,
    })
    .await
    .expect("configured Redis must connect");
    pool.health_check().await.expect("Redis must be healthy");
    Some(Arc::new(pool))
}

#[tokio::test]
async fn local_recovery_uses_real_successes_not_unrelated_shared_failure_counts() {
    use crate::core::router::circuit::apply_circuit_snapshot;
    use crate::storage::redis::circuit::CircuitState;

    for shared_failures in [0, 3] {
        let router = Router::new(RouterConfig {
            cooldown_time_secs: 1,
            success_threshold: 2,
            ..Default::default()
        });
        let deployment = create_test_deployment("recovery-ownership", "gpt-4").await;
        // Exercise cooldown recovery within one minute; window expiry is
        // covered separately below. Match the shared regression's margin.
        loop {
            let second = crate::core::router::deployment::current_timestamp() % 60;
            if second < 40 {
                break;
            }
            tokio::time::sleep(Duration::from_secs(61 - second)).await;
        }
        // Model a cancelled publication: only this process knows the failure.
        router.record_local_failure(&deployment, CooldownReason::RateLimit);
        let mut reloaded = deployment.clone();
        reloaded.state = deployment.state.for_snapshot_insertion();
        let healthy = CircuitState {
            status: 0,
            opened_until: 0,
            fails: shared_failures,
            consecutive_successes: 20,
            health: HealthStatus::Healthy as i64,
            owned: true,
        };
        let apply = |generation| apply_circuit_snapshot(&reloaded, &healthy, generation, 2);
        apply(reloaded.state.circuit_update_generation());
        assert!(reloaded.is_in_cooldown());
        assert_eq!(reloaded.state.consecutive_successes.load(Relaxed), 0);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        apply(reloaded.state.circuit_update_generation());
        assert!(!reloaded.is_in_cooldown());
        assert_eq!(reloaded.state.health_status(), HealthStatus::Degraded);
        assert_eq!(reloaded.state.consecutive_successes.load(Relaxed), 0);

        reloaded.record_success(1, 1);
        apply(reloaded.state.circuit_update_generation());
        assert_eq!(reloaded.state.health_status(), HealthStatus::Degraded);
        assert_eq!(reloaded.state.consecutive_successes.load(Relaxed), 1);
        let old_generation = reloaded.state.circuit_update_generation();
        reloaded.record_success(1, 1);
        apply(reloaded.state.circuit_update_generation());
        assert_eq!(reloaded.state.health_status(), HealthStatus::Healthy);
        let mut delayed_failure = healthy;
        delayed_failure.health = HealthStatus::Degraded as i64;
        delayed_failure.consecutive_successes = 0;
        apply_circuit_snapshot(&reloaded, &delayed_failure, old_generation, 2);
        assert_eq!(reloaded.state.health_status(), HealthStatus::Healthy);
        assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
    }
}

#[tokio::test]
async fn expired_failure_window_releases_local_recovery_without_masking_remote_failure() {
    use crate::core::router::circuit::apply_circuit_snapshot;
    use crate::storage::redis::circuit::CircuitState;

    let router = Router::new(RouterConfig {
        allowed_fails: 100,
        min_requests: 100,
        ..Default::default()
    });
    let deployment = create_test_deployment("recovery-window", "gpt-4").await;
    router.record_local_failure(&deployment, CooldownReason::ConsecutiveFailures);
    deployment.state.reset_minute();
    let mut remote = CircuitState {
        status: 0,
        opened_until: 0,
        fails: 0,
        consecutive_successes: 3,
        health: HealthStatus::Healthy as i64,
        owned: true,
    };
    apply_circuit_snapshot(
        &deployment,
        &remote,
        deployment.state.circuit_update_generation(),
        3,
    );
    assert_eq!(deployment.state.health_status(), HealthStatus::Healthy);
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 0);
    remote.status = 1;
    remote.opened_until = (crate::core::router::deployment::current_timestamp() + 10) as i64;
    remote.health = HealthStatus::Cooldown as i64;
    remote.fails = 8;
    apply_circuit_snapshot(
        &deployment,
        &remote,
        deployment.state.circuit_update_generation(),
        3,
    );
    assert!(deployment.is_in_cooldown());
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 8);
    assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_shared_observation_does_not_override_new_local_outcome() {
    const CHILD: &str = "LITELLM_REJECTED_CIRCUIT_OBSERVATION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = tokio::task::spawn_blocking(|| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "core::router::tests::circuit_snapshot_tests::rejected_shared_observation_does_not_override_new_local_outcome",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "isolated observation regression must pass once:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let Some(pool) = live_redis_pool().await else {
        return;
    };
    for half_open in [false, true] {
        for recover_during_observation in [false, true] {
            wait_for_window(&pool).await;
            let config = RouterConfig {
                allowed_fails: 100,
                min_requests: 100,
                success_threshold: 1,
                cooldown_time_secs: 60,
                ..Default::default()
            };
            let router = Router::new(config.clone()).with_circuit_redis(pool.clone());
            let local = Router::new(config);
            let id = format!("rejected-observation-{}", uuid::Uuid::new_v4());
            router.add_deployment(create_test_deployment(&id, "gpt-4").await);
            let deployment = router.get_deployment(&id).unwrap();
            local.record_local_failure(&deployment, CooldownReason::ConsecutiveFailures);
            assert_eq!(deployment.state.health_status(), HealthStatus::Degraded);
            let key = RedisPool::circuit_key(&deployment.shared_state_id());
            let now = crate::core::router::deployment::current_timestamp();
            let opened = if half_open { now - 1 } else { now + 60 };
            let health = if half_open {
                HealthStatus::Degraded
            } else {
                HealthStatus::Cooldown
            };
            let mut connection = pool.open_live_connection().await.unwrap();
            // Seed an open circuit or a half-open probe owned by another
            // replica. The real Lua observation must return a blocking state.
            redis::cmd("HSET")
                .arg(&key)
                .arg("opened")
                .arg(opened)
                .arg("owner")
                .arg("another-replica")
                .arg("own_until")
                .arg(now + 60)
                .arg("e")
                .arg(now / 60)
                .arg("h")
                .arg(health as u8)
                .query_async::<i64>(&mut connection)
                .await
                .unwrap();
            let generation = deployment.state.circuit_update_generation();
            let pause = pause_circuit_io().await;
            let mut observation = Box::pin(router.deployment_is_selectable_async(&deployment));
            assert!(futures::poll!(observation.as_mut()).is_pending());
            if recover_during_observation {
                // The observation captured its fence before bridge admission.
                // A real local success now advances it and completes recovery.
                local
                    .record_success_for_deployment_async(&deployment, 3, 1_000)
                    .await;
                assert!(deployment.state.circuit_update_generation() > generation);
                assert_eq!(deployment.state.health_status(), HealthStatus::Healthy);
            }
            drop(pause);
            assert!(
                !observation.await,
                "current shared restrictions must survive local recovery: half_open={half_open}, recovered={recover_during_observation}"
            );
            assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
            assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
            if recover_during_observation {
                assert_eq!(deployment.state.health_status(), health);
                assert_eq!(deployment.state.success_requests.load(Relaxed), 1);
                // The retry observes the still-current shared restriction.
                // Subsequent observations must retain the same denial.
                assert!(!router.deployment_is_selectable_async(&deployment).await);
            } else {
                assert_eq!(deployment.state.circuit_update_generation(), generation);
                assert_eq!(deployment.state.success_requests.load(Relaxed), 0);
            }
            assert!(router.select_deployment_lease_async("gpt-4").await.is_err());
            pool.delete(&key).await.unwrap();
        }
    }

    // The reverse race must remain fail-closed: an obsolete healthy
    // observation cannot erase the newer local failure or its deadline.
    wait_for_window(&pool).await;
    let router = Router::new(RouterConfig {
        cooldown_time_secs: 60,
        ..Default::default()
    })
    .with_circuit_redis(pool.clone());
    let id = format!("rejected-healthy-observation-{}", uuid::Uuid::new_v4());
    router.add_deployment(create_test_deployment(&id, "gpt-4").await);
    let deployment = router.get_deployment(&id).unwrap();
    let generation = deployment.state.circuit_update_generation();
    let pause = pause_circuit_io().await;
    let mut observation = Box::pin(router.deployment_is_selectable_async(&deployment));
    assert!(futures::poll!(observation.as_mut()).is_pending());
    router.record_local_failure(&deployment, CooldownReason::RateLimit);
    let deadline = deployment.state.cooldown_until.load(Relaxed);
    assert!(deployment.state.circuit_update_generation() > generation);
    drop(pause);
    assert!(!observation.await);
    assert_eq!(deployment.state.health_status(), HealthStatus::Cooldown);
    assert_eq!(deployment.state.cooldown_until.load(Relaxed), deadline);
    assert_eq!(deployment.state.fail_requests.load(Relaxed), 1);
    assert_eq!(deployment.state.fails_this_minute.load(Relaxed), 1);
    assert!(router.select_deployment_lease_async("gpt-4").await.is_err());
    pool.delete(&RedisPool::circuit_key(&deployment.shared_state_id()))
        .await
        .unwrap();
}

#[tokio::test]
async fn rejected_observation_retries_after_a_barrier_without_bypassing_the_fence() {
    use crate::core::router::circuit::{CircuitObserve, apply_circuit_snapshot};
    use crate::storage::redis::circuit::CircuitState;

    for keep_changing in [false, true] {
        let router = Router::new(RouterConfig {
            success_threshold: 1,
            ..Default::default()
        });
        let deployment =
            Arc::new(create_test_deployment("observation-generation-race", "gpt-4").await);
        let closed = CircuitState {
            status: 0,
            opened_until: 0,
            fails: 0,
            consecutive_successes: 1,
            health: HealthStatus::Healthy as i64,
            owned: true,
        };
        let old_open = CircuitState {
            status: 1,
            opened_until: (crate::core::router::deployment::current_timestamp() + 60) as i64,
            health: HealthStatus::Cooldown as i64,
            owned: false,
            ..closed
        };
        apply_circuit_snapshot(
            &deployment,
            &old_open,
            deployment.state.circuit_update_generation(),
            1,
        );
        assert!(deployment.is_in_cooldown());

        // The observer cannot complete until this other task has recorded a
        // real local success and applied its newer, closed shared snapshot.
        let (observe_tx, mut observe_rx) =
            tokio::sync::mpsc::unbounded_channel::<tokio::sync::oneshot::Sender<CircuitState>>();
        let recovered = Arc::clone(&deployment);
        let updater = tokio::spawn(async move {
            let mut calls = 0;
            while let Some(reply) = observe_rx.recv().await {
                calls += 1;
                let state = if calls == 1 || keep_changing {
                    recovered.record_success(3, 1_000);
                    apply_circuit_snapshot(
                        &recovered,
                        &closed,
                        recovered.state.circuit_update_generation(),
                        1,
                    );
                    old_open
                } else {
                    closed
                };
                reply.send(state).unwrap();
            }
            calls
        });
        let selectable = router
            .deployment_is_selectable_with_observer(&deployment, || {
                let observe_tx = observe_tx.clone();
                async move {
                    let (reply, observed) = tokio::sync::oneshot::channel();
                    observe_tx.send(reply).unwrap();
                    CircuitObserve::Shared(observed.await.unwrap())
                }
            })
            .await;
        drop(observe_tx);
        assert_eq!(updater.await.unwrap(), 2);
        assert_eq!(selectable, !keep_changing);
        assert_eq!(deployment.state.health_status(), HealthStatus::Healthy);
        assert!(!deployment.is_in_cooldown());
        assert_eq!(deployment.state.active_requests.load(Relaxed), 0);
        assert_eq!(deployment.state.fail_requests.load(Relaxed), 0);
        assert_eq!(
            deployment.state.success_requests.load(Relaxed),
            if keep_changing { 2 } else { 1 }
        );
    }
}
