use super::*;

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
