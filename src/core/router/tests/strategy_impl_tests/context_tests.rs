use super::*;

#[tokio::test]
async fn test_build_routing_contexts_skips_missing_deployments() {
    let deployments = DashMap::new();
    let config = DeploymentConfig {
        weight: 3,
        priority: 7,
        ..Default::default()
    };
    let deployment = create_test_deployment("d1", config).await;
    deployment.state.active_requests.store(2, Relaxed);
    deployment.state.tpm_current.store(120, Relaxed);
    deployment.state.rpm_current.store(12, Relaxed);
    deployment.state.avg_latency_us.store(55, Relaxed);
    deployments.insert("d1".to_string(), deployment);

    let candidates = vec!["d1".to_string(), "missing".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].deployment_id, "d1");
    assert_eq!(contexts[0].weight, 3);
    assert_eq!(contexts[0].priority, 7);
    assert_eq!(contexts[0].active_requests, 2);
    assert_eq!(contexts[0].tpm_current, 120);
    assert_eq!(contexts[0].rpm_current, 12);
    assert_eq!(contexts[0].avg_latency_us, 55);
}
