//! Concurrency and edge case tests for the routing module
//!
//! Covers gaps identified in issue #216:
//! 1. Concurrent `select_deployment` under DashMap contention
//! 2. `set_model_list` atomicity with concurrent readers
//! 3. Weighted random statistical distribution verification
//! 4. EMA latency calculation edge cases (overflow, boundary values)
//! 5. Cooldown expiry race conditions

use super::router_tests::create_test_deployment;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::{
    Deployment, DeploymentConfig, DeploymentState, HealthStatus,
};
use crate::core::router::strategy_impl::{
    RoutingContext, build_routing_contexts, weighted_random_from_context,
};
use crate::core::router::unified::Router;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

// ====================================================================================
// 1. Concurrent select_deployment under DashMap contention
// ====================================================================================

#[tokio::test]
async fn test_concurrent_select_deployment_simple_shuffle() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    }));

    for i in 0..5 {
        let d = create_test_deployment(&format!("d-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        router.add_deployment(d);
    }

    let mut handles = Vec::new();
    for _ in 0..20 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let mut results = Vec::new();
            for _ in 0..50 {
                match r.select_deployment("gpt-4") {
                    Ok(id) => {
                        results.push(id.clone());
                        r.release_deployment(&id);
                    }
                    Err(e) => panic!("select_deployment failed: {:?}", e),
                }
            }
            results
        }));
    }

    let mut all_results = Vec::new();
    for handle in handles {
        all_results.extend(handle.await.unwrap());
    }

    // All 1000 selections should succeed and return valid deployment IDs
    assert_eq!(all_results.len(), 1000);
    for id in &all_results {
        assert!(id.starts_with("d-"), "unexpected deployment id: {}", id);
    }
}

#[tokio::test]
async fn test_concurrent_select_deployment_round_robin() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    }));

    for i in 0..3 {
        let d = create_test_deployment(&format!("rr-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        router.add_deployment(d);
    }

    let mut handles = Vec::new();
    for _ in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let mut results = Vec::new();
            for _ in 0..100 {
                match r.select_deployment("gpt-4") {
                    Ok(id) => {
                        results.push(id.clone());
                        r.release_deployment(&id);
                    }
                    Err(e) => panic!("round robin select_deployment failed: {:?}", e),
                }
            }
            results
        }));
    }

    let mut all_results = Vec::new();
    for handle in handles {
        all_results.extend(handle.await.unwrap());
    }

    // All 1000 should succeed
    assert_eq!(all_results.len(), 1000);

    // Each deployment should be selected at least once
    let mut counts: HashMap<String, usize> = HashMap::new();
    for id in &all_results {
        *counts.entry(id.clone()).or_default() += 1;
    }
    assert_eq!(counts.len(), 3, "all 3 deployments should be selected");
    for (id, count) in &counts {
        assert!(
            *count > 100,
            "deployment {} selected only {} times, expected > 100",
            id,
            count
        );
    }
}

