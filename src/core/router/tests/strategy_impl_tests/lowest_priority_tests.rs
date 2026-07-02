use super::*;

// ====================================================================================
// lowest_priority Tests
// ====================================================================================

#[tokio::test]
async fn test_lowest_priority_single_candidate() {
    let deployments = DashMap::new();
    let config = DeploymentConfig {
        priority: 5,
        ..Default::default()
    };
    deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_priority_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_lowest_priority_selects_lowest_priority() {
    let deployments = DashMap::new();

    let config1 = DeploymentConfig {
        priority: 10,
        ..Default::default()
    };
    deployments.insert(
        "d1".to_string(),
        create_test_deployment("d1", config1).await,
    );

    let config2 = DeploymentConfig {
        priority: 1,
        ..Default::default()
    };
    deployments.insert(
        "d2".to_string(),
        create_test_deployment("d2", config2).await,
    );

    let config3 = DeploymentConfig {
        priority: 5,
        ..Default::default()
    };
    deployments.insert(
        "d3".to_string(),
        create_test_deployment("d3", config3).await,
    );

    let candidates = vec!["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_priority_from_context(&contexts).unwrap();

    // d2 has the lowest priority value
    assert_eq!(selected, "d2");
}

#[tokio::test]
async fn test_lowest_priority_all_same_priority() {
    let deployments = DashMap::new();
    for i in 1..=3 {
        let config = DeploymentConfig {
            priority: 5,
            ..Default::default()
        };
        deployments.insert(
            format!("d{}", i),
            create_test_deployment(&format!("d{}", i), config).await,
        );
    }

    let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = lowest_priority_from_context(&contexts).unwrap();

    // First one wins when all have same priority
    assert_eq!(selected, "d1");
}

#[test]
fn test_lowest_priority_empty_candidates() {
    assert!(lowest_priority_from_context(&[]).is_none());
}
