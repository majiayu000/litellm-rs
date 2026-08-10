# LiteLLM-RS Documentation

A high-performance AI Gateway written in Rust that provides unified access to 100+ AI providers through OpenAI-compatible APIs.

## 📚 Documentation Structure

### Architecture & Design
- [System Overview](./architecture/system-overview.md) - Complete system architecture and design patterns
- [Error System](./architecture/error-system.md) - Unified error handling architecture and patterns
- [Provider Implementation](./architecture/provider-implementation.md) - Guide for implementing individual providers
- [Architecture Improvements](./architecture/improvements.md) - Historical improvements and optimizations

### Implementation Guides
- [Getting Started](./guides/getting-started.md) - Quick start guide and basic usage
- [Configuration](./guides/configuration.md) - Configuration management and environment setup
- [Codex](./guides/codex.md) - Use Codex with the Responses API compatibility layer
- [Deployment](./guides/deployment.md) - Production deployment strategies
- [Testing](./guides/testing.md) - Testing strategies and best practices

### Provider Documentation
- [Provider Overview](./providers/README.md) - Supported providers and capabilities
- [DeepSeek](./providers/deepseek.md) - DeepSeek V4 integration guide
- [Xiaomi MiMo](./providers/xiaomi-mimo.md) - Xiaomi MiMo V2.5 OpenAI-compatible guide
- [OpenAI](./providers/openai.md) - OpenAI and compatible providers
- [Anthropic](./providers/anthropic.md) - Claude models integration
- [Adding Providers](./providers/adding-new-provider.md) - Step-by-step provider implementation

### Protocol Gateways
- [MCP Gateway](./protocols/mcp.md) - Model Context Protocol integration
- [A2A Protocol](./protocols/a2a.md) - Agent-to-Agent communication

### Examples & Tutorials
- [Basic Examples](./examples/basic-usage.md) - Simple completion examples
- [Advanced Features](./examples/advanced-features.md) - Streaming, function calling, etc.
- [Integration Examples](./examples/integrations.md) - Web frameworks and service integrations

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
    
    println!("Response: {}", response.choices[0].message.content);
    Ok(())
}
```

## 🏗️ Architecture Highlights

- **High Performance**: Built with Rust and Tokio for maximum throughput (10,000+ req/s)
- **OpenAI Compatible**: Drop-in replacement for OpenAI API
- **100+ Providers**: Unified interface to all major AI providers
- **Intelligent Routing**: Smart load balancing and failover
- **Enterprise Ready**: Authentication, monitoring, cost tracking
- **Type Safety**: Compile-time guarantees and zero-cost abstractions
- **MCP Gateway**: Model Context Protocol for external tool integration
- **A2A Protocol**: Agent-to-Agent communication with multi-provider support

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
| Record Success | **47.3 ns** | Atomic counter update (lock-free) |
| Record Failure | **65.5 ns** | Atomic failure counter update |

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

### Concurrent Performance (lock-free operations)

| Concurrent Tasks | Time | Throughput |
|------------------|------|------------|
| 10 | 37.3 µs | ~268K ops/s |
| 50 | 97.7 µs | ~512K ops/s |
| 100 | 172 µs | ~581K ops/s |
| 500 | 721 µs | **~693K ops/s** |

### Key Performance Characteristics

- **Lock-free design**: Uses `DashMap` and atomic operations for zero-lock concurrent access
- **Static dispatch**: Provider enum avoids vtable overhead
- **Nanosecond-level atomic ops**: Record success/failure in ~50ns
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
2. Check existing [issues](https://github.com/your-org/litellm-rs/issues)
3. Follow the [development setup](./guides/getting-started.md#development-setup)
4. Submit PRs with tests and documentation

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.