#[tokio::test]
async fn test_concurrent_select_deployment_least_busy() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::LeastBusy,
        ..Default::default()
    }));

    for i in 0..4 {
        let d = create_test_deployment(&format!("lb-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        router.add_deployment(d);
    }

    let mut handles = Vec::new();
    for _ in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..50 {
                match r.select_deployment("gpt-4") {
                    Ok(id) => {
                        // Hold deployment briefly to create contention
                        tokio::task::yield_now().await;
                        r.release_deployment(&id);
                    }
                    Err(e) => panic!("least busy select_deployment failed: {:?}", e),
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // After all done, active_requests should be 0 for all deployments
    for i in 0..4 {
        let id = format!("lb-{}", i);
        if let Some(d) = router.get_deployment(&id) {
            assert_eq!(
                d.state.active_requests.load(Ordering::Relaxed),
                0,
                "deployment {} should have 0 active requests after test",
                id
            );
        }
    }
}

#[tokio::test]
async fn test_concurrent_select_deployment_enforces_max_parallel_requests() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    }));

    let d = create_test_deployment("limited-1", "gpt-4")
        .await
        .with_config(DeploymentConfig {
            max_parallel_requests: Some(1),
            ..Default::default()
        });
    d.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    router.add_deployment(d);

    let barrier = Arc::new(tokio::sync::Barrier::new(101));
    let mut handles = Vec::new();

    for _ in 0..100 {
        let r = router.clone();
        let b = barrier.clone();
        handles.push(tokio::spawn(async move {
            b.wait().await;
            r.select_deployment("gpt-4").ok()
        }));
    }

    barrier.wait().await;

    let mut selected_ids = Vec::new();
    for handle in handles {
        if let Some(id) = handle.await.unwrap() {
            selected_ids.push(id);
        }
    }

    assert_eq!(
        selected_ids.len(),
        1,
        "only one concurrent selector should reserve the single parallel slot"
    );

    let d = router.get_deployment("limited-1").unwrap();
    assert_eq!(d.state.active_requests.load(Ordering::Relaxed), 1);
    drop(d);

    router.release_deployment(&selected_ids[0]);

    let d = router.get_deployment("limited-1").unwrap();
    assert_eq!(d.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_aborted_execute_once_releases_parallel_slot() {
    let router = Arc::new(Router::default());
    let d = create_test_deployment("abort-1", "gpt-4")
        .await
        .with_config(DeploymentConfig {
            max_parallel_requests: Some(1),
            ..Default::default()
        });
    router.add_deployment(d);

    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let handle = {
        let r = router.clone();
        tokio::spawn(async move {
            r.execute_once("gpt-4", move |_deployment_id| async move {
                let _ = entered_tx.send(());
                std::future::pending::<
                    Result<((), u64), crate::core::providers::unified_provider::ProviderError>,
                >()
                .await
            })
            .await
        })
    };

    entered_rx
        .await
        .expect("operation should start after deployment selection");

    let d = router.get_deployment("abort-1").unwrap();
    assert_eq!(d.state.active_requests.load(Ordering::Relaxed), 1);
    drop(d);

    handle.abort();
    let cancelled = handle.await.expect_err("task should be cancelled");
    assert!(cancelled.is_cancelled());

    for _ in 0..10 {
        if router
            .get_deployment("abort-1")
            .map(|d| d.state.active_requests.load(Ordering::Relaxed))
            == Some(0)
        {
            return;
        }
        tokio::task::yield_now().await;
    }

    let d = router.get_deployment("abort-1").unwrap();
    assert_eq!(d.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_concurrent_record_success_and_failure() {
    let router = Arc::new(Router::new(RouterConfig {
        allowed_fails: 1000, // high to prevent cooldown
        min_requests: 1,
        ..Default::default()
    }));
    let d = create_test_deployment("d-1", "gpt-4").await;
    router.add_deployment(d);

    let mut handles = Vec::new();

    // 10 tasks recording success
    for _ in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                r.record_success("d-1", 10, 1000);
            }
        }));
    }

    // 10 tasks recording failure
    for _ in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                r.record_failure("d-1");
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    if let Some(d) = router.get_deployment("d-1") {
        let total = d.state.total_requests.load(Ordering::Relaxed);
        let successes = d.state.success_requests.load(Ordering::Relaxed);
        let failures = d.state.fail_requests.load(Ordering::Relaxed);

        // 10 * 100 successes + 10 * 100 failures = 2000 total
        assert_eq!(total, 2000, "total_requests should be 2000");
        assert_eq!(successes, 1000, "success_requests should be 1000");
        assert_eq!(failures, 1000, "fail_requests should be 1000");
    } else {
        panic!("Deployment not found");
    }
}

// ====================================================================================
// 2. set_model_list atomicity with concurrent readers
// ====================================================================================

