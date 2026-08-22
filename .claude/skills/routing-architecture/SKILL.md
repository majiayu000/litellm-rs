---
name: routing-architecture
description: LiteLLM-RS Routing Architecture. Covers 7 routing strategies, lock-free design with DashMap, health-aware selection, fallback chains, and load balancing. Use when selecting or tuning a routing strategy, or configuring failover, health checks, and load balancing.
---

# Routing Architecture Guide

## Overview

LiteLLM-RS implements a high-performance, lock-free routing system with 7 strategies for intelligent provider selection across 66+ LLM providers.

### Key Design Principles

- **Lock-free**: Uses DashMap for concurrent access without mutex locks
- **Health-aware**: Continuous provider health monitoring
- **Configurable**: Multiple strategies with runtime selection
- **Fallback chains**: Automatic failover to backup providers

---

## Routing Strategies

### 1. SimpleShuffle (Default)

Random selection from healthy providers. Best for even load distribution.

```rust
pub struct SimpleShuffleRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    health_tracker: Arc<HealthTracker>,
}

impl Router for SimpleShuffleRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = rand::thread_rng().gen_range(0..healthy.len());
        Some(healthy[idx].clone())
    }
}
```

**Use when**: Equal provider capabilities, no preference for any provider.

### 2. RoundRobin

Sequential rotation through providers. Ensures even distribution.

```rust
pub struct RoundRobinRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    current_index: AtomicUsize,
    health_tracker: Arc<HealthTracker>,
}

impl Router for RoundRobinRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .collect();

        if healthy.is_empty() {
            return None;
        }

        let idx = self.current_index.fetch_add(1, Ordering::SeqCst) % healthy.len();
        Some(healthy[idx].clone())
    }
}
```

**Use when**: Predictable distribution needed, debugging provider issues.

### 3. LeastBusy

Routes to provider with fewest active requests.

```rust
pub struct LeastBusyRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    active_requests: DashMap<&'static str, AtomicU64>,
    health_tracker: Arc<HealthTracker>,
}

impl Router for LeastBusyRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .collect();

        healthy
            .into_iter()
            .min_by_key(|p| {
                self.active_requests
                    .get(p.name())
                    .map(|v| v.load(Ordering::SeqCst))
                    .unwrap_or(0)
            })
            .cloned()
    }

    fn on_request_start(&self, provider: &'static str) {
        self.active_requests
            .entry(provider)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::SeqCst);
    }

    fn on_request_end(&self, provider: &'static str) {
        if let Some(counter) = self.active_requests.get(provider) {
            counter.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
```

**Use when**: High concurrency, need to prevent provider overload.

### 4. LatencyBased

Routes to provider with lowest average latency.

```rust
pub struct LatencyBasedRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    latency_tracker: DashMap<&'static str, LatencyStats>,
    health_tracker: Arc<HealthTracker>,
}

#[derive(Default)]
struct LatencyStats {
    total_latency_ms: AtomicU64,
    request_count: AtomicU64,
}

impl LatencyStats {
    fn average_latency(&self) -> f64 {
        let total = self.total_latency_ms.load(Ordering::SeqCst) as f64;
        let count = self.request_count.load(Ordering::SeqCst) as f64;
        if count > 0.0 { total / count } else { f64::MAX }
    }
}

impl Router for LatencyBasedRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .collect();

        healthy
            .into_iter()
            .min_by(|a, b| {
                let lat_a = self.latency_tracker
                    .get(a.name())
                    .map(|s| s.average_latency())
                    .unwrap_or(f64::MAX);
                let lat_b = self.latency_tracker
                    .get(b.name())
                    .map(|s| s.average_latency())
                    .unwrap_or(f64::MAX);
                lat_a.partial_cmp(&lat_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }

    fn record_latency(&self, provider: &'static str, latency_ms: u64) {
        self.latency_tracker
            .entry(provider)
            .or_default()
            .total_latency_ms
            .fetch_add(latency_ms, Ordering::SeqCst);
        self.latency_tracker
            .get(provider)
            .unwrap()
            .request_count
            .fetch_add(1, Ordering::SeqCst);
    }
}
```

**Use when**: Response time is critical, providers have varying latencies.

### 5. PriorityBased

Routes to lowest-cost provider for the requested model.

