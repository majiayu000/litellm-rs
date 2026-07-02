use super::*;

// ====================================================================================
// round_robin Tests
// ====================================================================================

#[test]
fn test_round_robin_single_candidate() {
    let counters: DashMap<String, AtomicUsize> = DashMap::new();
    let candidate_ids = ["d1".to_string()];
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 1,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    let selected = round_robin_from_context("gpt-4", &contexts, &counters).unwrap();
    assert_eq!(selected, "d1");
}

#[test]
fn test_round_robin_cycles_through_candidates() {
    let counters: DashMap<String, AtomicUsize> = DashMap::new();
    let candidate_ids = ["d1".to_string(), "d2".to_string(), "d3".to_string()];
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 1,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    // First cycle
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d1"
    );
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d2"
    );
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d3"
    );

    // Second cycle
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d1"
    );
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d2"
    );
}

#[test]
fn test_round_robin_separate_counters_per_model() {
    let counters: DashMap<String, AtomicUsize> = DashMap::new();
    let candidate_ids = ["d1".to_string(), "d2".to_string()];
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 1,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    // gpt-4 model
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d1"
    );
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d2"
    );

    // claude model has its own counter
    assert_eq!(
        round_robin_from_context("claude-3", &contexts, &counters).unwrap(),
        "d1"
    );
    assert_eq!(
        round_robin_from_context("claude-3", &contexts, &counters).unwrap(),
        "d2"
    );

    // gpt-4 continues from where it left off
    assert_eq!(
        round_robin_from_context("gpt-4", &contexts, &counters).unwrap(),
        "d1"
    );
}

#[test]
fn test_round_robin_wraps_around() {
    let counters: DashMap<String, AtomicUsize> = DashMap::new();
    let candidate_ids = ["d1".to_string(), "d2".to_string()];
    let contexts: Vec<RoutingContext<'_>> = candidate_ids
        .iter()
        .map(|id| RoutingContext {
            deployment_id: id,
            weight: 1,
            priority: 1,
            active_requests: 0,
            tpm_current: 0,
            tpm_limit: None,
            rpm_current: 0,
            rpm_limit: None,
            avg_latency_us: 0,
        })
        .collect();

    // Run many times and verify it keeps cycling
    for i in 0..100 {
        let selected = round_robin_from_context("gpt-4", &contexts, &counters).unwrap();
        if i % 2 == 0 {
            assert_eq!(selected, "d1");
        } else {
            assert_eq!(selected, "d2");
        }
    }
}

#[test]
fn test_round_robin_empty_candidates() {
    let counters: DashMap<String, AtomicUsize> = DashMap::new();
    let contexts: Vec<RoutingContext<'_>> = vec![];
    assert!(round_robin_from_context("gpt-4", &contexts, &counters).is_none());
}