#[tokio::test]
async fn test_set_model_list_with_concurrent_readers() {
    let router = Arc::new(Router::default());

    // Seed initial deployments
    for i in 0..3 {
        let d = create_test_deployment(&format!("old-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        router.add_deployment(d);
    }

    // Spawn readers that continuously read models/deployments
    let reader_router = router.clone();
    let reader_handle = tokio::spawn(async move {
        let mut observations = 0u64;
        for _ in 0..500 {
            let models = reader_router.list_models();
            let deployments = reader_router.list_deployments();

            // Models list should never be empty (either old or new deployments exist)
            // With the entry-by-entry swap, there's no point where all entries are removed.
            // The deployments should always include at least some entries.
            assert!(
                !deployments.is_empty() || models.is_empty(),
                "deployments should not be empty when models exist"
            );
            observations += 1;
            tokio::task::yield_now().await;
        }
        observations
    });

    // Give readers time to start
    tokio::task::yield_now().await;

    // Swap model list
    let mut new_deployments = Vec::new();
    for i in 0..3 {
        let d = create_test_deployment(&format!("new-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        new_deployments.push(d);
    }
    router.set_model_list(new_deployments);

    let observations = reader_handle.await.unwrap();
    assert!(observations > 0, "reader should have made observations");

    // After swap, only new deployments should exist
    let final_deployments = router.list_deployments();
    assert_eq!(final_deployments.len(), 3);
    for id in &final_deployments {
        assert!(
            id.starts_with("new-"),
            "expected new deployment, got: {}",
            id
        );
    }
}

#[tokio::test]
async fn test_set_model_list_with_concurrent_selectors() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    }));

    // Seed initial deployments
    for i in 0..3 {
        let d = create_test_deployment(&format!("init-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        router.add_deployment(d);
    }

    // Spawn selectors
    let mut handles = Vec::new();
    for _ in 0..5 {
        let r = router.clone();
        handles.push(tokio::spawn(async move {
            let mut success_count = 0;
            let mut error_count = 0;
            for _ in 0..200 {
                match r.select_deployment("gpt-4") {
                    Ok(id) => {
                        success_count += 1;
                        r.release_deployment(&id);
                    }
                    // During swap, NoAvailableDeployment or ModelNotFound can occur transiently
                    Err(_) => {
                        error_count += 1;
                    }
                }
                tokio::task::yield_now().await;
            }
            (success_count, error_count)
        }));
    }

    // Perform swap mid-flight
    tokio::task::yield_now().await;
    let mut new_deployments = Vec::new();
    for i in 0..3 {
        let d = create_test_deployment(&format!("swapped-{}", i), "gpt-4").await;
        d.state
            .health
            .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
        new_deployments.push(d);
    }
    router.set_model_list(new_deployments);

    let mut total_success = 0;
    let mut total_error = 0;
    for handle in handles {
        let (s, e) = handle.await.unwrap();
        total_success += s;
        total_error += e;
    }

    // The vast majority should succeed; transient errors during swap are acceptable
    assert!(
        total_success > total_error * 10,
        "too many errors: {} success vs {} errors",
        total_success,
        total_error
    );
}

// ====================================================================================
// 3. Weighted random statistical distribution verification
// ====================================================================================

#[tokio::test]
async fn test_weighted_random_statistical_distribution() {
    use dashmap::DashMap;

    let deployments = DashMap::new();

    // d1: weight=50, d2: weight=30, d3: weight=20
    let configs = [("d1", 50u32), ("d2", 30), ("d3", 20)];

    for (id, weight) in &configs {
        let config = DeploymentConfig {
            weight: *weight,
            ..Default::default()
        };
        let d = Deployment {
            id: id.to_string(),
            provider: crate::core::providers::Provider::OpenAI(
                crate::core::providers::openai::OpenAIProvider::with_api_key(
                    "sk-test-key-for-unit-testing-only",
                )
                .await
                .unwrap(),
            ),
            model: "gpt-4".to_string(),
            model_name: "gpt-4".to_string(),
            config,
            state: DeploymentState::new(),
            tags: vec![],
        };
        deployments.insert(id.to_string(), d);
    }

    let candidates: Vec<String> = configs.iter().map(|(id, _)| id.to_string()).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);

    let iterations = 10_000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        *counts.entry(selected.clone()).or_default() += 1;
    }

    let d1_pct = counts.get("d1").copied().unwrap_or(0) as f64 / iterations as f64;
    let d2_pct = counts.get("d2").copied().unwrap_or(0) as f64 / iterations as f64;
    let d3_pct = counts.get("d3").copied().unwrap_or(0) as f64 / iterations as f64;

    // Expected: d1=50%, d2=30%, d3=20%, with tolerance of +/-5%
    assert!(
        (d1_pct - 0.50).abs() < 0.05,
        "d1 expected ~50%, got {:.1}%",
        d1_pct * 100.0
    );
    assert!(
        (d2_pct - 0.30).abs() < 0.05,
        "d2 expected ~30%, got {:.1}%",
        d2_pct * 100.0
    );
    assert!(
        (d3_pct - 0.20).abs() < 0.05,
        "d3 expected ~20%, got {:.1}%",
        d3_pct * 100.0
    );
}

