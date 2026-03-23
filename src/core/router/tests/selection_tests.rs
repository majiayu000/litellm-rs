//! Unit tests for routing selection logic
//!
//! Tests edge cases and deterministic behavior for SimpleShuffle,
//! RoundRobin, and LatencyBased strategies via `select_deployment`.

use super::router_tests::create_test_deployment;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::router::deployment::HealthStatus;
use crate::core::router::unified::Router;
use std::collections::HashMap;
use std::sync::atomic::Ordering::Relaxed;

// ==================== SimpleShuffle ====================

#[tokio::test]
async fn test_simple_shuffle_single_deployment_always_selected() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let d = create_test_deployment("only-one", "gpt-4").await;
    d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    router.add_deployment(d);

    for _ in 0..50 {
        let id = router.select_deployment("gpt-4").unwrap();
        assert_eq!(id, "only-one");
        router.release_deployment(&id);
    }
}

#[tokio::test]
async fn test_simple_shuffle_respects_weight() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d_heavy = create_test_deployment("heavy", "gpt-4").await;
    let mut d_light = create_test_deployment("light", "gpt-4").await;
    d_heavy
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);
    d_light
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);

    d_heavy.config.weight = 90;
    d_light.config.weight = 10;

    router.add_deployment(d_heavy);
    router.add_deployment(d_light);

    let mut counts: HashMap<String, u32> = HashMap::new();
    for _ in 0..1000 {
        let id = router.select_deployment("gpt-4").unwrap();
        *counts.entry(id.clone()).or_default() += 1;
        router.release_deployment(&id);
    }

    let heavy_count = *counts.get("heavy").unwrap_or(&0);
    // With 90/10 weight split over 1000 trials, heavy should dominate.
    assert!(
        heavy_count > 700,
        "Expected weighted bias toward 'heavy', got {} out of 1000",
        heavy_count
    );
}

#[tokio::test]
async fn test_simple_shuffle_skips_unhealthy() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("healthy", "gpt-4").await;
    let d2 = create_test_deployment("sick", "gpt-4").await;
    d1.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    d2.state
        .health
        .store(HealthStatus::Unhealthy as u8, Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);

    for _ in 0..50 {
        let id = router.select_deployment("gpt-4").unwrap();
        assert_eq!(id, "healthy", "unhealthy deployment must not be selected");
        router.release_deployment(&id);
    }
}

#[tokio::test]
async fn test_simple_shuffle_increments_active_requests() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::SimpleShuffle,
        ..Default::default()
    };
    let router = Router::new(config);

    let d = create_test_deployment("d1", "gpt-4").await;
    d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    router.add_deployment(d);

    let id = router.select_deployment("gpt-4").unwrap();
    let active = router
        .get_deployment(&id)
        .unwrap()
        .state
        .active_requests
        .load(Relaxed);
    assert_eq!(
        active, 1,
        "select_deployment should increment active_requests"
    );

    router.release_deployment(&id);
    let after = router
        .get_deployment(&id)
        .unwrap()
        .state
        .active_requests
        .load(Relaxed);
    assert_eq!(
        after, 0,
        "release_deployment should decrement active_requests"
    );
}

// ==================== RoundRobin ====================

#[tokio::test]
async fn test_round_robin_cycles_deterministically() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("rr-a", "gpt-4").await;
    let d2 = create_test_deployment("rr-b", "gpt-4").await;
    d1.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    d2.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    router.add_deployment(d1);
    router.add_deployment(d2);

    // Collect 6 selections; round-robin should alternate between 2 deployments
    let mut ids = Vec::new();
    for _ in 0..6 {
        let id = router.select_deployment("gpt-4").unwrap();
        ids.push(id.clone());
        router.release_deployment(&id);
    }

    // Each consecutive pair should differ
    for pair in ids.windows(2) {
        assert_ne!(
            pair[0], pair[1],
            "consecutive round-robin picks must differ"
        );
    }

    // The cycle should repeat: id[0] == id[2] == id[4]
    assert_eq!(ids[0], ids[2]);
    assert_eq!(ids[2], ids[4]);
    assert_eq!(ids[1], ids[3]);
    assert_eq!(ids[3], ids[5]);
}

#[tokio::test]
async fn test_round_robin_single_deployment() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d = create_test_deployment("solo", "gpt-4").await;
    d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    router.add_deployment(d);

    for _ in 0..10 {
        let id = router.select_deployment("gpt-4").unwrap();
        assert_eq!(id, "solo");
        router.release_deployment(&id);
    }
}