```rust
pub struct PriorityBasedRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    pricing_db: Arc<PricingDatabase>,
    health_tracker: Arc<HealthTracker>,
}

impl Router for PriorityBasedRouter {
    async fn select_provider(&self, request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .filter(|p| p.supports_model(&request.model))
            .collect();

        healthy
            .into_iter()
            .min_by(|a, b| {
                let cost_a = self.pricing_db
                    .get_cost(a.name(), &request.model)
                    .unwrap_or(f64::MAX);
                let cost_b = self.pricing_db
                    .get_cost(b.name(), &request.model)
                    .unwrap_or(f64::MAX);
                cost_a.partial_cmp(&cost_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned()
    }
}
```

**Use when**: Cost optimization is primary goal, multiple providers offer same model.

### 6. UsageBased

Routes based on token/request usage quotas.

```rust
pub struct UsageBasedRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    usage_tracker: DashMap<&'static str, UsageStats>,
    quotas: DashMap<&'static str, Quota>,
    health_tracker: Arc<HealthTracker>,
}

struct UsageStats {
    requests_today: AtomicU64,
    tokens_today: AtomicU64,
    last_reset: AtomicU64,
}

struct Quota {
    max_requests_per_day: u64,
    max_tokens_per_day: u64,
}

impl Router for UsageBasedRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        self.reset_daily_usage_if_needed();

        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .filter(|p| !self.is_quota_exceeded(p.name()))
            .collect();

        // Prefer providers with most remaining quota
        healthy
            .into_iter()
            .max_by_key(|p| self.remaining_quota_percentage(p.name()))
            .cloned()
    }

    fn is_quota_exceeded(&self, provider: &'static str) -> bool {
        let usage = match self.usage_tracker.get(provider) {
            Some(u) => u,
            None => return false,
        };
        let quota = match self.quotas.get(provider) {
            Some(q) => q,
            None => return false,
        };

        usage.requests_today.load(Ordering::SeqCst) >= quota.max_requests_per_day
            || usage.tokens_today.load(Ordering::SeqCst) >= quota.max_tokens_per_day
    }
}
```

**Use when**: Managing quotas across providers, cost control.

### 7. RateLimitAware

Routes avoiding rate-limited providers.

```rust
pub struct RateLimitAwareRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    rate_limit_tracker: DashMap<&'static str, RateLimitState>,
    health_tracker: Arc<HealthTracker>,
}

struct RateLimitState {
    is_limited: AtomicBool,
    retry_after: AtomicU64,
    limited_at: AtomicU64,
}

impl Router for RateLimitAwareRouter {
    async fn select_provider(&self, _request: &ChatRequest) -> Option<Arc<dyn LLMProvider>> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let healthy: Vec<_> = self.providers
            .iter()
            .filter(|p| self.health_tracker.is_healthy(p.name()))
            .filter(|p| !self.is_rate_limited(p.name(), now))
            .collect();

        if healthy.is_empty() {
            // All providers rate limited, return one with shortest wait
            return self.providers
                .iter()
                .min_by_key(|p| self.time_until_available(p.name(), now))
                .cloned();
        }

        // Random selection from non-limited providers
        let idx = rand::thread_rng().gen_range(0..healthy.len());
        Some(healthy[idx].clone())
    }

    fn mark_rate_limited(&self, provider: &'static str, retry_after: u64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.rate_limit_tracker
            .entry(provider)
            .or_insert_with(|| RateLimitState {
                is_limited: AtomicBool::new(true),
                retry_after: AtomicU64::new(retry_after),
                limited_at: AtomicU64::new(now),
            });

        if let Some(state) = self.rate_limit_tracker.get(provider) {
            state.is_limited.store(true, Ordering::SeqCst);
            state.retry_after.store(retry_after, Ordering::SeqCst);
            state.limited_at.store(now, Ordering::SeqCst);
        }
    }

    fn is_rate_limited(&self, provider: &'static str, now: u64) -> bool {
        self.rate_limit_tracker
            .get(provider)
            .map(|state| {
                if !state.is_limited.load(Ordering::SeqCst) {
                    return false;
                }
                let limited_at = state.limited_at.load(Ordering::SeqCst);
                let retry_after = state.retry_after.load(Ordering::SeqCst);
                now < limited_at + retry_after
            })
            .unwrap_or(false)
    }
}
```

**Use when**: High request volume, providers have strict rate limits.

---

## References

- [reference/health-and-fallbacks.md](reference/health-and-fallbacks.md) — HealthTracker implementation and fallback chain execution flow
- [reference/router-configuration.md](reference/router-configuration.md) — YAML routing configuration keys and router factory wiring
- [reference/performance-and-practices.md](reference/performance-and-practices.md) — per-strategy performance table and routing best practices
