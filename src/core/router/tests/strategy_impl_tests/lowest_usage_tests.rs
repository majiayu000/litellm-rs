use super::*;

// ====================================================================================
// lowest_usage Tests
// ====================================================================================

#[tokio::test]
async fn test_lowest_usage_single_candidate() {
    let deployments = DashMap::new();
    let config = DeploymentConfig {
        tpm_limit: Some(1000),
        ..Default::default()
    };
    deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_usage_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_lowest_usage_selects_lowest_percentage() {
    let deployments = DashMap::new();

    // d1: 50% usage (500/1000)
    let config1 = DeploymentConfig {
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d1 = create_test_deployment("d1", config1).await;
    d1.state.tpm_current.store(500, Relaxed);
    deployments.insert("d1".to_string(), d1);

    // d2: 20% usage (200/1000)
    let config2 = DeploymentConfig {
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d2 = create_test_deployment("d2", config2).await;
    d2.state.tpm_current.store(200, Relaxed);
    deployments.insert("d2".to_string(), d2);

    // d3: 80% usage (800/1000)
    let config3 = DeploymentConfig {
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d3 = create_test_deployment("d3", config3).await;
    d3.state.tpm_current.store(800, Relaxed);
    deployments.insert("d3".to_string(), d3);

    let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_usage_from_context(&contexts).unwrap();

    // d2 has the lowest usage percentage
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_lowest_usage_no_limit_treated_as_zero() {
    let deployments = DashMap::new();

    // d1 has no limit (0% usage)
    let config1 = DeploymentConfig {
        tpm_limit: None,
        ..Default::default()
    };
    let d1 = create_test_deployment("d1", config1).await;
    deployments.insert("d1".to_string(), d1);

    // d2 has 50% usage
    let config2 = DeploymentConfig {
        tpm_limit: Some(1000),
        ..Default::default()
    };
    let d2 = create_test_deployment("d2", config2).await;
    d2.state.tpm_current.store(500, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_usage_from_context(&contexts).unwrap();

    // d1 has 0% usage (no limit)
    assert_eq!(selected, "d1");
}

#[test]
fn test_lowest_usage_empty_candidates() {
    assert!(lowest_usage_from_context(&[]).is_none());
}
