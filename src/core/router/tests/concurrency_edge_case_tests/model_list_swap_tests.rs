use super::*;

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

    // Snapshot swaps install one complete generation, so selectors should see
    // either the old or the new deployment set without transient routing gaps.
    assert!(
        total_error == 0,
        "snapshot swap should not produce transient selector errors: {} success vs {} errors",
        total_success,
        total_error
    );
    assert!(total_success > 0, "selectors should have made progress");
}
