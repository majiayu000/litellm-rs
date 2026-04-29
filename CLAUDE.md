# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Version Management

### Automated Version Bumping (CI/CD)

Version bumping is **fully automated** via GitHub Actions. On every push to `main`:

1. **Analyzes commits** using [Conventional Commits](https://conventionalcommits.org/):
   - `feat:` → Minor bump (0.1.x → 0.2.0)
   - `fix:`, `perf:`, `refactor:` → Patch bump (0.1.3 → 0.1.4)
   - `feat!:`, `BREAKING CHANGE:` → Major bump (0.x.x → 1.0.0)

2. **Auto-updates**:
   - `Cargo.toml` version
   - `CHANGELOG.md` with categorized changes
   - Creates git tag `v0.1.4`

3. **Triggers release pipeline** → builds binaries, Docker images, publishes to crates.io

### Commit Message Format

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`

**Examples**:
```bash
git commit -m "feat(router): add weighted load balancing"     # → minor
git commit -m "fix(auth): resolve JWT validation issue"       # → patch
git commit -m "feat!: redesign provider interface"            # → major
```

### Version Info in Code

Access build information programmatically:
```rust
use litellm_rs::{VERSION, GIT_HASH, full_version, build_info};

println!("Version: {}", VERSION);           // "0.1.4"
println!("Full: {}", full_version());       // "0.1.4-a1b2c3d"
println!("Info: {}", build_info());         // "0.1.4-a1b2c3d (built 1704067200 with rustc 1.87)"
```

### Manual Release (if needed)

```bash
# 1. Update version
cargo set-version 0.1.4

# 2. Update CHANGELOG.md

# 3. Commit and tag
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): bump version to 0.1.4"
git tag -a v0.1.4 -m "Release v0.1.4"
git push && git push --tags
```

### Quick Commands
```bash
make version          # Show current version info
cargo pkgid           # Show package identifier with version
```

## Essential Commands

### Development Commands
- **Start development**: `make dev` or `cargo run` (auto-loads config/gateway.yaml)
- **Build**: `cargo build --all-features` 
- **Test**: `cargo test --all-features`
- **Lint**: `cargo clippy --all-targets --all-features -- -D warnings`
- **Format**: `cargo fmt --all`
- **Quick start**: `make start` (fastest way to start the gateway)

### Testing Commands
- **All tests**: `make test`
- **Unit tests only**: `make test-unit` 
- **Integration tests**: `make test-integration`
- **Test coverage**: `make test-coverage`
- **Single test**: `cargo test <test_name> --all-features`

### Development Services
- **Start dev services**: `make dev-services` (starts PostgreSQL, Redis)
- **Stop dev services**: `make dev-stop`
- **Database migration**: `make db-migrate`
- **Reset database**: `make db-reset`

## Architecture Overview

This is a **high-performance AI Gateway** written in Rust that provides OpenAI-compatible APIs with intelligent routing across 100+ AI providers. It's a Rust implementation of the Python LiteLLM library, designed for production environments requiring maximum throughput and minimal latency.

### Core Components

**Gateway Architecture**: Modular, trait-based design with dependency injection
- `src/core/` - Central orchestrator and business logic
- `src/server/` - Actix-web HTTP server with middleware pipeline
- `src/auth/` - Multi-layered authentication (JWT, API keys, RBAC)
- `src/core/providers/` - Pluggable provider system (OpenAI, Anthropic, Azure, Google, etc.)
- `src/core/router/` - Intelligent routing with multiple strategies
- `src/core/mcp/` - MCP Gateway for external tool integration (90 tests)
- `src/core/a2a/` - A2A Protocol for agent-to-agent communication (48 tests)
- `src/storage/` - Multi-backend storage (PostgreSQL, Redis, S3, Vector DB)
- `src/monitoring/` - Observability (Prometheus, tracing, health checks)

### Key Design Patterns
- **Async-first**: All I/O is non-blocking using Tokio
- **Trait-based abstractions**: Pluggable components via traits
- **Error handling**: Comprehensive error types with context preservation
- **Configuration**: Type-safe config models with Default implementations
- **No backward compatibility**: Break old formats freely - prioritize clean architecture over legacy support

### Provider Integration
- **Unified Provider trait**: Common interface for all AI providers
- **Format conversion**: Automatic translation between OpenAI and provider-specific APIs
- **Health monitoring**: Per-provider health checks and failover
- **Cost calculation**: Built-in token counting and cost estimation

### Request Flow
1. HTTP Request → Authentication → Authorization → Router → Provider → Response
2. Middleware pipeline handles auth, logging, metrics, and transformations
3. Intelligent routing selects optimal provider based on health, latency, cost

## Configuration

- **Main config**: `config/gateway.yaml` (auto-loaded by default)
- **Example config**: `config/gateway.yaml.example`
- **Environment variables**: Override config values with `${ENV_VAR}` syntax
- **Config validation**: `make config-validate`

## Important Files

- `src/main.rs` - Application entry point
- `src/lib.rs` - Library entry point with core Gateway struct and Python LiteLLM compatible exports
- `Cargo.toml` - Dependencies and features (use `--all-features` for development)
- `Makefile` - All development commands and workflows
- `config/gateway.yaml` - Main configuration file

## Binaries

- `gateway` (default) - Main gateway server

## Features

The codebase uses Cargo features extensively:
- **Storage**: `postgres`, `sqlite`, `redis`, `s3`
- **Monitoring**: `metrics`, `tracing` 
- **Advanced**: `vector-db`, `websockets`, `analytics`, `enterprise`
- **Development**: Use `--all-features` flag for full functionality

## Database & Storage

- **Primary DB**: PostgreSQL with Sea-ORM migrations
- **Cache**: Redis for high-speed operations
- **File storage**: S3-compatible object storage
- **Vector DB**: Optional Qdrant integration for semantic caching

## Testing Architecture

- Unit tests in each module (`#[cfg(test)]`)
- Test files use inline tests within source files
- Postman collections for API testing (`tests/*.postman_collection.json`)
- Mock implementations for external services

