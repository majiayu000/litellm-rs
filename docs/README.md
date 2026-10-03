# LiteLLM-RS Documentation

A self-hosted Rust LLM gateway with OpenAI-compatible HTTP APIs, routing,
load balancing, and failover. The gateway is the primary product; Rust APIs
and legacy adapters have narrower coverage. See the current
[provider support matrix](../README.md#provider-support).

## Start with an HTTP request

For the self-hosted gateway, follow the [source quick start](../README.md#quick-start-self-hosted-gateway)
with [the development config](../config/gateway.dev.yaml.example). That config
binds the gateway to `127.0.0.1:8080`, permits anonymous local development, and
routes `local-model` to a separate local vLLM service at port 8000. It does not
start vLLM or download a model for you.

```sh
curl --fail-with-body http://127.0.0.1:8080/openapi.json
# Requires a running vLLM endpoint serving the configured local-model:
curl --fail-with-body http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  --data '{"model":"local-model","messages":[{"role":"user","content":"Say hello."}],"stream":false}'
```

The OpenAPI fetch verifies the HTTP contract is reachable; a generated reply
additionally requires a configured, reachable provider and a model it serves.
For a remote provider, use the [production-style config](../config/gateway.yaml.example)
and configure its credentials and gateway auth. Do not expose the anonymous
development config outside loopback.

## Gateway and Rust API questions

**Is this the Python LiteLLM SDK or a drop-in replacement?** This repository is
[majiayu000/litellm-rs](https://github.com/majiayu000/litellm-rs), a Rust gateway
and reusable kernel. Python LiteLLM has its own [proxy configuration](https://docs.litellm.ai/docs/proxy/quick_start).
Use this repository's schema, [inference contract](openapi/inference.json), and
[support matrix](../README.md#provider-support); matching names do not guarantee
configuration, provider, SDK, or feature parity.

**Which client path should I choose?** Use HTTP for an existing OpenAI-compatible
application, [Codex setup](guides/codex.md) for its Responses integration, or the
[Runtime-backed Rust API policy](../README.md#supported-product-surfaces) when
embedding the kernel. The library snippet below is a separate path from starting
the HTTP gateway. Legacy selector adapters have their own narrower matrix.

**A server starts but generation fails. What should I check?** Confirm the actual
provider endpoint, model identity and supported capability, then its authentication.
The development vLLM prerequisite is separate from gateway readiness. For native
Bedrock versus an OpenAI-compatible Bedrock proxy, follow the distinct
[native guide](providers/bedrock.md) and [proxy guide](providers/openai-compatible-bedrock-proxy.md).

**Why does docs.rs differ from main?** [docs.rs](https://docs.rs/litellm-rs) renders
published crate versions. GitHub main may contain unreleased changes; check
[Releases](https://github.com/majiayu000/litellm-rs/releases) and
[CHANGELOG](../CHANGELOG.md) before using source-only APIs. Bugs belong in
[Issues](https://github.com/majiayu000/litellm-rs/issues); source license is [MIT](../LICENSE).

## 📚 Documentation Structure

### Architecture & Design
- [System Overview](./architecture/system-overview.md) - Complete system architecture and design patterns
- [Error System](./architecture/error-system.md) - Unified error handling architecture and patterns
- [Provider Implementation](./architecture/provider-implementation.md) - Guide for implementing individual providers
- [Cost Compatibility](./architecture/cost-compatibility.md) - Pricing authority and legacy API lifecycle
- [Router Runtime](./architecture/router-runtime.md) - Canonical runtime ownership and facade lifecycle
- [GH838 subsystem migration](./architecture/GH838-subsystem-migration-0.6-to-0.7.md) - Runtime gates and 0.6-to-0.7 removals
- [Architecture Improvements](./architecture/improvements.md) - Historical improvements and optimizations

### Implementation Guides
- [Getting Started](../README.md#quick-start-self-hosted-gateway) - Quick start guide and basic usage
- [Configuration](../README.md#gateway-configuration) - Configuration management and environment setup
- [Codex](./guides/codex.md) - Use Codex with the Responses API compatibility layer
- [Deployment](../deployment/README.md) - Production deployment strategies
- [Testing](../CONTRIBUTING.md#testing) - Testing strategies and best practices

### Provider Documentation
- [Provider Overview](./providers/README.md) - Supported providers and capabilities
- [DeepSeek](./providers/deepseek.md) - DeepSeek V4 integration guide
- [Xiaomi MiMo](./providers/xiaomi-mimo.md) - Xiaomi MiMo V2.5 OpenAI-compatible guide
- [OpenAI implementation](../src/core/providers/openai/) - OpenAI request and response handling
- [Anthropic implementation](../src/core/providers/anthropic/) - Claude request and response handling
- [Adding Providers](./architecture/provider-implementation.md) - Step-by-step provider implementation

### Protocol gateways and libraries
- [MCP HTTP gateway](./gateway/mcp.md) - Authenticated Streamable HTTP proxy (`mcp` feature); [legacy library API](./protocols/mcp.md)
- [A2A library](./protocols/a2a.md) - Default-off `a2a` feature; no HTTP gateway route

### Examples & Tutorials
- [Basic Examples](../examples/README.md) - Simple completion examples
- [Advanced Features](../examples/) - Streaming, function calling, etc.
- [Integration Examples](../examples/) - Web frameworks and service integrations

## 🚀 Quick Start

```rust
use litellm_rs::{completion, user_message, system_message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let response = completion(
        "gpt-4",
        vec![
            system_message("You are a helpful assistant."),
            user_message("Hello, how are you?"),
        ],
        None,
    ).await?;
    
    if let Some(content) = &response.choices[0].message.content {
        println!("Response: {}", content);
    }
    Ok(())
}
```

## 🏗️ Architecture Highlights

- **High Performance**: Rust and Tokio, with a reproducible [gateway-overhead benchmark](./benchmarks/gateway-overhead.md)
- **OpenAI Compatible**: [Versioned inference contract](./openapi/inference.json)
- **Provider Coverage**: [Runtime and legacy adapter support](../README.md#provider-support) varies by provider and capability
- **Intelligent Routing**: Smart load balancing and failover
- **Gateway Controls**: Authentication, monitoring, and cost tracking
- **Type Safety**: Compile-time guarantees and zero-cost abstractions
- **MCP gateway**: Feature-gated, authenticated Streamable HTTP routes; see [runtime scope](./gateway/mcp.md)
- **Experimental A2A library**: Default-off agent protocol types; no mounted gateway route

## Pricing Configuration

`pricing.allow_degraded` only controls startup behavior when the pricing source
cannot be loaded. Request-time behavior for provider/model pairs without pricing
is configured separately with `pricing.unpriced_model_policy`, which defaults to
`reject`; `allow_unpriced` must be paired with the #831 settlement enforcement
tranche before it is used in production.

## 📊 Performance Benchmarks

Real benchmark results from our unified router (run with `cargo bench`):

### Single Operation Performance

| Operation | Time | Description |
|-----------|------|-------------|
| Router Creation | **39.4 ns** | Create empty router instance |
| Add Deployment | **1.04 µs** | Insert single deployment |
| Alias Resolution | **31.9 ns** | Model name alias lookup |

### Routing Strategy Performance (10 deployments)

| Strategy | Time | Use Case |
|----------|------|----------|
| **RoundRobin** | 1.24 µs | Equal distribution |
| **LatencyBased** | 1.81 µs | Lowest latency first |
| **SimpleShuffle** | 1.85 µs | Random selection |
| **LeastBusy** | 2.04 µs | Fewest active requests |

### Get Healthy Deployments (by count)

| Deployments | Time | Throughput |
|-------------|------|------------|
| 1 | 130 ns | ~7.7M ops/s |
| 5 | 388 ns | ~2.6M ops/s |
| 10 | 694 ns | ~1.4M ops/s |
| 50 | 3.2 µs | ~312K ops/s |
| 100 | 6.3 µs | ~159K ops/s |

### Concurrent Performance (historical benchmark)

| Concurrent Tasks | Time | Throughput |
|------------------|------|------------|
| 10 | 37.3 µs | ~268K ops/s |
| 50 | 97.7 µs | ~512K ops/s |
| 100 | 172 µs | ~581K ops/s |
| 500 | 721 µs | **~693K ops/s** |

### Key Performance Characteristics

- **Low-contention design**: Uses `DashMap`, atomics, and a per-deployment rollover gate
- **Static dispatch**: Provider enum avoids vtable overhead
- **Linear scaling**: Concurrent throughput scales with task count
- **Sub-microsecond routing**: Most strategies complete under 2µs

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark groups
cargo bench -- unified_router      # Router operations
cargo bench -- concurrent_router   # Concurrent performance
cargo bench -- cache_operations    # Cache benchmarks

# Generate HTML report
cargo bench -- --noplot  # Skip plot generation for faster runs
```

Benchmark results are generated using [Criterion.rs](https://github.com/bheisler/criterion.rs) and saved to `target/criterion/`.

## 📖 Key Concepts

### Provider System
LiteLLM-RS uses a trait-based provider system that ensures consistency across all AI providers while allowing for provider-specific optimizations.

### Routing Engine
Sophisticated routing with multiple strategies:
- Round Robin
- Least Latency
- Cost Optimized
- Health-Based
- Custom Weighted

### Rate Limiting
Redis-backed distributed rate limiting fails closed by default when Redis commands fail, preserving global limits across multi-node deployments. Operators that need the old per-process fallback behavior can set `rate_limit.redis_failure_mode: fail_open_local`; degraded Redis operations are still exported as `rate_limiter_degraded_total{operation,mode}`.

### Unified Error Handling
All provider-specific errors are mapped to a unified error system for consistent error handling across the entire system.

## 🛠️ Development

### Prerequisites
- Rust 1.70+
- PostgreSQL (optional)
- Redis (optional)

### Essential Commands
```bash
# Development
make dev              # Start development server
cargo test --all-features  # Run tests
cargo clippy --all-features  # Lint code

# Production
make build            # Build release binary
make docker           # Build Docker image
```

## 🤝 Contributing

1. Read the [Provider Implementation Guide](./architecture/provider-implementation.md)
2. Check existing [issues](https://github.com/majiayu000/litellm-rs/issues)
3. Follow the [development setup](../CONTRIBUTING.md#development-setup)
4. Submit PRs with tests and documentation

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.
