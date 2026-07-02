use super::*;

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