#[test]
fn test_weighted_random_single_weight_dominates() {
    let candidate_ids = ["heavy".to_string(), "light".to_string()];
    let contexts: Vec<RoutingContext<'_>> = vec![
        RoutingContext {
            deployment_id: &candidate_ids[0],
            weight: 1000,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
        RoutingContext {
            deployment_id: &candidate_ids[1],
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
    ];

    let iterations = 5_000;
    let mut heavy_count = 0;

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        if *selected == "heavy" {
            heavy_count += 1;
        }
    }

    let heavy_pct = heavy_count as f64 / iterations as f64;
    // Expected: heavy = 1000/1001 ~ 99.9%
    assert!(
        heavy_pct > 0.98,
        "heavy deployment should get >98%, got {:.1}%",
        heavy_pct * 100.0
    );
}

#[test]
fn test_weighted_random_equal_weights_uniform() {
    let candidate_ids: Vec<String> = (0..4).map(|i| format!("eq-{}", i)).collect();
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    let iterations = 10_000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        *counts.entry(selected.clone()).or_default() += 1;
    }

    // Each should get ~25% +/- 5%
    for (id, count) in &counts {
        let pct = *count as f64 / iterations as f64;
        assert!(
            (pct - 0.25).abs() < 0.05,
            "{} expected ~25%, got {:.1}%",
            id,
            pct * 100.0
        );
    }
}

#[test]
fn test_weighted_random_u32_max_weight() {
    // Test that very large weights don't overflow
    let candidate_ids = ["big".to_string(), "small".to_string()];
    let contexts: Vec<RoutingContext<'_>> = vec![
        RoutingContext {
            deployment_id: &candidate_ids[0],
            weight: u32::MAX / 2,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
        RoutingContext {
            deployment_id: &candidate_ids[1],
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
    ];

    // Should not panic due to overflow
    for _ in 0..100 {
        let selected = weighted_random_from_context(&contexts);
        assert!(selected.is_some());
    }
}

// ====================================================================================
// 4. EMA latency calculation edge cases
// ====================================================================================

#[tokio::test]
async fn test_ema_latency_first_measurement() {
    let d = create_test_deployment("ema-1", "gpt-4").await;

    // First measurement: avg should equal the measurement itself
    d.record_success(100, 5000);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), 5000);
}

#[tokio::test]
async fn test_ema_latency_converges() {
    let d = create_test_deployment("ema-2", "gpt-4").await;

    // Seed with initial value
    d.record_success(100, 1000);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), 1000);

    // Apply same value many times: should converge to that value
    for _ in 0..100 {
        d.record_success(100, 500);
    }

    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // After many iterations of recording 500, the EMA should converge near 500
    assert!(
        (499..=510).contains(&avg),
        "EMA should converge to ~500, got {}",
        avg
    );
}

#[tokio::test]
async fn test_ema_latency_zero_measurement() {
    let d = create_test_deployment("ema-3", "gpt-4").await;

    // First measurement is 1000
    d.record_success(100, 1000);

    // Record zero latency: EMA = (0 + 4 * 1000) / 5 = 800
    d.record_success(100, 0);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    assert_eq!(avg, 800, "EMA after zero: expected 800, got {}", avg);
}

#[tokio::test]
async fn test_ema_latency_large_values() {
    let d = create_test_deployment("ema-4", "gpt-4").await;

    // Use large but not overflowing values
    // u64::MAX / 5 is safe for the EMA formula: (new + 4 * old) / 5
    let large_val = u64::MAX / 10;
    d.record_success(100, large_val);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), large_val);

    // Next measurement should compute without overflow
    d.record_success(100, large_val);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (large_val + 4 * large_val) / 5 = large_val
    assert_eq!(
        avg, large_val,
        "EMA with large equal values should stay stable"
    );
}