#[tokio::test]
async fn test_round_robin_independent_per_model() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("gpt4-a", "gpt-4").await;
    let d2 = create_test_deployment("gpt4-b", "gpt-4").await;
    let d3 = create_test_deployment("claude-a", "claude").await;
    let d4 = create_test_deployment("claude-b", "claude").await;

    for d in [&d1, &d2, &d3, &d4] {
        d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    }
    router.add_deployment(d1);
    router.add_deployment(d2);
    router.add_deployment(d3);
    router.add_deployment(d4);

    // Advance gpt-4 counter by 3
    for _ in 0..3 {
        let id = router.select_deployment("gpt-4").unwrap();
        router.release_deployment(&id);
    }

    // Claude counter should still be at 0 — first claude pick is independent
    let claude_first = router.select_deployment("claude").unwrap();
    let claude_second = router.select_deployment("claude").unwrap();
    router.release_deployment(&claude_first);
    router.release_deployment(&claude_second);

    assert_ne!(
        claude_first, claude_second,
        "round-robin counters must be independent per model"
    );
}

#[tokio::test]
async fn test_round_robin_skips_cooldown() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::RoundRobin,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("ok", "gpt-4").await;
    let d2 = create_test_deployment("cooled", "gpt-4").await;

    d1.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    d2.state.health.store(HealthStatus::Healthy as u8, Relaxed);

    // Put d2 into cooldown
    d2.enter_cooldown(3600);

    router.add_deployment(d1);
    router.add_deployment(d2);

    // All selections must go to "ok"
    for _ in 0..10 {
        let id = router.select_deployment("gpt-4").unwrap();
        assert_eq!(id, "ok", "deployment in cooldown must be skipped");
        router.release_deployment(&id);
    }
}

// ==================== LatencyBased ====================

#[tokio::test]
async fn test_latency_based_picks_lowest() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let d1 = create_test_deployment("slow", "gpt-4").await;
    let d2 = create_test_deployment("fast", "gpt-4").await;
    let d3 = create_test_deployment("medium", "gpt-4").await;

    for d in [&d1, &d2, &d3] {
        d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    }

    d1.state.avg_latency_us.store(200_000, Relaxed);
    d2.state.avg_latency_us.store(10_000, Relaxed);
    d3.state.avg_latency_us.store(100_000, Relaxed);

    router.add_deployment(d1);
    router.add_deployment(d2);
    router.add_deployment(d3);

    let id = router.select_deployment("gpt-4").unwrap();
    assert_eq!(id, "fast", "should pick deployment with lowest latency");
    router.release_deployment(&id);
}

#[tokio::test]
async fn test_latency_based_zero_latency_uses_average() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let d_known = create_test_deployment("known", "gpt-4").await;
    let d_new = create_test_deployment("new", "gpt-4").await;

    d_known
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);
    d_new
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);

    // d_known has measured latency; d_new has 0 (no data yet)
    d_known.state.avg_latency_us.store(50_000, Relaxed);
    // d_new defaults to 0

    router.add_deployment(d_known);
    router.add_deployment(d_new);

    // With only one non-zero latency, average = 50_000.
    // d_new gets avg (50_000), d_known has 50_000, so either could be picked.
    // The key assertion is that it does NOT error out.
    let id = router.select_deployment("gpt-4").unwrap();
    router.release_deployment(&id);
}

#[tokio::test]
async fn test_latency_based_single_deployment() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let d = create_test_deployment("only", "gpt-4").await;
    d.state.health.store(HealthStatus::Healthy as u8, Relaxed);
    d.state.avg_latency_us.store(42_000, Relaxed);
    router.add_deployment(d);

    let id = router.select_deployment("gpt-4").unwrap();
    assert_eq!(id, "only");
    router.release_deployment(&id);
}

#[tokio::test]
async fn test_latency_based_skips_rate_limited() {
    let config = RouterConfig {
        routing_strategy: RoutingStrategy::LatencyBased,
        ..Default::default()
    };
    let router = Router::new(config);

    let mut d_fast = create_test_deployment("fast-limited", "gpt-4").await;
    let d_slow = create_test_deployment("slow-ok", "gpt-4").await;

    d_fast
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);
    d_slow
        .state
        .health
        .store(HealthStatus::Healthy as u8, Relaxed);

    d_fast.state.avg_latency_us.store(1_000, Relaxed);
    d_slow.state.avg_latency_us.store(100_000, Relaxed);

    // Rate-limit the fast one
    d_fast.config.rpm_limit = Some(100);
    d_fast.state.rpm_current.store(100, Relaxed);

    router.add_deployment(d_fast);
    router.add_deployment(d_slow);

    let id = router.select_deployment("gpt-4").unwrap();
    assert_eq!(
        id, "slow-ok",
        "rate-limited deployment must be excluded even if it has lowest latency"
    );
    router.release_deployment(&id);
}
