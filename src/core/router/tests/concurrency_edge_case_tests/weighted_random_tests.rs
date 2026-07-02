use super::*;

// ====================================================================================
// 3. Weighted random statistical distribution verification
// ====================================================================================

#[tokio::test]
async fn test_weighted_random_statistical_distribution() {
    use dashmap::DashMap;

    let deployments = DashMap::new();

    // d1: weight=50, d2: weight=30, d3: weight=20
    let configs = [("d1", 50u32), ("d2", 30), ("d3", 20)];

    for (id, weight) in &configs {
        let config = DeploymentConfig {
            weight: *weight,
            ..Default::default()
        };
        let d = Deployment {
            id: id.to_string(),
            provider: crate::core::providers::Provider::OpenAI(
                crate::core::providers::openai::OpenAIProvider::with_api_key(
                    "sk-test-key-for-unit-testing-only",
                )
                .await
                .unwrap(),
            ),
            model: "gpt-4".to_string(),
            model_name: "gpt-4".to_string(),
            config,
            state: DeploymentState::new(),
            tags: vec![],
        };
        deployments.insert(id.to_string(), d);
    }

    let candidates: Vec<String> = configs.iter().map(|(id, _)| id.to_string()).collect();
    let contexts = build_routing_contexts(&candidates, &deployments);

    let iterations = 10_000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        *counts.entry(selected.clone()).or_default() += 1;
    }

    let d1_pct = counts.get("d1").copied().unwrap_or(0) as f64 / iterations as f64;
    let d2_pct = counts.get("d2").copied().unwrap_or(0) as f64 / iterations as f64;
    let d3_pct = counts.get("d3").copied().unwrap_or(0) as f64 / iterations as f64;

    // Expected: d1=50%, d2=30%, d3=20%, with tolerance of +/-5%
    assert!(
        (d1_pct - 0.50).abs() < 0.05,
        "d1 expected ~50%, got {:.1}%",
        d1_pct * 100.0
    );
    assert!(
        (d2_pct - 0.30).abs() < 0.05,
        "d2 expected ~30%, got {:.1}%",
        d2_pct * 100.0
    );
    assert!(
        (d3_pct - 0.20).abs() < 0.05,
        "d3 expected ~20%, got {:.1}%",
        d3_pct * 100.0
    );
}

#[test]
fn test_weighted_random_single_weight_dominates() {
    let candidate_ids = ["heavy".to_string(), "light".to_string()];
    let contexts: Vec<RoutingContext<'_>> = vec![
        RoutingContext {
            deployment_id: &candidate_ids[0],
            weight: 1000,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
        RoutingContext {
            deployment_id: &candidate_ids[1],
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
    ];

    let iterations = 5_000;
    let mut heavy_count = 0;

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        if *selected == "heavy" {
            heavy_count += 1;
        }
    }

    let heavy_pct = heavy_count as f64 / iterations as f64;
    // Expected: heavy = 1000/1001 ~ 99.9%
    assert!(
        heavy_pct > 0.98,
        "heavy deployment should get >98%, got {:.1}%",
        heavy_pct * 100.0
    );
}

#[test]
fn test_weighted_random_equal_weights_uniform() {
    let candidate_ids: Vec<String> = (0..4).map(|i| format!("eq-{}", i)).collect();
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    let iterations = 10_000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let selected = weighted_random_from_context(&contexts).unwrap();
        *counts.entry(selected.clone()).or_default() += 1;
    }

    // Each should get ~25% +/- 5%
    for (id, count) in &counts {
        let pct = *count as f64 / iterations as f64;
        assert!(
            (pct - 0.25).abs() < 0.05,
            "{} expected ~25%, got {:.1}%",
            id,
            pct * 100.0
        );
    }
}

#[test]
fn test_weighted_random_u32_max_weight() {
    // Test that very large weights don't overflow
    let candidate_ids = ["big".to_string(), "small".to_string()];
    let contexts: Vec<RoutingContext<'_>> = vec![
        RoutingContext {
            deployment_id: &candidate_ids[0],
            weight: u32::MAX / 2,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
        RoutingContext {
            deployment_id: &candidate_ids[1],
            weight: 1,
            priority: 0,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        },
    ];

    // Should not panic due to overflow
    for _ in 0..100 {
        let selected = weighted_random_from_context(&contexts);
        assert!(selected.is_some());
    }
}