#[tokio::test]
async fn test_ema_latency_spike_dampened() {
    let d = create_test_deployment("ema-5", "gpt-4").await;

    // Establish baseline of 1000
    d.record_success(100, 1000);

    // Record a huge spike
    d.record_success(100, 100_000);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (100_000 + 4 * 1000) / 5 = 20_800
    assert_eq!(avg, 20_800, "spike should be dampened by EMA, got {}", avg);

    // Record normal value again
    d.record_success(100, 1000);
    let avg2 = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (1000 + 4 * 20_800) / 5 = 16_840
    assert_eq!(avg2, 16_840, "should continue dampening, got {}", avg2);
}

#[tokio::test]
async fn test_ema_concurrent_updates() {
    let d = Arc::new(create_test_deployment("ema-c", "gpt-4").await);

    // Seed initial
    d.record_success(100, 1000);

    let mut handles = Vec::new();
    for _ in 0..10 {
        let d_clone = d.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                d_clone.record_success(10, 1000);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total = d.state.total_requests.load(Ordering::Relaxed);
    // 1 initial + 10 * 100 = 1001
    assert_eq!(total, 1001, "total_requests should be 1001, got {}", total);

    // The EMA should be near 1000 since all measurements were 1000
    // Note: due to non-atomic read-modify-write in EMA, the exact value
    // may have minor drift under concurrency. The important thing is no panic.
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    assert!(
        avg > 500 && avg < 2000,
        "EMA should be near 1000 despite concurrency, got {}",
        avg
    );
}

// ====================================================================================
// 5. Cooldown expiry race conditions
// ====================================================================================

#[tokio::test]
async fn test_cooldown_expiry_transitions_to_degraded() {
    let d = create_test_deployment("cd-1", "gpt-4").await;

    // Enter cooldown for 0 seconds (immediate expiry)
    d.enter_cooldown(0);
    assert_eq!(
        d.state.health.load(Ordering::Relaxed),
        HealthStatus::Cooldown as u8
    );

    // Wait a moment to ensure timestamp passes
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // is_in_cooldown should return false AND transition health to Degraded
    assert!(!d.is_in_cooldown());
    assert_eq!(
        d.state.health.load(Ordering::Relaxed),
        HealthStatus::Degraded as u8
    );

    // Degraded is healthy, so deployment should be selectable
    assert!(d.is_healthy());
}

#[tokio::test]
async fn test_cooldown_expiry_concurrent_check() {
    let d = Arc::new(create_test_deployment("cd-2", "gpt-4").await);

    // Enter very short cooldown
    d.enter_cooldown(0);

    // Wait for expiry
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Multiple concurrent tasks check is_in_cooldown simultaneously
    let mut handles = Vec::new();
    for _ in 0..20 {
        let d_clone = d.clone();
        handles.push(tokio::spawn(async move { d_clone.is_in_cooldown() }));
    }

    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // All should report not in cooldown (cooldown expired)
    for (i, result) in results.iter().enumerate() {
        assert!(
            !result,
            "task {} should see expired cooldown, but got in_cooldown=true",
            i
        );
    }

    // After concurrent checks, health should be Degraded (CAS ensures single transition)
    assert_eq!(
        d.state.health.load(Ordering::Relaxed),
        HealthStatus::Degraded as u8
    );
}

#[tokio::test]
async fn test_cooldown_expiry_with_concurrent_selection() {
    let router = Arc::new(Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        cooldown_time_secs: 0, // immediate expiry
        allowed_fails: 1,
        min_requests: 1,
        ..Default::default()
    }));

    let d = create_test_deployment("cd-sel-1", "gpt-4").await;
    d.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    router.add_deployment(d);

    // Force into cooldown with 0-second duration
    router.record_success("cd-sel-1", 100, 1000); // need a success for rpm_current
    router.record_failure("cd-sel-1");

    // Wait for cooldown to expire
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Concurrent tasks should all be able to select the deployment
    let mut handles = Vec::new();
    for _ in 0..10 {
        let r = router.clone();
        handles.push(tokio::spawn(async move { r.select_deployment("gpt-4") }));
    }

    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }

    // At least some should succeed since cooldown expired
    assert!(
        success_count > 0,
        "at least some selections should succeed after cooldown expiry"
    );
}

