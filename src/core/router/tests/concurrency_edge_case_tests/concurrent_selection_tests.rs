use super::*;

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
