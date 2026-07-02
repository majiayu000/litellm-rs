use super::*;

// ====================================================================================
// rate_limit_aware Tests
// ====================================================================================

#[tokio::test]
async fn test_rate_limit_aware_single_candidate() {
    let deployments = DashMap::new();
    let config = DeploymentConfig {
        tpm_limit: Some(1000),
        rpm_limit: Some(100),
        ..Default::default()
    };
    deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = rate_limit_aware_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_rate_limit_aware_selects_most_headroom() {
    let deployments = DashMap::new();

    // d1: 80% TPM usage (little headroom)
    let config1 = DeploymentConfig {
        tpm_limit: Some(1000),
        rpm_limit: Some(100),
        ..Default::default()
    };
    let d1 = create_test_deployment("d1", config1).await;
    d1.state.tpm_current.store(800, Relaxed);
    d1.state.rpm_current.store(20, Relaxed);
    deployments.insert("d1".to_string(), d1);

    // d2: 20% TPM usage (lots of headroom)
    let config2 = DeploymentConfig {
        tpm_limit: Some(1000),
        rpm_limit: Some(100),
        ..Default::default()
    };
    let d2 = create_test_deployment("d2", config2).await;
    d2.state.tpm_current.store(200, Relaxed);
    d2.state.rpm_current.store(20, Relaxed);
    deployments.insert("d2".to_string(), d2);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = rate_limit_aware_from_context(&contexts).unwrap();

    // d2 has more headroom
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_rate_limit_aware_considers_rpm() {
    let deployments = DashMap::new();

    // d1: Low TPM usage but high RPM usage
    let config1 = DeploymentConfig {
        tpm_limit: Some(1000),
        rpm_limit: Some(100),
        ..Default::default()
    };
    let d1 = create_test_deployment("d1", config1).await;
    d1.state.tpm_current.store(100, Relaxed);
    d1.state.rpm_current.store(90, Relaxed); // 90% RPM usage
    deployments.insert("d1".to_string(), d1);

    // d2: Moderate usage on both
    let config2 = DeploymentConfig {
        tpm_limit: Some(1000),
        rpm_limit: Some(100),
        ..Default::default()
    };
    let d2 = create_test_deployment("d2", config2).await;
    d2.state.tpm_current.store(400, Relaxed); // 40% TPM
    d2.state.rpm_current.store(40, Relaxed); // 40% RPM
    deployments.insert("d2".to_string(), d2);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = rate_limit_aware_from_context(&contexts).unwrap();

    // d2 should win because d1 is constrained by RPM (10% headroom vs 60%)
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_rate_limit_aware_no_limits() {
    let deployments = DashMap::new();

    // No limits = maximum distance (1.0)
    let config = DeploymentConfig {
        tpm_limit: None,
        rpm_limit: None,
        ..Default::default()
    };
    deployments.insert(
        "d1".to_string(),
        create_test_deployment("d1", config.clone()).await,
    );
    deployments.insert("d2".to_string(), create_test_deployment("d2", config).await);

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = rate_limit_aware_from_context(&contexts).unwrap();

    // Both have maximum distance, first one wins
    assert_eq!(selected, "d1");
}

#[test]
fn test_rate_limit_aware_empty_candidates() {
    assert!(rate_limit_aware_from_context(&[]).is_none());
}