#[tokio::test]
async fn test_cooldown_reentry_during_expiry() {
    let d = create_test_deployment("cd-re", "gpt-4").await;

    // Enter cooldown for 0 seconds
    d.enter_cooldown(0);
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // First check: transitions Cooldown -> Degraded
    assert!(!d.is_in_cooldown());
    assert_eq!(
        d.state.health.load(Ordering::Relaxed),
        HealthStatus::Degraded as u8
    );

    // Re-enter cooldown while in Degraded state
    d.enter_cooldown(3600); // 1 hour

    assert!(d.is_in_cooldown());
    assert_eq!(
        d.state.health.load(Ordering::Relaxed),
        HealthStatus::Cooldown as u8
    );
}

// ====================================================================================
// Additional edge cases
// ====================================================================================

#[tokio::test]
async fn test_add_and_remove_deployment_concurrently() {
    let router = Arc::new(Router::default());

    // Add initial deployments
    for i in 0..5 {
        let d = create_test_deployment(&format!("base-{}", i), "gpt-4").await;
        router.add_deployment(d);
    }

    let mut handles = Vec::new();

    // Task adding deployments
    let r = router.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..50 {
            let d = create_test_deployment(&format!("added-{}", i), "gpt-4").await;
            r.add_deployment(d);
            tokio::task::yield_now().await;
        }
    }));

    // Task removing deployments
    let r = router.clone();
    handles.push(tokio::spawn(async move {
        for i in 0..5 {
            r.remove_deployment(&format!("base-{}", i));
            tokio::task::yield_now().await;
        }
    }));

    // Task reading deployments
    let r = router.clone();
    handles.push(tokio::spawn(async move {
        for _ in 0..100 {
            let _ = r.list_deployments();
            let _ = r.list_models();
            tokio::task::yield_now().await;
        }
    }));

    for handle in handles {
        handle.await.unwrap();
    }

    // All base deployments should be removed
    for i in 0..5 {
        assert!(
            router.get_deployment(&format!("base-{}", i)).is_none(),
            "base-{} should have been removed",
            i
        );
    }

    // All added deployments should exist
    for i in 0..50 {
        assert!(
            router.get_deployment(&format!("added-{}", i)).is_some(),
            "added-{} should exist",
            i
        );
    }
}

#[tokio::test]
async fn test_select_deployment_all_in_cooldown() {
    let router = Router::new(RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    });

    for i in 0..3 {
        let d = create_test_deployment(&format!("cool-{}", i), "gpt-4").await;
        d.enter_cooldown(3600); // 1 hour cooldown
        router.add_deployment(d);
    }

    let result = router.select_deployment("gpt-4");
    assert!(result.is_err(), "should error when all are in cooldown");
}

#[tokio::test]
async fn test_release_deployment_saturating_sub() {
    let router = Router::default();
    let d = create_test_deployment("sat-1", "gpt-4").await;
    d.state.active_requests.store(0, Ordering::Relaxed);
    router.add_deployment(d);

    // Release when already at 0 should not underflow
    router.release_deployment("sat-1");

    if let Some(d) = router.get_deployment("sat-1") {
        assert_eq!(
            d.state.active_requests.load(Ordering::Relaxed),
            0,
            "active_requests should stay at 0 (saturating sub)"
        );
    }
}

#[tokio::test]
async fn test_release_nonexistent_deployment() {
    let router = Router::default();

    // Should not panic
    router.release_deployment("does-not-exist");
}

#[tokio::test]
async fn test_record_on_nonexistent_deployment() {
    let router = Router::default();

    // None of these should panic
    router.record_success("nope", 100, 1000);
    router.record_failure("nope");
}

#[tokio::test]
async fn test_concurrent_alias_resolution() {
    let router = Arc::new(Router::default());

    let d = create_test_deployment("alias-d", "gpt-4").await;
    d.state
        .health
        .store(HealthStatus::Healthy as u8, Ordering::Relaxed);
    router.add_deployment(d);

    let _ = router.add_model_alias("gpt4", "gpt-4");
    let _ = router.add_model_alias("gpt-4-latest", "gpt-4");

    let mut handles = Vec::new();
    let aliases = ["gpt4", "gpt-4-latest", "gpt-4"];

    for alias in &aliases {
        let r = router.clone();
        let a = alias.to_string();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let result = r.select_deployment(&a);
                assert!(result.is_ok(), "select via alias '{}' should succeed", a);
                r.release_deployment(&result.unwrap());
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
