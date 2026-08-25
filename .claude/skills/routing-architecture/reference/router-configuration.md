## Router Configuration

### YAML Configuration

The gateway reads a top-level `router:` key into `GatewayRouterConfig`
(`src/config/models/router.rs:16`, `#[serde(deny_unknown_fields)]`). Keys like
`routing.health_check.*`, `routing.fallback.*`, `load_balancing.weights`, or
`rate_limit.track_per_provider` do not exist — unknown keys fail config parsing.

```yaml
router:
  strategy: "priority_based"    # snake_case RoutingStrategy
  circuit_breaker:
    failure_threshold: 5        # failures per minute before cooldown -> RouterConfig.allowed_fails
    recovery_timeout: 60        # cooldown seconds -> RouterConfig.cooldown_time_secs
    min_requests: 10            # min requests this minute before the breaker may trip
    success_threshold: 3        # consecutive successes to promote Degraded -> Healthy
  load_balancer:
    health_check_enabled: true  # -> RouterConfig.enable_pre_call_checks (gates active probes)
```

### Gateway-to-Runtime Mapping

`runtime_router_config_from_gateway` (`src/core/router/gateway_config.rs:156-170`) maps:

| YAML key | `RouterConfig` field | `RouterConfig::default()` | Gateway effective default |
|----------|----------------------|---------------------------|---------------------------|
| `router.strategy` | `routing_strategy` | `SimpleShuffle` | `RoundRobin` |
| `circuit_breaker.failure_threshold` | `allowed_fails` | 3 | 5 |
| `circuit_breaker.recovery_timeout` | `cooldown_time_secs` | 5 | 60 |
| `circuit_breaker.min_requests` | `min_requests` | 10 | 10 |
| `circuit_breaker.success_threshold` | `success_threshold` | 3 | 3 |
| `load_balancer.health_check_enabled` | `enable_pre_call_checks` | true | true |

Not exposed via YAML — `num_retries` (3), `retry_after_secs` (0), `timeout_secs` (60),
and `max_fallbacks` (5) keep their `RouterConfig::default()` values unless the router is
built programmatically (`src/core/router/config.rs:97-112`).

The gateway deserializes its own defaults before mapping them into `RouterConfig`, so
omitting values does not preserve every programmatic default. In particular it runs
RoundRobin with failure threshold 5 and 60-second recovery, while a directly constructed
`RouterConfig::default()` uses SimpleShuffle, 3, and 5 seconds.

`load_balancer.sticky_sessions` / `session_timeout` exist on the struct but validation
rejects any non-default value ("not implemented by runtime router yet",
`src/config/validation/router_validators.rs:46-56`).

### Per-Provider Routing Knobs

Routing-relevant fields live on each provider in `providers:`, not under `router:`.
`ProviderConfig` (`src/config/models/provider.rs:13-70`) carries `weight`, `priority`,
`rpm`, `tpm`, `max_concurrent_requests`, `timeout`, `max_retries`, plus:

```yaml
providers:
  - name: "openai-primary"
    provider_type: "openai"
    weight: 10.0          # SimpleShuffle weighting
    priority: 0           # PriorityBased; lower wins
    rpm: 1000             # rpm_limit fed to RateLimitAware/UsageBased filtering
    tpm: 100000           # tpm_limit, same
    max_concurrent_requests: 100   # max_parallel_requests cap during selection
    timeout: 30
    max_retries: 3
    retry:                # RetrySchedule: base_delay_ms/max_delay_ms/backoff/jitter
      base_delay: 100
      max_delay: 5000
      backoff_multiplier: 2.0
      jitter: 0.1
    health_check:         # ProviderHealthCheckConfig -> active probe policy
      interval: 30
      failure_threshold: 5
      recovery_timeout: 60
      endpoint: null      # null = native provider health_check()
      expected_codes: [200]
    models: ["gpt-4o"]
```

With a non-empty `models` list, each enabled provider becomes one deployment per listed
model with ID `{provider_name}-{model}`. When `models` is omitted or empty, construction
instead expands `provider.list_models()`: discovered models use that same deployment-ID
form. If the provider reports no models, its configured provider name becomes the route
and deployment ID. Provider compatibility policy can also preserve that provider-name
route alongside discovered models (currently `meta_llama`); that route likewise uses
`{provider_name}` as its deployment ID (`gateway_config.rs:228-268`).

### Gateway Wiring

There is no `create_router` factory. Construction path:

1. `Router::from_gateway_config_with_aliases` (`gateway_config.rs:186`) builds a
   deployment per provider+model via the provider factory, stages model aliases, and
   installs them in one snapshot update, then starts configured health checks before
   returning.
2. `HttpServer::new` places the router directly in `AppState` as an
   `Arc<UnifiedRouter>` (`src/server/http.rs:147-183`, `src/server/state.rs:40,69-94`).
3. Gateway request handlers route through `state.unified_router`; the gateway does not
   install or replace a process-default `RuntimeBinding`.

`RuntimeBinding`, `install_default_runtime` / `replace_default_runtime`, and
`RuntimeHandle::bind()` are separate library APIs used by the completion compatibility
facade. They are not the HTTP gateway's router ownership or request path.
