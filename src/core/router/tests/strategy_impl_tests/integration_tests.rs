use super::*;

// ====================================================================================
// Integration Tests
// ====================================================================================

#[tokio::test]
async fn test_strategy_consistency() {
    // Verify that with same input, deterministic strategies produce same output
    let deployments = DashMap::new();

    let config1 = DeploymentConfig {
        weight: 1,
        priority: 10,
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d1 = create_test_deployment("d1", config1).await;
    d1.state.tpm_current.store(500, Relaxed);
    d1.state.active_requests.store(5, Relaxed);
    d1.state.avg_latency_us.store(100, Relaxed);
    deployments.insert("d1".to_string(), d1);

    let config2 = DeploymentConfig {
        weight: 1,
        priority: 1,
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d2 = create_test_deployment("d2", config2).await;
    d2.state.tpm_current.store(100, Relaxed);
    d2.state.active_requests.store(2, Relaxed);
    d2.state.avg_latency_us.store(200, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);

    // Deterministic strategies should consistently return same result
    for _ in 0..10 {
        // least_busy always picks d2 (2 active vs 5)
        assert_eq!(least_busy_from_context(&contexts).unwrap(), "d2");

        // lowest_usage always picks d2 (10% vs 50%)
        assert_eq!(lowest_usage_from_context(&contexts).unwrap(), "d2");

        // lowest_latency always picks d1 (100us vs 200us)
        assert_eq!(lowest_latency_from_context(&contexts).unwrap(), "d1");

        // lowest_priority always picks d2 (priority 1 vs 10)
        assert_eq!(lowest_priority_from_context(&contexts).unwrap(), "d2");
    }
}
