//! Bounded histogram for metrics collection

use std::collections::VecDeque;

/// Maximum number of samples to keep in histogram (prevents unbounded memory growth)
pub const HISTOGRAM_MAX_SAMPLES: usize = 1000;

/// Bounded histogram that maintains a rolling window of samples
#[derive(Debug, Clone)]
pub struct BoundedHistogram {
    /// Rolling window of duration samples
    samples: VecDeque<f64>,
    /// Maximum number of samples to retain
    max_samples: usize,
    /// Running sum for efficient mean calculation
    sum: f64,
    /// Total count of all samples ever recorded (for accurate counting)
    total_count: u64,
}

impl BoundedHistogram {
    /// Create a new bounded histogram with specified capacity
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
            sum: 0.0,
            total_count: 0,
        }
    }

    /// Record a new duration sample
    pub fn record(&mut self, value: f64) {
        self.total_count += 1;
        self.sum += value;

        // If at capacity, remove oldest sample from sum
        if self.samples.len() >= self.max_samples
            && let Some(oldest) = self.samples.pop_front()
        {
            self.sum -= oldest;
        }

        self.samples.push_back(value);
    }

    /// Get the mean of current samples
    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() {
            0.0
        } else {
            self.sum / self.samples.len() as f64
        }
    }

    /// Get percentile value (0-100)
    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let mut sorted: Vec<f64> = self.samples.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Use linear interpolation for more accurate percentiles
        let n = sorted.len();
        if n == 1 {
            return sorted[0];
        }

        // Calculate position using the standard percentile formula
        let pos = (p / 100.0) * (n - 1) as f64;
        let lower = pos.floor() as usize;
        let upper = pos.ceil() as usize;

        if lower == upper {
            sorted[lower]
        } else {
            // Linear interpolation between lower and upper
            let frac = pos - lower as f64;
            sorted[lower] * (1.0 - frac) + sorted[upper] * frac
        }
    }

    /// Get the total count of samples ever recorded
    pub fn count(&self) -> u64 {
        self.total_count
    }

    /// Get current number of samples in the window
    pub fn window_size(&self) -> usize {
        self.samples.len()
    }

    /// Get min value in current window
    pub fn min(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }

    /// Get max value in current window
    pub fn max(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    }
}

impl Default for BoundedHistogram {
    fn default() -> Self {
        Self::new(HISTOGRAM_MAX_SAMPLES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== Creation Tests ====================

    #[test]
    fn test_new_histogram() {
        let hist = BoundedHistogram::new(100);
        assert_eq!(hist.count(), 0);
        assert_eq!(hist.window_size(), 0);
        assert_eq!(hist.mean(), 0.0);
    }

    #[test]
    fn test_default_histogram() {
        let hist = BoundedHistogram::default();
        assert_eq!(hist.max_samples, HISTOGRAM_MAX_SAMPLES);
        assert_eq!(hist.count(), 0);
    }

    #[test]
    fn test_histogram_max_samples_constant() {
        assert_eq!(HISTOGRAM_MAX_SAMPLES, 1000);
    }

    // ==================== Record Tests ====================

    #[test]
    fn test_record_single() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(5.0);

        assert_eq!(hist.count(), 1);
        assert_eq!(hist.window_size(), 1);
        assert_eq!(hist.mean(), 5.0);
    }

    #[test]
    fn test_record_multiple() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(1.0);
        hist.record(2.0);
        hist.record(3.0);

        assert_eq!(hist.count(), 3);
        assert_eq!(hist.window_size(), 3);
        assert_eq!(hist.mean(), 2.0);
    }

    #[test]
    fn test_record_overflow() {
        let mut hist = BoundedHistogram::new(3);

        hist.record(1.0);
        hist.record(2.0);
        hist.record(3.0);
        assert_eq!(hist.window_size(), 3);
        assert_eq!(hist.count(), 3);

        // This should evict the oldest (1.0)
        hist.record(4.0);
        assert_eq!(hist.window_size(), 3);
        assert_eq!(hist.count(), 4);

        // Mean should be (2+3+4)/3 = 3.0
        assert_eq!(hist.mean(), 3.0);
    }

