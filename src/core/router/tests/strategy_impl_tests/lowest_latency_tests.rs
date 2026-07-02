use super::*;

// ====================================================================================
// lowest_latency Tests
// ====================================================================================

#[tokio::test]
async fn test_lowest_latency_single_candidate() {
    let deployments = DashMap::new();
    let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
    d1.state.avg_latency_us.store(100, Relaxed);
    deployments.insert("d1".to_string(), d1);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_latency_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_lowest_latency_selects_fastest() {
    let deployments = DashMap::new();

    let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
    d1.state.avg_latency_us.store(500, Relaxed);
    deployments.insert("d1".to_string(), d1);

    let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
    d2.state.avg_latency_us.store(100, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
    d3.state.avg_latency_us.store(300, Relaxed);
    deployments.insert("d3".to_string(), d3);

    let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_latency_from_context(&contexts).unwrap();

    // d2 has the lowest latency
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_lowest_latency_new_deployment_uses_average() {
    let deployments = DashMap::new();

    // d1 has measured latency
    let d1 = create_test_deployment("d1", DeploymentConfig::default()).await;
    d1.state.avg_latency_us.store(1000, Relaxed);
    deployments.insert("d1".to_string(), d1);

    // d2 is new (latency = 0, will use average)
    let d2 = create_test_deployment("d2", DeploymentConfig::default()).await;
    d2.state.avg_latency_us.store(0, Relaxed);
    deployments.insert("d2".to_string(), d2);

    // d3 has high latency
    let d3 = create_test_deployment("d3", DeploymentConfig::default()).await;
    d3.state.avg_latency_us.store(2000, Relaxed);
    deployments.insert("d3".to_string(), d3);

    let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_latency_from_context(&contexts).unwrap();

    // d1 has the lowest actual latency (1000)
    // d2 gets average = (1000 + 2000) / 2 = 1500
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_lowest_latency_all_zero() {
    let deployments = DashMap::new();
    for i in 1..=3 {
        let d = create_test_deployment(&format!("d{}", i), DeploymentConfig::default()).await;
        d.state.avg_latency_us.store(0, Relaxed);
        deployments.insert(format!("d{}", i), d);
    }

    let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_latency_from_context(&contexts).unwrap();

    // Any candidate is valid when all have zero latency
    assert!(candidates.contains(selected));
}

#[test]
fn test_lowest_latency_empty_candidates() {
    assert!(lowest_latency_from_context(&[]).is_none());
}
