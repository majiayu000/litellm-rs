# LiteLLM vs LiteLLM-RS Routing Strategy Deep Comparison

This document provides an in-depth analysis of the routing strategies between the Python LiteLLM project and its Rust implementation (LiteLLM-RS).

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Routing Strategy Types](#routing-strategy-types)
3. [Load Balancing](#load-balancing)
4. [Fallback Mechanisms](#fallback-mechanisms)
5. [Model Routing](#model-routing)
6. [Routing Configuration](#routing-configuration)
7. [Implementation Differences](#implementation-differences)
8. [Feature Matrix](#feature-matrix)

---

## Executive Summary

| Aspect | Python LiteLLM | Rust LiteLLM-RS |
|--------|---------------|-----------------|
| **Architecture** | Plugin-based with external caching (Redis) | Atomic counters with synchronized rollover, in-memory |
| **Strategy Count** | 6 strategies + custom | 7 strategies built-in |
| **Concurrency Model** | asyncio + external state | DashMap + Atomic operations |
| **State Management** | Distributed (Redis/DualCache) | Local (Atomic counters) |
| **Performance Focus** | Flexibility | Raw throughput |

---

## 1. Routing Strategy Types

### 1.1 Strategy Comparison Table

| Strategy | Python (litellm) | Rust (litellm-rs) | Notes |
|----------|------------------|-------------------|-------|
| Simple Shuffle / Weighted Random | `simple-shuffle` | `SimpleShuffle` | Default in both |
| Least Busy | `least-busy` | `LeastBusy` | Active request tracking |
| Latency Based | `latency-based-routing` | `LatencyBased` | P50/average latency |
| Priority Based | `cost-based-routing` | `PriorityBased` | Priority-based routing (uses priority field) |
| Usage Based (TPM/RPM) | `usage-based-routing` | `UsageBased` | Rate limit optimization |
| Usage Based V2 | `usage-based-routing-v2` | N/A | Enhanced TPM/RPM tracking |
| Provider Budget | `provider-budget-routing` | N/A | $ budget limits |
| Rate Limit Aware | N/A | `RateLimitAware` | Proactive 429 avoidance |
| Round Robin | N/A | `RoundRobin` | Deterministic distribution |

### 1.2 Python Implementation Details

**File**: `/litellm/router_strategy/`

```python
# Python RoutingStrategy Enum
class RoutingStrategy(enum.Enum):
    LEAST_BUSY = "least-busy"
    LATENCY_BASED = "latency-based-routing"
    COST_BASED = "cost-based-routing"
    USAGE_BASED_ROUTING_V2 = "usage-based-routing-v2"
    USAGE_BASED_ROUTING = "usage-based-routing"
    PROVIDER_BUDGET_LIMITING = "provider-budget-routing"
```

**Key Characteristics**:
- Uses `CustomLogger` callbacks for state tracking
- External cache (Redis/DualCache) for distributed state
- Supports custom routing strategies via `CustomRoutingStrategyBase`
- State tracked via litellm callbacks (pre/post API call)

### 1.3 Rust Implementation Details

**File**: `/src/core/router/config.rs`

```rust
// Rust RoutingStrategy Enum
pub enum RoutingStrategy {
    SimpleShuffle,  // Default
    LeastBusy,
    UsageBased,
    LatencyBased,
    PriorityBased,
    RateLimitAware,
    RoundRobin,
}
```

**Key Characteristics**:
- Atomic counters with synchronized per-minute rollover
- State stored directly on `DeploymentState` struct
- Uses `DashMap` for concurrent access
- Counter updates use `Ordering::Relaxed`; rollover publication uses acquire/release ordering

---

## 2. Load Balancing

### 2.1 Weight Distribution

#### Python

```python
# Weight from litellm_params
litellm_params = {
    "model": "gpt-4",
    "weight": 2,  # Higher = more traffic
    "order": 1,   # Priority order
}
```

- Weights stored in `litellm_params`
- Simple shuffle uses weighted random selection
- Order parameter for priority-based selection

#### Rust

```rust
pub struct DeploymentConfig {
    pub weight: u32,       // Weight for weighted selection (default: 1)
    pub priority: u32,     // Lower = higher priority (default: 0)
}
```

- Weights stored in `DeploymentConfig`
- Weighted selection in `SimpleShuffle` strategy
- Priority ordering for group-based routing

### 2.2 Health Checking

#### Python Implementation

```python
# Health determined by cooldown state
def get_healthy_deployments(self, model):
    cooldown_deployments = self.get_cooldown_deployments()
    return [d for d in deployments if d['id'] not in cooldown_deployments]
```

- External cooldown list in cache
- Implicit health from failure tracking
- Distributed health state via Redis

#### Rust Implementation

```rust
pub enum HealthStatus {
    Unknown = 0,   // Newly created
    Healthy = 1,   // Passing checks
    Degraded = 2,  // Issues but functional
    Unhealthy = 3, // Failing checks
    Cooldown = 4,  // Temporarily disabled
}

impl Deployment {
    pub fn is_healthy(&self) -> bool {
        matches!(status, HealthStatus::Healthy | HealthStatus::Degraded)
    }
}
```

- Explicit 5-state health model
- Atomic state transitions
- Local state per deployment

### 2.3 Capacity Tracking

#### Python

```python
# TPM/RPM from litellm_params
class LiteLLM_Params:
    tpm: Optional[int] = None  # Tokens per minute
    rpm: Optional[int] = None  # Requests per minute
    max_parallel_requests: Optional[int] = None
```

- Global tracking via Redis cache keys
- Pattern: `global_router:{id}:{model}:tpm:{current_minute}`
- Async semaphores for parallel request limits

#### Rust

```rust
pub struct DeploymentConfig {
    pub tpm_limit: Option<u64>,
    pub rpm_limit: Option<u64>,
    pub max_parallel_requests: Option<u32>,
}

pub struct DeploymentState {
    pub tpm_current: AtomicU64,    // Current minute TPM
    pub rpm_current: AtomicU64,    // Current minute RPM
    pub active_requests: AtomicU32, // Active requests
}
```

- Local atomic counters per deployment with a shared rollover gate
- Readers and writers lazily roll expired windows; optional `Weak`-owned maintenance performs eager cleanup
- No external dependencies for rate tracking

---

## 3. Fallback Mechanisms

### 3.1 Fallback Types Comparison

| Fallback Type | Python | Rust | Trigger Condition |
|--------------|--------|------|-------------------|
| General | Yes | Yes | Any failure |
| Context Window | Yes | Yes | `ContextWindowExceededError` |
| Content Policy | Yes | Yes | `ContentPolicyViolationError` |
| Rate Limit | No | Yes | 429 errors |

### 3.2 Python Fallback Configuration

```python
# Router configuration
fallbacks = [
    {"gpt-4": ["claude-3-opus", "gpt-3.5-turbo"]},
    {"*": ["fallback-model"]}  # Wildcard fallback
]

context_window_fallbacks = [
    {"gpt-4-32k": ["gpt-4-turbo", "claude-3-opus"]}
]

content_policy_fallbacks = [
    {"gpt-4": ["claude-3-opus"]}
]
```

**Features**:
- Model-specific fallback chains
- Wildcard (`*`) catch-all fallbacks
- Separate fallback lists per error type
- `max_fallbacks` limit (default: 5)

### 3.3 Rust Fallback Configuration

```rust
pub enum FallbackType {
    General,
    ContextWindow,
    ContentPolicy,
    RateLimit,
}

pub struct FallbackConfig {
    general: HashMap<String, Vec<String>>,
    context_window: HashMap<String, Vec<String>>,
    content_policy: HashMap<String, Vec<String>>,
    rate_limit: HashMap<String, Vec<String>>,
}
```

**Features**:
- Type-safe fallback configuration
- Automatic fallback type inference from errors
- Chain: specific fallbacks -> general fallbacks
- `max_fallbacks` config parameter

### 3.4 Retry Logic Comparison

#### Python

```python
class RetryPolicy(BaseModel):
    BadRequestErrorRetries: Optional[int] = None
    AuthenticationErrorRetries: Optional[int] = None
    TimeoutErrorRetries: Optional[int] = None
    RateLimitErrorRetries: Optional[int] = None
    ContentPolicyViolationErrorRetries: Optional[int] = None
    InternalServerErrorRetries: Optional[int] = None
```

- Per-exception-type retry configuration
- `num_retries` global setting
- `retry_after` delay between retries

#### Rust

```rust
pub struct RouterConfig {
    pub num_retries: u32,          // Default: 3
    pub retry_after_secs: u64,     // Default: 0
    pub max_fallbacks: u32,        // Default: 5
}

// Exponential backoff
pub fn calculate_retry_delay(config: &RouterConfig, attempt: u32) -> Duration {
    let base = config.retry_after_secs.max(1);
    let delay = base * (2_u64.pow(attempt.saturating_sub(1)));
    Duration::from_secs(delay.min(30)) // Cap at 30 seconds
}
```

- Exponential backoff with 30s cap
- Unified retry count across error types
- Retryable errors: RateLimit, Timeout, ProviderUnavailable, Network

---

## 4. Cooldown / Failure Handling

### 4.1 Cooldown Triggers

#### Python

```python
# _is_cooldown_required function
def _is_cooldown_required(model_id, exception_status):
    if exception_status == 429:  # Rate limit
        return True
    elif exception_status == 401:  # Auth error
        return True
    elif exception_status == 408:  # Timeout
        return True
    elif exception_status == 404:  # Not found
        return True
    elif exception_status >= 400 and exception_status < 500:
        return False  # Other 4xx don't cooldown
    else:
        return True  # 5xx and other errors cooldown
```

#### Rust

```rust
pub enum CooldownReason {
    RateLimit,           // 429
    AuthError,           // 401
    NotFound,            // 404
    Timeout,             // 408
    ConsecutiveFailures, // Threshold exceeded
    HighFailureRate,     // >50% failure rate
    Manual,              // Manual cooldown
}
```

### 4.2 Cooldown Duration

| Parameter | Python Default | Rust Default |
|-----------|---------------|--------------|
| Cooldown Time | 5 seconds | 5 seconds |
| Allowed Fails | None (dynamic) | 3 |
| Failure Threshold | 50% | 50% |
| Min Requests | 5 | 10 |

### 4.3 Python Cooldown Logic

```python
# V2 Logic (Current)
def _should_cooldown_deployment(deployment, exception_status):
    # Immediate cooldown for specific errors
    if exception_status in [429, 401, 404, 408]:
        return True

    # Failure rate based cooldown
    failures = get_deployment_failures_for_current_minute(deployment)
    successes = get_deployment_successes_for_current_minute(deployment)
    total = failures + successes

    if total >= MIN_REQUESTS:
        failure_rate = failures / total
        if failure_rate > ALLOWED_FAILURE_RATE:
            return True

    return False
```

### 4.4 Rust Cooldown Logic

```rust
pub fn record_failure_with_reason(&self, deployment_id: &str, reason: CooldownReason) {
    if let Some(d) = self.deployments.get(deployment_id) {
        d.record_failure();

        let should_cooldown = match reason {
            // Immediate cooldown for these errors
            CooldownReason::RateLimit
            | CooldownReason::AuthError
            | CooldownReason::NotFound
            | CooldownReason::Timeout
            | CooldownReason::Manual => true,

            // Threshold-based cooldown
            CooldownReason::ConsecutiveFailures => {
                d.state.fails_this_minute.load(Relaxed) >= self.config.allowed_fails
            }

            // Rate-based cooldown
            CooldownReason::HighFailureRate => {
                let total = d.state.total_requests.load(Relaxed);
                let fails = d.state.fail_requests.load(Relaxed);
                total >= 10 && (fails * 100 / total) > 50
            }
        };

        if should_cooldown {
            d.enter_cooldown(self.config.cooldown_time_secs);
        }
    }
}
```

---

## 5. Model Routing

### 5.1 Model Aliasing

#### Python

```python
# Model group aliases
model_group_alias = {
    "gpt4": "gpt-4",
    "claude": "claude-3-opus",
    "best": ["gpt-4", "claude-3-opus"]  # List = fallback chain
}

def _get_model_from_alias(self, model):
    alias_config = self.model_group_alias.get(model)
    if isinstance(alias_config, str):
        return alias_config
    elif isinstance(alias_config, dict):
        return alias_config.get("model")
    return None
```

#### Rust

```rust
// Model aliases stored in DashMap
pub(crate) model_aliases: DashMap<String, String>,

pub fn add_model_alias(&self, alias: &str, model_name: &str) {
    self.model_aliases.insert(alias.to_string(), model_name.to_string());
}

pub fn resolve_model_name(&self, name: &str) -> String {
    self.model_aliases
        .get(name)
        .map(|v| v.clone())
        .unwrap_or_else(|| name.to_string())
}
```

### 5.2 Tag-Based Routing

#### Python

```python
# Tag filtering in deployment selection
def get_deployments_for_tag(healthy_deployments, request_tags, match_any=True):
    """
    - match_any=True: deployment has ANY of the request tags
    - match_any=False: deployment has ALL request tags
    """
    for deployment in healthy_deployments:
        deployment_tags = deployment.get("litellm_params", {}).get("tags", [])

        if match_any:
            if set(deployment_tags) & set(request_tags):
                yield deployment
        else:
            if set(request_tags).issubset(set(deployment_tags)):
                yield deployment
```

#### Rust

```rust
pub async fn select_provider_with_tags(
    &self,
    model: &str,
    tags: &[String],
    require_all_tags: bool,
    context: &RequestContext,
) -> Result<Provider> {
    let tagged_providers: Vec<String> = supporting_providers
        .into_iter()
        .filter(|name| {
            self.deployments.get(name).map(|info| {
                if require_all_tags {
                    info.has_all_tags(tags)
                } else {
                    info.has_any_tag(tags)
                }
            }).unwrap_or(false)
        })
        .collect();

    // ... select from tagged providers
}
```

### 5.3 Model Group Routing

#### Python

```python
# Pattern-based routing for wildcards
class PatternMatchRouter:
    def add_pattern(self, pattern: str, deployment: dict):
        # e.g., "openai/*" matches "openai/gpt-4", "openai/gpt-3.5-turbo"
        ...
```

#### Rust

```rust
pub async fn select_provider_by_group(
    &self,
    model: &str,
    group: &str,
    context: &RequestContext,
) -> Result<Provider> {
    let grouped_providers: Vec<(String, u32)> = supporting_providers
        .into_iter()
        .filter_map(|name| {
            self.deployments.get(&name).and_then(|info| {
                if info.model_group.as_deref() == Some(group) {
                    Some((name, info.priority))
                } else {
                    None
                }
            })
        })
        .collect();

    // Sort by priority
    grouped_providers.sort_by_key(|(_, priority)| *priority);
    // ... select provider
}
```

---

## 6. Routing Configuration

### 6.1 Python Router Configuration

```python
class RouterConfig(BaseModel):
    model_list: List[ModelConfig]

    # Cache
    redis_url: Optional[str] = None
    redis_host: Optional[str] = None
    redis_port: Optional[int] = None
    redis_password: Optional[str] = None
    cache_responses: Optional[bool] = False

    # Retry/Fallback
    num_retries: Optional[int] = 0
    timeout: Optional[float] = None
    fallbacks: Optional[List] = []
    context_window_fallbacks: Optional[List] = []
    allowed_fails: Optional[int] = None
    retry_after: Optional[int] = 0

    # Strategy
    routing_strategy: Literal[
        "simple-shuffle",
        "least-busy",
        "usage-based-routing",
        "latency-based-routing",
    ] = "simple-shuffle"
```

### 6.2 Rust Router Configuration

```rust
pub struct RouterConfig {
    pub routing_strategy: RoutingStrategy,  // Default: SimpleShuffle
    pub num_retries: u32,                   // Default: 3
    pub retry_after_secs: u64,              // Default: 0
    pub allowed_fails: u32,                 // Default: 3
    pub cooldown_time_secs: u64,            // Default: 5
    pub timeout_secs: u64,                  // Default: 60
    pub max_fallbacks: u32,                 // Default: 5
    pub enable_pre_call_checks: bool,       // Default: true
}
```

### 6.3 Runtime Configuration Updates

#### Python

```python
class UpdateRouterConfig(BaseModel):
    routing_strategy_args: Optional[dict] = None
    routing_strategy: Optional[str] = None
    model_group_retry_policy: Optional[dict] = None
    allowed_fails: Optional[int] = None
    cooldown_time: Optional[float] = None
    num_retries: Optional[int] = None
    timeout: Optional[float] = None
    max_retries: Optional[int] = None
    retry_after: Optional[float] = None
    fallbacks: Optional[List[dict]] = None
    context_window_fallbacks: Optional[List[dict]] = None

# Runtime update
router.update_settings(UpdateRouterConfig(...))
```

#### Rust

```rust
impl Router {
    // Builder pattern for configuration
    pub fn with_fallback_config(mut self, config: FallbackConfig) -> Self {
        self.fallback_config = config;
        self
    }

    // Runtime mutation
    pub fn set_fallback_config(&mut self, config: FallbackConfig) {
        self.fallback_config = config;
    }
}
```

---

## 7. Implementation Differences

### 7.1 Concurrency Model

| Aspect | Python | Rust |
|--------|--------|------|
| **Async Runtime** | asyncio | Tokio |
| **State Storage** | Redis/DualCache | DashMap + Atomics |
| **Lock Strategy** | Distributed locks | Low-contention per-deployment rollover gate |
| **Memory Model** | GIL-protected | Atomic ordering |

### 7.2 State Tracking

#### Python State Flow
```
Request -> CustomLogger.log_pre_api_call()
        -> Redis incr(deployment_count)
        -> API Call
        -> CustomLogger.log_success/failure()
        -> Redis decr(deployment_count)
```

#### Rust State Flow
```
Request -> Deployment.state.active_requests.fetch_add(1)
        -> API Call
        -> Deployment.record_success/failure()
        -> Deployment.state.active_requests.fetch_sub(1)
```

### 7.3 Performance Characteristics

| Metric | Python | Rust |
|--------|--------|------|
| **Routing Overhead** | ~1-5ms (Redis RTT) | <0.1ms (atomic ops) |
| **Memory per Deployment** | External cache | ~256 bytes inline |
| **Scalability** | Horizontal (Redis) | Vertical (multi-core) |
| **Distributed Support** | Native | Requires extension |

---

## 8. Feature Matrix

### 8.1 Complete Feature Comparison

| Feature | Python LiteLLM | Rust LiteLLM-RS |
|---------|---------------|-----------------|
| **Routing Strategies** | | |
| Simple Shuffle (Weighted) | Yes | Yes |
| Least Busy | Yes | Yes |
| Latency Based | Yes | Yes |
| Cost Based | Yes | Yes |
| Usage Based (TPM/RPM) | Yes | Yes |
| Usage Based V2 | Yes | No |
| Provider Budget Routing | Yes | No |
| Rate Limit Aware | No | Yes |
| Round Robin | No | Yes |
| Custom Strategy | Yes | No |
| **Load Balancing** | | |
| Weighted Distribution | Yes | Yes |
| Priority Ordering | Yes | Yes |
| TPM/RPM Limits | Yes | Yes |
| Max Parallel Requests | Yes | Yes |
| **Health Management** | | |
| Cooldown State | Yes | Yes |
| Degraded State | Implicit | Explicit |
| Health Checks | Via Callbacks | Atomic State |
| **Fallback** | | |
| General Fallback | Yes | Yes |
| Context Window Fallback | Yes | Yes |
| Content Policy Fallback | Yes | Yes |
| Rate Limit Fallback | No | Yes |
| Wildcard Fallback | Yes | No |
| **Retry** | | |
| Configurable Retries | Yes | Yes |
| Per-Error Retry Policy | Yes | No |
| Exponential Backoff | No | Yes |
| **Model Routing** | | |
| Model Aliases | Yes | Yes |
| Tag-Based Routing | Yes | Yes |
| Wildcard Patterns | Yes | No |
| Group-Based Routing | Implicit | Explicit |
| **Configuration** | | |
| Runtime Updates | Yes | Limited |
| Distributed State | Yes | No |
| Persistent State | Yes (Redis) | No |

### 8.2 Recommendations

**Use Python LiteLLM when:**
- Distributed deployments with multiple gateway instances
- Provider budget limiting is required
- Custom routing strategies are needed
- Per-error-type retry policies are important
- Wildcard model patterns are needed

**Use Rust LiteLLM-RS when:**
- Maximum throughput is critical (<10ms overhead)
- Single-instance deployment
- Memory efficiency is important
- Rate limit avoidance is priority
- Deterministic round-robin is needed

---

## Appendix: Code References

### Python Files
- Router: `/litellm/router.py`
- Types: `/litellm/types/router.py`
- Strategies: `/litellm/router_strategy/`
- Cooldown: `/litellm/router_utils/cooldown_handlers.py`
- Fallback: `/litellm/router_utils/fallback_event_handlers.py`

### Rust Files
- Router: `/src/core/router/router.rs`
- Config: `/src/core/router/config.rs`
- Deployment: `/src/core/router/deployment.rs`
- Execution: `/src/core/router/execute_impl.rs`
- Fallback: `/src/core/router/fallback.rs`
- Selection: `/src/core/router/selection.rs`
- Tag Routing: `/src/core/router/load_balancer/tag_routing.rs`