    #[test]
    fn test_record_rolling_window() {
        let mut hist = BoundedHistogram::new(2);

        hist.record(10.0);
        hist.record(20.0);
        assert_eq!(hist.mean(), 15.0);

        hist.record(30.0); // Evicts 10.0
        assert_eq!(hist.mean(), 25.0); // (20+30)/2

        hist.record(40.0); // Evicts 20.0
        assert_eq!(hist.mean(), 35.0); // (30+40)/2
    }

    // ==================== Mean Tests ====================

    #[test]
    fn test_mean_empty() {
        let hist = BoundedHistogram::new(10);
        assert_eq!(hist.mean(), 0.0);
    }

    #[test]
    fn test_mean_single_value() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(42.0);
        assert_eq!(hist.mean(), 42.0);
    }

    #[test]
    fn test_mean_multiple_values() {
        let mut hist = BoundedHistogram::new(10);
        for i in 1..=10 {
            hist.record(i as f64);
        }
        // Mean of 1..10 = 5.5
        assert_eq!(hist.mean(), 5.5);
    }

    #[test]
    fn test_mean_precision() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.1);
        hist.record(0.2);
        hist.record(0.3);

        let mean = hist.mean();
        assert!((mean - 0.2).abs() < 0.0001);
    }

    // ==================== Percentile Tests ====================

    #[test]
    fn test_percentile_empty() {
        let hist = BoundedHistogram::new(10);
        assert_eq!(hist.percentile(50.0), 0.0);
        assert_eq!(hist.percentile(99.0), 0.0);
    }

    #[test]
    fn test_percentile_single_value() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(100.0);

        assert_eq!(hist.percentile(0.0), 100.0);
        assert_eq!(hist.percentile(50.0), 100.0);
        assert_eq!(hist.percentile(100.0), 100.0);
    }

    #[test]
    fn test_percentile_two_values() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.0);
        hist.record(100.0);

        assert_eq!(hist.percentile(0.0), 0.0);
        assert_eq!(hist.percentile(50.0), 50.0);
        assert_eq!(hist.percentile(100.0), 100.0);
    }

    #[test]
    fn test_percentile_p50() {
        let mut hist = BoundedHistogram::new(100);
        for i in 1..=100 {
            hist.record(i as f64);
        }

        let p50 = hist.percentile(50.0);
        // For 100 values, p50 should be around 50.5
        assert!((49.0..=52.0).contains(&p50));
    }

    #[test]
    fn test_percentile_p90() {
        let mut hist = BoundedHistogram::new(100);
        for i in 1..=100 {
            hist.record(i as f64);
        }

        let p90 = hist.percentile(90.0);
        // For 100 values, p90 should be around 90
        assert!((89.0..=92.0).contains(&p90));
    }

    #[test]
    fn test_percentile_p99() {
        let mut hist = BoundedHistogram::new(100);
        for i in 1..=100 {
            hist.record(i as f64);
        }

        let p99 = hist.percentile(99.0);
        // For 100 values, p99 should be around 99
        assert!((98.0..=100.0).contains(&p99));
    }

    #[test]
    fn test_percentile_interpolation() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.0);
        hist.record(10.0);

        // Test interpolation at 25%
        let p25 = hist.percentile(25.0);
        assert_eq!(p25, 2.5);

        // Test interpolation at 75%
        let p75 = hist.percentile(75.0);
        assert_eq!(p75, 7.5);
    }

    // ==================== Count Tests ====================

    #[test]
    fn test_count_tracks_all_samples() {
        let mut hist = BoundedHistogram::new(3);

        for i in 1..=10 {
            hist.record(i as f64);
        }

        // Count should be 10 even though window only holds 3
        assert_eq!(hist.count(), 10);
        assert_eq!(hist.window_size(), 3);
    }

    #[test]
    fn test_window_size_bounded() {
        let mut hist = BoundedHistogram::new(5);

        for i in 1..=100 {
            hist.record(i as f64);
        }

        assert_eq!(hist.window_size(), 5);
        assert_eq!(hist.count(), 100);
    }

    // ==================== Min/Max Tests ====================

    #[test]
    fn test_min_empty() {
        let hist = BoundedHistogram::new(10);
        assert_eq!(hist.min(), 0.0);
    }

    #[test]
    fn test_max_empty() {
        let hist = BoundedHistogram::new(10);
        assert_eq!(hist.max(), 0.0);
    }

    #[test]
    fn test_min_single() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(42.0);
        assert_eq!(hist.min(), 42.0);
    }

    #[test]
    fn test_max_single() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(42.0);
        assert_eq!(hist.max(), 42.0);
    }

    #[test]
    fn test_min_multiple() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(5.0);
        hist.record(1.0);
        hist.record(10.0);
        hist.record(3.0);

        assert_eq!(hist.min(), 1.0);
    }

    #[test]
    fn test_max_multiple() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(5.0);
        hist.record(1.0);
        hist.record(10.0);
        hist.record(3.0);

        assert_eq!(hist.max(), 10.0);
    }

    #[test]
    fn test_min_max_after_eviction() {
        let mut hist = BoundedHistogram::new(3);

        hist.record(1.0); // Will be evicted
        hist.record(5.0);
        hist.record(10.0);
        hist.record(3.0); // Evicts 1.0

        assert_eq!(hist.min(), 3.0);
        assert_eq!(hist.max(), 10.0);
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_clone() {
        let mut hist1 = BoundedHistogram::new(10);
        hist1.record(1.0);
        hist1.record(2.0);
        hist1.record(3.0);

        let hist2 = hist1.clone();

        assert_eq!(hist1.count(), hist2.count());
        assert_eq!(hist1.mean(), hist2.mean());
        assert_eq!(hist1.window_size(), hist2.window_size());
    }

    #[test]
    fn test_clone_independence() {
        let mut hist1 = BoundedHistogram::new(10);
        hist1.record(1.0);

        let mut hist2 = hist1.clone();
        hist2.record(100.0);

        assert_eq!(hist1.count(), 1);
        assert_eq!(hist2.count(), 2);
        assert_eq!(hist1.mean(), 1.0);
        assert_eq!(hist2.mean(), 50.5);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_zero_values() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.0);
        hist.record(0.0);
        hist.record(0.0);

        assert_eq!(hist.mean(), 0.0);
        assert_eq!(hist.min(), 0.0);
        assert_eq!(hist.max(), 0.0);
    }

    #[test]
    fn test_negative_values() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(-5.0);
        hist.record(-10.0);
        hist.record(5.0);

        assert_eq!(hist.min(), -10.0);
        assert_eq!(hist.max(), 5.0);
        assert!((hist.mean() - (-10.0 / 3.0)).abs() < 0.0001);
    }

    #[test]
    fn test_very_small_values() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.0001);
        hist.record(0.0002);
        hist.record(0.0003);

        let mean = hist.mean();
        assert!((mean - 0.0002).abs() < 0.00001);
    }

    #[test]
    fn test_very_large_values() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(1e15);
        hist.record(2e15);
        hist.record(3e15);

        assert_eq!(hist.mean(), 2e15);
    }

    #[test]
    fn test_mixed_precision() {
        let mut hist = BoundedHistogram::new(10);
        hist.record(0.001);
        hist.record(1000.0);
        hist.record(0.5);

        let mean = hist.mean();
        let expected = (0.001 + 1000.0 + 0.5) / 3.0;
        assert!((mean - expected).abs() < 0.0001);
    }

    #[test]
    fn test_size_one_histogram() {
        let mut hist = BoundedHistogram::new(1);

        hist.record(10.0);
        assert_eq!(hist.mean(), 10.0);
        assert_eq!(hist.window_size(), 1);

        hist.record(20.0);
        assert_eq!(hist.mean(), 20.0);
        assert_eq!(hist.window_size(), 1);
        assert_eq!(hist.count(), 2);
    }

    // ==================== Stress Tests ====================

    #[test]
    fn test_many_records() {
        let mut hist = BoundedHistogram::new(100);

        for i in 1..=10000 {
            hist.record(i as f64);
        }

        assert_eq!(hist.count(), 10000);
        assert_eq!(hist.window_size(), 100);

        // Window should contain 9901..10000
        // Mean should be (9901+10000)/2 = 9950.5
        assert!((hist.mean() - 9950.5).abs() < 0.001);
    }

    #[test]
    fn test_alternating_values() {
        let mut hist = BoundedHistogram::new(100);

        for _ in 0..100 {
            hist.record(0.0);
            hist.record(100.0);
        }

        assert_eq!(hist.count(), 200);
        assert_eq!(hist.mean(), 50.0);
    }
}
