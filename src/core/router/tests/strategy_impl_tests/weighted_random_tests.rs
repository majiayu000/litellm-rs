use super::*;

// ====================================================================================
// weighted_random Tests
// ====================================================================================

#[tokio::test]
async fn test_weighted_random_single_candidate() {
    let deployments = DashMap::new();
    let config = DeploymentConfig {
        weight: 1,
        ..Default::default()
    };
    deployments.insert("d1".to_string(), create_test_deployment("d1", config).await);

    let candidates = vec!["d1".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);
    let selected = weighted_random_from_context(&contexts).unwrap();
    assert_eq!(selected, "d1");
}

#[tokio::test]
async fn test_weighted_random_returns_valid_candidate() {
    let deployments = DashMap::new();
    for i in 1..=3 {
        let config = DeploymentConfig {
            weight: 1,
            ..Default::default()
        };
        deployments.insert(
            format!("d{}", i),
            create_test_deployment(&format!("d{}", i), config).await,
        );
    }

    let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);

    // Run multiple times and verify result is always in candidates
    for _ in 0..100 {
        let selected = weighted_random_from_context(&contexts).unwrap();
        assert!(candidates.contains(selected));
    }
}

#[tokio::test]
async fn test_weighted_random_respects_weights() {
    let deployments = DashMap::new();

    // d1 has weight 10, d2 has weight 1
    let config1 = DeploymentConfig {
        weight: 10,
        ..Default::default()
    };
    let config2 = DeploymentConfig {
        weight: 1,
        ..Default::default()
    };
    deployments.insert(
        "d1".to_string(),
        create_test_deployment("d1", config1).await,
    );
    deployments.insert(
        "d2".to_string(),
        create_test_deployment("d2", config2).await,
    );

    let candidates = vec!["d1".to_string(), "d2".to_string()];
    let contexts = build_routing_contexts(&candidates, &deployments);

    let mut d1_count = 0;
    let mut d2_count = 0;

    for _ in 0..1000 {
        let selected = weighted_random_from_context(&contexts).unwrap();
        if selected == "d1" {
            d1_count += 1;
        } else {
            d2_count += 1;
        }
    }

    // d1 should be selected significantly more often (roughly 10x)
    assert!(
        d1_count > d2_count * 5,
        "d1 should be selected much more often due to higher weight"
    );
}

#[tokio::test]
async fn test_weighted_random_all_zero_weights() {
    let deployments = DashMap::new();
    for i in 1..=3 {
        let config = DeploymentConfig {
            weight: 0,
            ..Default::default()
        };
        deployments.insert(
            format!("d{}", i),
            create_test_deployment(&format!("d{}", i), config).await,
        );
    }

    let candidates: Vec<String> = (1..=3).map(|i| format!("d{}", i)).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);

    // Should fall back to uniform random
    for _ in 0..10 {
        let selected = weighted_random_from_context(&contexts).unwrap();
        assert!(candidates.contains(selected));
    }
}

#[test]
fn test_weighted_random_empty_candidates() {
    assert!(weighted_random_from_context(&[]).is_none());
}