## Provider Tiers

Providers are split into two tiers based on whether they need custom Rust code.

### Tier 1 — Catalog-only (zero code)

A provider belongs in Tier 1 when **all** of the following are true:

- The remote API is OpenAI-compatible (`/v1/chat/completions`, standard request/response shape)
- No custom request transformation is needed (no special headers, param filtering, or model-name mangling)
- No custom streaming logic is needed (standard SSE with `data: [DONE]`)
- No provider-specific model metadata is required at runtime

**How to add a Tier 1 provider**: add a single `def()` entry in
`src/core/providers/registry/catalog.rs` and a commented annotation in
`src/core/providers/mod.rs`:

```rust
// in catalog.rs
def("myprovider", "My Provider", "https://api.myprovider.com/v1", "MYPROVIDER_API_KEY"),

// in mod.rs
// myprovider: Tier 1 -> registry/catalog.rs
```

No other files need to change. The factory in `src/core/providers/factory/mod.rs`
automatically routes Tier 1 names through `OpenAILikeProvider`.

### Tier 2 — Code-based (custom implementation)

A provider requires Tier 2 treatment when **any** of the following apply:

- Non-OpenAI request/response format (e.g., Anthropic, Gemini, Cohere, Bedrock)
- Custom HTTP client with auth signing (e.g., AWS SigV4 for Bedrock, SageMaker)
- Unique streaming protocol (e.g., non-SSE, multipart, proprietary framing)
- Provider-specific model info or capability metadata
- Special parameter handling (e.g., tool-call transformation, response_format mapping)
- Rerank, embed, image-generation, or audio endpoints with diverging schemas

**How to add a Tier 2 provider**: create a directory under `src/core/providers/<name>/`
containing at minimum `mod.rs`, then add a variant to `ProviderType` and implement
the relevant trait methods. Also add the `pub mod <name>;` declaration in
`src/core/providers/mod.rs` (guarded by the appropriate feature flag).

### Resolving half-migrated providers

If `git status` shows `DU` (deleted-by-us, unresolved) files under `src/core/providers/`:

1. Decide the tier using the criteria above.
2. **Tier 1**: delete the directory and add a catalog entry + `mod.rs` comment.
3. **Tier 2**: restore the directory (`git checkout HEAD -- <path>`) and complete
   the implementation, or add stub methods that return `ProviderError::not_implemented`.
