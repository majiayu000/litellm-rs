# Agent Teams Guide for litellm-rs

This document defines agent team configurations for the litellm-rs project.
Lead agents should read this file before spawning teammates.

## Project Context

- **Language**: Rust (1097 files, ~330K lines)
- **Architecture**: High-performance AI Gateway, trait-based, async-first
- **Providers**: 87 AI providers with unified interface
- **Verification**: `cargo test --all-features` / `cargo clippy --all-targets --all-features -- -D warnings`

## Team Roles

### 1. Architect

- **Mode**: plan (read-only)
- **Scope**: Cross-module design review, trait interface consistency, dependency analysis
- **Files**:
  - `src/core/mod.rs`
  - `src/core/traits/`
  - `src/core/router/`
  - `src/lib.rs`
  - `CLAUDE.md`
- **Rules**:
  - Does NOT write code
  - Outputs design decisions, interface contracts, and risk assessment
  - Challenges other agents' approaches when they violate architectural patterns

### 2. Provider Engineer

- **Mode**: default
- **Scope**: AI provider implementation, streaming, error mapping, cost calculation
- **Files**:
  - `src/core/providers/` (owned)
  - `src/core/streaming/` (shared with Router)
  - `src/core/cost/`
  - `src/core/types/`
- **Rules**:
  - Follow existing provider patterns (reference: `src/core/providers/anthropic/`)
  - Every provider must implement the `Provider` trait
  - Include unit tests in the same file (`#[cfg(test)]`)
  - Update cost config when adding new models

### 3. Router & Performance

- **Mode**: default
- **Scope**: Routing strategies, load balancing, fallback, caching, rate limiting, benchmarks
- **Files**:
  - `src/core/router/` (owned)
  - `src/core/cache/` (owned)
  - `src/core/rate_limiter/` (owned)
  - `src/core/streaming/` (shared with Provider Engineer)
  - `benches/`
- **Rules**:
  - All I/O must be non-blocking (Tokio async)
  - Benchmark before and after performance changes
  - Preserve fallback and retry semantics

### 4. Security & Auth

- **Mode**: default
- **Scope**: Authentication, authorization, secrets, MCP/A2A protocol security
- **Files**:
  - `src/auth/` (owned)
  - `src/core/security/` (owned)
  - `src/core/secret_managers/` (owned)
  - `src/core/virtual_keys/` (owned)
  - `src/core/mcp/permissions.rs`
  - `src/core/a2a/`
  - `src/core/ip_access/`
  - `src/core/guardrails/`
- **Rules**:
  - Never log secrets or tokens
  - Parameterized queries only (no string concatenation for SQL)
  - Validate all external input at system boundaries

### 5. QA & Integration

- **Mode**: default
- **Scope**: Testing, CI validation, documentation build verification
- **Files**:
  - `tests/` (owned)
  - `Makefile`
  - `codecov.yml`
  - Any module's `#[cfg(test)]` blocks (review only)
- **Rules**:
  - Run `cargo test --all-features` before reporting success
  - Run `cargo clippy --all-targets --all-features -- -D warnings` for lint
  - Verify docs.rs build: `env DOCS_RS=1 cargo doc --no-deps --features "postgres sqlite redis s3 metrics tracing websockets analytics"`
  - Report test coverage changes

## File Ownership

Prevent two agents from editing the same file. When boundaries overlap, coordinate via messaging.

```
src/core/providers/    -> Provider Engineer (exclusive)
src/core/router/       -> Router & Performance (exclusive)
src/core/cache/        -> Router & Performance (exclusive)
src/core/rate_limiter/ -> Router & Performance (exclusive)
src/auth/              -> Security & Auth (exclusive)
src/core/security/     -> Security & Auth (exclusive)
src/core/secret_managers/ -> Security & Auth (exclusive)
src/core/mcp/          -> Security & Auth (primary), Provider Engineer (read-only)
src/core/a2a/          -> Security & Auth (primary), Provider Engineer (read-only)
src/core/streaming/    -> Provider Engineer + Router (shared, coordinate before editing)
src/server/            -> Router & Performance (primary)
src/storage/           -> Router & Performance (primary)
src/monitoring/        -> QA & Integration (primary)
tests/                 -> QA & Integration (exclusive)
benches/               -> Router & Performance (exclusive)
```

## Spawn Prompts

### Full Team (5 agents) - Large refactors or cross-layer changes

```
Read AGENTS.md for team configuration. Create a 5-agent team:

1. Architect (plan mode): Review architecture impact, define interface contracts,
   challenge approaches. Focus on src/core/traits/ and src/core/mod.rs.

2. Provider Engineer: Implement provider changes per AGENTS.md file ownership.
   Reference src/core/providers/anthropic/ for patterns.

3. Router & Performance: Handle routing, caching, rate limiting changes.
   Benchmark before and after. Own src/core/router/ and src/core/cache/.

4. Security & Auth: Handle auth, secrets, permissions, protocol security.
   Own src/auth/ and src/core/security/. Never log sensitive data.

5. QA & Integration: Run cargo test --all-features and cargo clippy continuously.
   Report regressions immediately. Own tests/ directory.

Task: [describe your task here]
```

### New Provider (3 agents)

```
Read AGENTS.md. Create a 3-agent team to add [PROVIDER_NAME] provider:

1. Architect (plan mode): Analyze Provider trait interface, map [PROVIDER_NAME] API
   to existing patterns, review src/core/providers/anthropic/ as reference.

2. Provider Engineer: Implement the provider in src/core/providers/[provider_name]/,
   including chat completion, streaming, error mapping, and cost config.

3. QA: Write unit tests, run cargo test --all-features, verify no regressions.

Reference API docs: [URL]
```

### Bug Investigation (3 agents)

```
Read AGENTS.md. Create a 3-agent debug team for: [describe bug]

1. Agent A: Investigate [hypothesis 1], focus on [relevant module]
2. Agent B: Investigate [hypothesis 2], focus on [relevant module]
3. Agent C: Investigate [hypothesis 3], focus on [relevant module]

Challenge each other's findings. Coordinate via messaging.
The first agent to identify root cause should notify others.
```

### Security Audit (2 agents)

```
Read AGENTS.md. Create a 2-agent security audit team:

1. Security Auditor (plan mode): Scan src/auth/, src/core/security/,
   src/core/secret_managers/ for OWASP top 10 vulnerabilities.
   Check for hardcoded secrets, SQL injection, input validation gaps.

2. Fix Engineer: Apply fixes based on auditor findings.
   Run cargo test --all-features after each fix.
```

### Performance Optimization (2 agents)

```
Read AGENTS.md. Create a 2-agent performance team:

1. Profiler (plan mode): Analyze hot paths in src/core/router/ and
   src/core/streaming/. Identify allocation patterns, lock contention,
   unnecessary clones. Review benches/ for baseline metrics.

2. Optimizer: Implement targeted optimizations based on profiler findings.
   Run benchmarks before and after each change. Own src/core/router/ and benches/.
```

## Verification Checklist

Every team must complete before finishing:

1. `cargo build --all-features` passes
2. `cargo test --all-features` passes
3. `cargo clippy --all-targets --all-features -- -D warnings` clean
4. `cargo fmt --all -- --check` clean
5. No files edited outside of assigned ownership boundaries
