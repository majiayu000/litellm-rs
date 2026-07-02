use super::*;

// ====================================================================================
// 4. EMA latency calculation edge cases
// ====================================================================================

#[tokio::test]
async fn test_ema_latency_first_measurement() {
    let d = create_test_deployment("ema-1", "gpt-4").await;

    // First measurement: avg should equal the measurement itself
    d.record_success(100, 5000);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), 5000);
}

#[tokio::test]
async fn test_ema_latency_converges() {
    let d = create_test_deployment("ema-2", "gpt-4").await;

    // Seed with initial value
    d.record_success(100, 1000);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), 1000);

    // Apply same value many times: should converge to that value
    for _ in 0..100 {
        d.record_success(100, 500);
    }

    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // After many iterations of recording 500, the EMA should converge near 500
    assert!(
        (499..=510).contains(&avg),
        "EMA should converge to ~500, got {}",
        avg
    );
}

#[tokio::test]
async fn test_ema_latency_zero_measurement() {
    let d = create_test_deployment("ema-3", "gpt-4").await;

    // First measurement is 1000
    d.record_success(100, 1000);

    // Record zero latency: EMA = (0 + 4 * 1000) / 5 = 800
    d.record_success(100, 0);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    assert_eq!(avg, 800, "EMA after zero: expected 800, got {}", avg);
}

#[tokio::test]
async fn test_ema_latency_large_values() {
    let d = create_test_deployment("ema-4", "gpt-4").await;

    // Use large but not overflowing values
    // u64::MAX / 5 is safe for the EMA formula: (new + 4 * old) / 5
    let large_val = u64::MAX / 10;
    d.record_success(100, large_val);
    assert_eq!(d.state.avg_latency_us.load(Ordering::Relaxed), large_val);

    // Next measurement should compute without overflow
    d.record_success(100, large_val);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (large_val + 4 * large_val) / 5 = large_val
    assert_eq!(
        avg, large_val,
        "EMA with large equal values should stay stable"
    );
}

#[tokio::test]
async fn test_ema_latency_spike_dampened() {
    let d = create_test_deployment("ema-5", "gpt-4").await;

    // Establish baseline of 1000
    d.record_success(100, 1000);

    // Record a huge spike
    d.record_success(100, 100_000);
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (100_000 + 4 * 1000) / 5 = 20_800
    assert_eq!(avg, 20_800, "spike should be dampened by EMA, got {}", avg);

    // Record normal value again
    d.record_success(100, 1000);
    let avg2 = d.state.avg_latency_us.load(Ordering::Relaxed);
    // EMA: (1000 + 4 * 20_800) / 5 = 16_840
    assert_eq!(avg2, 16_840, "should continue dampening, got {}", avg2);
}

#[tokio::test]
async fn test_ema_concurrent_updates() {
    let d = Arc::new(create_test_deployment("ema-c", "gpt-4").await);

    // Seed initial
    d.record_success(100, 1000);

    let mut handles = Vec::new();
    for _ in 0..10 {
        let d_clone = d.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                d_clone.record_success(10, 1000);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total = d.state.total_requests.load(Ordering::Relaxed);
    // 1 initial + 10 * 100 = 1001
    assert_eq!(total, 1001, "total_requests should be 1001, got {}", total);

    // The EMA should be near 1000 since all measurements were 1000
    // Note: due to non-atomic read-modify-write in EMA, the exact value
    // may have minor drift under concurrency. The important thing is no panic.
    let avg = d.state.avg_latency_us.load(Ordering::Relaxed);
    assert!(
        avg > 500 && avg < 2000,
        "EMA should be near 1000 despite concurrency, got {}",
        avg
    );
}