4. Verify with `cargo check --all-features` — zero DU files means no unresolved paths.

## Common Development Patterns

1. **Adding a Tier 1 provider**: add a `def()` entry in `src/core/providers/registry/catalog.rs`
2. **Adding a Tier 2 provider**: create a provider directory in `src/core/providers/<name>/`
3. **New API endpoints**: add routes in `src/server/routes/`
4. **Authentication**: extend auth modules in `src/auth/`
5. **Configuration**: update models in `src/config/models/`
6. **Monitoring**: add metrics in respective modules
7. **MCP servers**: add server configs in `src/core/mcp/config.rs`
8. **A2A agents**: add agent configs in `src/core/a2a/config.rs`

## Protocol Gateways

### MCP Gateway (`src/core/mcp/`)
Model Context Protocol for connecting LLMs to external tools:
- `config.rs` - Server configuration, authentication (Bearer, API Key, OAuth 2.0)
- `transport.rs` - HTTP, SSE, WebSocket, stdio transports
- `protocol.rs` - JSON-RPC 2.0 implementation
- `tools.rs` - Tool definitions and invocation
- `server.rs` - Individual server connection management
- `gateway.rs` - Main gateway aggregating servers
- `permissions.rs` - Fine-grained access control

### A2A Protocol (`src/core/a2a/`)
Agent-to-Agent communication with multi-provider support:
- `config.rs` - Agent configuration, provider types
- `message.rs` - JSON-RPC 2.0 message format, task states
- `provider.rs` - Provider adapters (LangGraph, Vertex AI, Azure, Bedrock, Pydantic AI)
- `registry.rs` - Agent discovery and health monitoring
- `gateway.rs` - Main gateway for agent management

## Docker & Deployment

- **Docker build**: `make docker`
- **Development stack**: `make docker-compose-dev`
- **Production**: `make docker-compose`
- **Kubernetes**: `make k8s-apply`

## Performance Characteristics

- **Throughput**: 10,000+ requests/second
- **Latency**: <10ms routing overhead
- **Memory**: ~50MB base footprint
- **Architecture**: Fully async, connection pooling, zero-copy where possible

## Python LiteLLM Compatibility

This Rust implementation maintains API compatibility with the original Python LiteLLM:
- Core completion API exposed via `src/core/completion.rs`
- Helper functions: `completion()`, `user_message()`, `system_message()`, `assistant_message()`
- Unified interface for 100+ providers with automatic routing

## Agent / Multi-PR Rules

When using AI agents (Claude, Codex, Copilot) to create PRs:

### Branch Rules
- **One issue → one branch → one PR**. Never bundle unrelated fixes.
- **Always branch from latest `main`**. Never fork from another feature branch.
- **Max 10 files / 500 lines per PR** (excluding Cargo.lock, docs). Use `scripts/guards/check_pr_scope.sh` to verify.
- **Run overlap check before pushing**: `scripts/guards/check_pr_overlap.sh` detects file conflicts with open PRs.

### Agent Isolation
- Parallel agents **must** use `git worktree` for isolation:
  ```bash
  git worktree add /tmp/agent-task-{id} -b fix/issue-{id} main
  ```
- Two agents must **never** modify the same file concurrently.

### Before Creating PR
1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --all-features`
4. `bash scripts/guards/check_pr_scope.sh`
5. `bash scripts/guards/check_pr_overlap.sh`

### Toolchain
- Rust version is pinned in `rust-toolchain.toml`. CI uses the same version.
- Never use `@stable` in CI — always reference the pinned version.

## Known Issues & Solutions

### docs.rs Build Issue
The `vector-db` feature (which includes `qdrant-client`) fails to build on docs.rs due to its read-only filesystem. The qdrant-client build script attempts to write files during compilation.

**Solution**:
- In `Cargo.toml`, the `[package.metadata.docs.rs]` section explicitly:
  - Sets `all-features = false` to prevent docs.rs from using `--all-features`
  - Lists specific features excluding `vector-db`
  - This allows documentation to build successfully on docs.rs

**Testing docs.rs compatibility locally**:
```bash
env DOCS_RS=1 cargo doc --no-deps --features "postgres sqlite redis s3 metrics tracing websockets analytics"
```
