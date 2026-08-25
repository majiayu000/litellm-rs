## Cache Metrics

```rust
#[derive(Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
    pub l1_hits: AtomicU64,
    pub l2_hits: AtomicU64,
    pub l3_hits: AtomicU64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        if hits + misses == 0.0 {
            0.0
        } else {
            hits / (hits + misses)
        }
    }

    pub fn report_metrics(&self, metrics: &MetricsReporter) {
        metrics.gauge("cache_hit_rate", self.hit_rate());
        metrics.counter("cache_hits_total", self.hits.load(Ordering::Relaxed));
        metrics.counter("cache_misses_total", self.misses.load(Ordering::Relaxed));
        metrics.counter("cache_evictions_total", self.evictions.load(Ordering::Relaxed));
    }
}
```
