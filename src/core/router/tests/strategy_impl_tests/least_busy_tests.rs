use super::*;

// ====================================================================================
// least_busy Tests
// ====================================================================================

#[tokio::test]
async fn test_least_busy_single_candidate() {
    let deployments = DashMap::new();
    let config = DeploymentConfig::default();
    deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = least_busy_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_least_busy_selects_lowest_active() {
    let deployments = DashMap::new();

    let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
    d1.state.active_requests.store(10, Relaxed);
    deployments.insert("d1".to_string(), d1);

    let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
    d2.state.active_requests.store(5, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
    d3.state.active_requests.store(15, Relaxed);
    deployments.insert("d3".to_string(), d3);

    let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = least_busy_from_context(&contexts).unwrap();

    // d2 has the fewest active requests
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_least_busy_with_ties() {
    let deployments = DashMap::new();

    let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
    d1.state.active_requests.store(5, Relaxed);
    deployments.insert("d1".to_string(), d1);

    let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
    d2.state.active_requests.store(5, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);

    // Result should be one of the tied deployments
    for _ in 0..10 {
        let selected = least_busy_from_context(&contexts).unwrap();
        assert!(selected == "d1" || selected == "d2");
    }
}

#[tokio::test]
async fn test_least_busy_all_zero() {
    let deployments = DashMap::new();
    for i in 1..=3 {
        let d = create_test_deployment(&format!("d{}", i), DeploymentConfig::default()).await;
        d.state.active_requests.store(0, Relaxed);
        deployments.insert(format!("d{}", i), d);
    }

    let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = least_busy_from_context(&contexts).unwrap();
    assert!(candidates.contains(selected));
}

#[test]
fn test_least_busy_empty_candidates() {
    assert!(least_busy_from_context(&[]).is_none());
}
