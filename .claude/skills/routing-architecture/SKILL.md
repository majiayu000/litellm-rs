---
name: routing-architecture
description: LiteLLM-RS Routing Architecture. Covers 7 routing strategies over immutable routing snapshots (ArcSwap) with atomic/DashMap state, health-aware deployment selection with cooldown circuit breaker, model-keyed fallback chains, and load balancing. Use when selecting or tuning a routing strategy, or configuring failover, health checks, and load balancing.
---

# Routing Architecture Guide

## Overview

The router (`src/core/router/`) selects among **deployments** — concrete provider+model
pairs registered via `Router::add_deployment` / `Router::set_model_list` — using one of
7 strategies. There is no per-strategy router struct, no `Router` trait, and no
`create_router` factory: strategy dispatch is a `match` on the `RoutingStrategy` enum
calling free functions in `src/core/router/strategy_impl.rs`.

### Key Design Principles

- **Snapshot isolation**: deployments, the model index, and aliases live in an immutable
  `RoutingSnapshot`, published through `ArcSwap` (`src/core/router/unified.rs:61,308`).
  Readers load one generation lock-free; writers clone-modify-store under a
  `parking_lot::Mutex` that only serializes writers (`unified.rs:312,379-398`).
- **Atomic deployment state**: per-deployment runtime state (`DeploymentState`) is plain
  atomics with `Relaxed` ordering (`deployment.rs:210-252`); RoundRobin uses a
  `DashMap<String, AtomicUsize>` of per-model counters (`unified.rs:321`).
- **Health-aware**: candidates are filtered by cooldown, health status, parallel limits,
  and RPM/TPM limits before a strategy picks among them.
- **Fallback chains**: the built-in execution path tries the model-keyed `General`
  fallback list after retries are exhausted. Typed context-window, content-policy, and
  rate-limit lists require explicit caller lookup/wiring today.

---

## Selection Flow

Entry point `Router::select_deployment_lease` (`selection.rs:95`) delegates to
`select_deployment_matching` (`selection.rs:226`). The real flow:

1. Load the current snapshot; resolve model aliases via `resolve_model_name`
   (max `MAX_ALIAS_HOPS = 16` hops, `unified.rs:26`).
2. Look up the resolved model in `snapshot.model_index` to get candidate `DeploymentId`s.
3. One filter pass builds `Vec<RoutingContext>` (`strategy_impl.rs:17`), skipping
   deployments that are in cooldown (`is_in_cooldown`), unhealthy (`is_healthy`), at their
   `max_parallel_requests` limit, or at their `rpm_limit` / `tpm_limit`
   (`selection.rs:287-344`). Each context copies `weight`, `priority`,
   `active_requests`, `tpm_current/tpm_limit`, `rpm_current/rpm_limit`, `avg_latency_us`.
4. `Router::select_from_routing_contexts` dispatches on the configured strategy
   (`selection.rs:195-224`):

```rust
match strategy {
    RoutingStrategy::SimpleShuffle => strategy_impl::weighted_random_from_context(routing_contexts),
    RoutingStrategy::LeastBusy     => strategy_impl::least_busy_from_context(routing_contexts),
    RoutingStrategy::UsageBased    => strategy_impl::lowest_usage_from_context(routing_contexts),
    RoutingStrategy::LatencyBased  => strategy_impl::lowest_latency_from_context(routing_contexts),
    RoutingStrategy::PriorityBased => strategy_impl::lowest_priority_from_context(routing_contexts),
    RoutingStrategy::RateLimitAware => strategy_impl::rate_limit_aware_from_context(routing_contexts),
    RoutingStrategy::RoundRobin    => strategy_impl::round_robin_from_context(
        model_name, routing_contexts, round_robin_counters),
}
```

5. The winner is reserved by `try_reserve_deployment` — an atomic increment of
   `active_requests` (CAS loop when `max_parallel_requests` is set). If another caller
   wins the last slot, that candidate is removed from the contexts and selection retries
   (`selection.rs:378-415`).
6. Returns a `DeploymentLease`; dropping it decrements `active_requests` (RAII release,
   `selection.rs:64-70`). The deprecated ID-returning `select_deployment` still exists
   but converts the lease to an ID without release-on-drop.

## Routing Strategies

Enum `RoutingStrategy` (`src/core/router/config.rs:22-39`) serializes as `snake_case`
(`simple_shuffle`, `round_robin`, `least_busy`, `latency_based`, `priority_based`,
`usage_based`, `rate_limit_aware`); `PriorityBased` also accepts the serde alias
`"cost_based"`.

### 1. SimpleShuffle (runtime default)

Weighted random selection: draws a point in `0..total_weight` and walks candidates until
the cumulative weight covers it; uniform random when total weight is 0
(`weighted_random_from_context`, `strategy_impl.rs:56-85`). Weights come from
`DeploymentConfig.weight` (default 1).

**Use when**: general traffic where deployments have different capacity.

### 2. RoundRobin (gateway YAML default)

Per-model counter in `round_robin_counters: DashMap<String, AtomicUsize>` cycles through
candidate order (`round_robin_from_context`, `strategy_impl.rs:247-274`). Note the
defaults diverge: `GatewayRouterConfig` defaults to `round_robin`
(`src/config/models/router.rs:48-50`) while runtime `RouterConfig` defaults to `SimpleShuffle`.

**Use when**: predictable distribution needed, debugging provider issues.

### 3. LeastBusy

Single pass for the fewest `active_requests`; ties are broken randomly with reservoir
sampling so equal-load deployments share traffic (`least_busy_from_context`,
`strategy_impl.rs:88-116`).

**Use when**: high concurrency, need to prevent deployment overload.

### 4. LatencyBased

Lowest `avg_latency_us` wins. Deployments reporting 0 latency (no success yet) inherit
the average of non-zero latencies in the pool, so new deployments neither always win nor
starve (`lowest_latency_from_context`, `strategy_impl.rs:145-182`). Latency is recorded
per request via `record_success` into `DeploymentState.avg_latency_us`.

**Use when**: response time is critical, deployments have varying latencies.

### 5. PriorityBased

Lowest `priority` value wins (lower = higher priority; `u32`, default 0). This is tier
ordering, not cost — despite the legacy `"cost_based"` serde alias
(`lowest_priority_from_context`, `strategy_impl.rs:185-203`).

**Use when**: primary/backup tiering, e.g. production vs backup deployments
(see `gateway.yaml.example` provider `priority`).

### 6. UsageBased

Lowest TPM usage percentage wins: `(tpm_current * 100) / tpm_limit`; deployments with no
limit count as 0% usage (`lowest_usage_from_context`, `strategy_impl.rs:119-142`).

**Use when**: spreading load relative to token budgets, avoiding TPM exhaustion.
Requires `tpm` to be configured on deployments to be meaningful.

### 7. RateLimitAware

Picks the deployment furthest from its rate limits: score is the minimum of remaining
TPM fraction and remaining RPM fraction; unlimited axes score 1.0
(`rate_limit_aware_from_context`, `strategy_impl.rs:206-241`).

**Use when**: high request volume against deployments with strict TPM/RPM limits.

---

## References

- [reference/health-and-fallbacks.md](reference/health-and-fallbacks.md) — health states, cooldown/circuit-breaker mechanics, probe tasks, and the FallbackConfig execution flow
- [reference/router-configuration.md](reference/router-configuration.md) — real `router:` YAML keys, gateway-to-runtime config mapping, and per-provider routing knobs
- [reference/performance-and-practices.md](reference/performance-and-practices.md) — verified complexity table and routing best practices
