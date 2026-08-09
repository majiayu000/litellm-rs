# RFC: Workspace Crate Split for LiteLLM-RS

- Status: Draft for review
- Issue: #716
- Date: 2026-06-25
- Scope: RFC only, no code behavior change
- Related roadmap: `docs/plan/router-budget-provider-infra-hardening-spec.md`
- Source of truth: this RFC under `docs/plan/` supersedes historical
  issue-scoped planning snapshots.

## Summary

Split `litellm-rs` from a single package into a Cargo workspace that separates
the stable SDK and provider contracts from router, gateway, storage, and heavy
provider integrations.

The current package should remain as a compatibility facade during migration.
New crates can then expose smaller dependency surfaces for SDK-only users while
preserving the existing default gateway experience for current consumers.

This RFC intentionally does not start the split. The provider, router, budget,
and error contracts need to be stabilized first so the workspace boundary does
not freeze the wrong abstractions into semver-visible crates.

## Problem

`litellm-rs` currently owns the SDK, provider API, provider implementations,
router, gateway server, auth, storage, Redis, and infrastructure modules in one
crate.

The feature graph reflects that coupling:

- `default = ["sqlite", "redis", "metrics", "tracing"]`
- `sqlite` enables `storage`
- `storage` enables `gateway` and `redis`
- `gateway` enables Actix, auth, AES-GCM, Argon2, multipart, and CLI deps
- the `gateway` binary requires `storage`

This is workable for a gateway-first package, but it creates a broad default
dependency surface for SDK-only users and makes semver changes in unrelated
areas feel like one public API.

## Goals

- Define a staged workspace architecture for SDK, core types, provider API,
  provider implementations, router, gateway, and storage.
- Preserve current `litellm-rs` package behavior through a compatibility
  facade while the split lands.
- Make the default package behavior explicit: gateway users should keep a
  convenient default, while API-only users should get a small feature path.
- Reduce semver blast radius by moving unstable implementation details behind
  narrower crate contracts.
- Define migration phases that can land as non-breaking adapter releases.
- Name the existing issues that should land before implementation starts.

## Non-Goals

- Do not move modules into workspace crates in this PR.
- Do not change current Cargo features in this PR.
- Do not remove the current `litellm-rs` crate name.
- Do not decide every future provider package name before provider contract
  work is complete.
- Do not treat docs-only acceptance as proof that compile time improves.

## Current Evidence

- `Cargo.toml` declares a single library crate, `litellm_rs`, and one package
  named `litellm-rs`.
- `src/lib.rs` exports SDK, core, provider, router, server, storage, and gateway
  surfaces from one crate.
- `src/lib.rs` gates `auth`, `monitoring`, `server`, and `storage` behind the
  `gateway` feature, while SDK and core remain always visible.
- `src/core/providers/provider_type.rs` contains a closed provider selector enum
  plus a `Custom(String)` escape hatch.
- `src/core/providers/unified_provider.rs` makes `ProviderError` carry provider
  facts, retry policy helpers, and HTTP mapping documentation in one type.
- `src/core/budget/tracker.rs` still exposes separate `check_spend` and
  `record_spend` operations.
- The router has started moving toward lease-based reservations, but metadata,
  budget, provider, registry, and retry contracts are still tracked by open
  issues.

## Proposed Workspace Shape

### `litellm-core`

Stable shared primitives:

- canonical request and response value types
- model identifiers, deployment identifiers, capability identifiers
- config primitives that do not depend on gateway, storage, or provider crates
- shared error traits or low-level error facts
- telemetry-neutral metadata types

This crate must avoid Actix, SeaORM, Redis, object storage, provider-specific
HTTP clients, and gateway auth dependencies.

### `litellm-provider-api`

Provider-facing contracts:

- mandatory chat trait
- optional capability traits for embeddings, images, audio, batches, and
  streaming
- provider metadata and capability declarations
- fact-only provider failure data after #715
- object-safe provider trait boundaries after #713
- registry declaration data or validation hooks after #714

This crate should depend on `litellm-core`, not on router, gateway, or storage.

### Provider Implementation Crates

Initial options:

- `litellm-providers` for built-in low-dependency providers
- `litellm-provider-openai-compatible` for OpenAI-compatible catalog providers
- heavier native provider crates only when they carry meaningful optional deps

The first implementation phase should not create one crate per provider by
default. Start with grouped crates, then split heavy providers when dependency
or release cadence pressure justifies it.

Provider crates should depend on `litellm-core` and `litellm-provider-api`.
They should not depend on gateway or storage.

### `litellm-router`

Routing runtime:

- deployment selection
- routing strategy implementations
- cooldown and health integration
- metadata snapshot boundary after #710
- rate, parallel, and budget reservation integration after #711
- retry policy integration after #715

This crate should depend on `litellm-core`, `litellm-provider-api`, and selected
provider implementation crates through features.

### `litellm-gateway`

HTTP gateway:

- Actix server
- OpenAI-compatible HTTP adapters
- auth middleware
- request parsing and response mapping
- gateway-specific error to HTTP mapping after #715
- CLI entrypoint and gateway binary

This crate can depend on router, provider crates, storage, and auth
dependencies. It should not be a dependency of SDK-only users.

### `litellm-storage`

Persistence and external infrastructure:

- SeaORM entities and migrations
- Redis integration
- S3/object storage integration
- persisted budgets, keys, sessions, and pricing data
- storage repository traits and implementations

This crate should expose repository contracts that router and gateway can use
without making storage a default dependency of the SDK path.

### Compatibility Facade: `litellm-rs`

Keep the existing crate name as the public adapter during migration:

- re-export stable SDK/core APIs from new crates
- keep existing default features for at least the first adapter release
- provide opt-in compatibility aliases for old paths
- deprecate moved paths only after the new crate names and modules are stable
- keep the `gateway` binary behavior compatible unless a release note says
  otherwise

## Feature Compatibility Strategy

The first split should preserve old feature names on the facade crate.

| Current Feature | Facade Behavior During Migration | Target Owner |
| --- | --- | --- |
| `default` | Preserve current gateway-oriented behavior initially | `litellm-rs` facade |
| `lite` | SDK/core API without storage or gateway deps | `litellm-rs` facade + `litellm-core` |
| `gateway` | Enable HTTP server and auth stack | `litellm-gateway` |
| `storage` | Enable persistence adapters, not the gateway by default after migration | `litellm-storage` |
| `sqlite` | Enable SQLite storage adapter | `litellm-storage` |
| `postgres` | Enable Postgres storage adapter | `litellm-storage` |
| `redis` | Enable Redis storage/cache adapter | `litellm-storage` |
| `s3` | Enable object storage adapter | `litellm-storage` |
| `metrics` | Enable system metrics collection (`sysinfo`); `monitoring` is gated behind `gateway` today | `litellm-gateway` |
| `tracing` | Enable tracing subscriber; observability spans multiple crates | facade + per-crate feature passthrough |
| `mcp-validation` | Enable JSON Schema validation for MCP tool parameters | `litellm-gateway` |
| `vector-db` | Enable vector store for semantic caching (implies `storage`) | `litellm-storage` |
| `websockets` | Enable WebSocket transport surfaces | `litellm-gateway` |
| `analytics` | Enable analytics pipeline (proposed; confirm against analytics module in Phase 0) | `litellm-storage` (proposed) |
| `enterprise` | Aggregate `analytics` + `vector-db` bundle | `litellm-rs` facade |
| `aws-secrets` | Enable AWS Secrets Manager resolution | `litellm-storage` |
| `gcp-secrets` | Enable GCP Secret Manager resolution | `litellm-storage` |
| `azure-secrets` | Enable Azure Key Vault resolution | `litellm-storage` |
| `vault-secrets` | Enable HashiCorp Vault resolution | `litellm-storage` |
| `cloud-secrets` | Aggregate all cloud secret backends | `litellm-rs` facade |
| `providers-extra` | Enable grouped additional providers | provider crates |
| `providers-extended` | Enable heavier native providers | provider crates |
| `full` | Gateway + storage + all provider/infrastructure features | facade + workspace crates |

Longer term, `storage` should stop implying `gateway`; the gateway should
depend on storage when it needs persistence, not the other way around. That
decoupling should be a separate compatibility PR because it changes how users
reason about features.

The Target Owner column for the features added beyond the first draft
(`metrics`, `tracing`, `mcp-validation`, `vector-db`, `websockets`,
`analytics`, `enterprise`, and the `*-secrets` family) is a proposed assignment
derived from current module placement, not a frozen decision. Secret
resolution, analytics persistence, and observability each span more than one
crate, so Phase 0 boundary inventory must confirm these owners before any of
them is moved off the facade.

## Public API And Semver Strategy

1. Keep `litellm-rs` as the published compatibility package.
2. Add new workspace crates with explicit unstable or pre-1.0 versioning if the
   internal contracts are not stable enough.
3. Re-export stable types from the facade before moving public examples to new
   crates.
4. Use deprecation warnings only after replacement imports are documented.
5. Keep old imports compiling for at least two minor releases after the first
   split release.
6. Do not expose router/provider internals as stable APIs until #713, #714, and
   #715 settle the contracts.
7. Document any behavior-changing feature graph change in `CHANGELOG.md` and
   migration docs.

## Compile-Time And Dependency-Surface Impact

The RFC should be accepted with a measurement plan, not with guessed numbers.

Baseline commands before implementation:

```sh
cargo tree -e features --no-default-features --features lite
cargo tree -e features
cargo metadata --no-deps --format-version 1
cargo check --no-default-features --features lite
cargo check
```

Target outcomes after implementation:

- SDK-only builds do not compile Actix, SeaORM, Redis, Argon2, AES-GCM, or
  object storage dependencies.
- Gateway builds keep the current convenient default path.
- Provider-only crates compile without gateway or storage.
- Storage crates compile without provider-specific dispatch logic.
- The facade crate remains a compatibility adapter, not the only owner of every
  public contract.

## Migration Phases

### Phase 0: RFC And Boundary Inventory

- Land this RFC.
- Inventory public re-exports in `src/lib.rs`.
- Generate a dependency-surface baseline with `cargo tree -e features`.
- Mark which modules are public API, internal API, or compatibility-only.

### Phase 1: Stabilize Contracts In Place

Land the hardening issues before moving code:

- #710: routing metadata snapshot boundary
- #711: budget reserve and settle semantics
- #713: provider enum, trait, and handle contract alignment
- #714: provider declaration source and conformance tests
- #715: provider failure facts, retry policy, and HTTP mapping split
- #519: type-tree and provider abstraction roadmap, at least enough to avoid
  freezing known duplicate type systems into separate crates

This phase should happen inside the existing crate so behavior is easier to
test before crate boundaries add publication and feature-resolution complexity.

### Phase 2: Create Workspace Skeletons

- Add `Cargo.toml` workspace members.
- Add empty crates with minimal dependency graphs.
- Keep implementation in the existing crate.
- Add compile checks for each new crate.

### Phase 3: Move Core And Provider API

- Move canonical low-level types into `litellm-core`.
- Move provider traits and failure facts into `litellm-provider-api`.
- Re-export moved items from the facade.
- Add compatibility tests proving old imports still compile.

### Phase 4: Move Router And Storage

- Move router runtime after snapshot and reservation APIs are stable.
- Move storage implementations behind repository traits.
- Keep facade feature names mapped to new crate features.
- Add integration tests for gateway + storage feature combinations.

### Phase 5: Move Gateway And Providers

- Move Actix server, HTTP adapters, auth, and gateway binary into
  `litellm-gateway`.
- Move provider implementations into grouped provider crates.
- Split heavy providers only when dependencies or release cadence justify it.

### Phase 6: Deprecate Old Paths

- Update README examples.
- Add migration guide.
- Deprecate facade-only old paths once the replacement imports are stable.
- Keep adapter releases until downstream users have had a clear migration
  window.

## Required Issue Ordering

Workspace split implementation should wait until these issues are reviewed and
their accepted direction is reflected in code or an approved design:

1. #713, because provider API crate boundaries depend on whether routing uses a
   closed enum, object-safe custom providers, or a hybrid model.
2. #714, because provider crates need one declaration and conformance story.
3. #715, because provider failure facts must not pull gateway HTTP mapping into
   provider API crates.
4. #710, because router crate boundaries should wrap an atomic snapshot model,
   not split `DashMap` invariants.
5. #711, because router and storage boundaries need a stable budget
   authorization contract.
6. #519, because duplicated type trees and provider abstraction drift should not
   be baked into separate public crates.

The RFC can be reviewed before those issues land. The implementation split
should not start before these decisions are accepted.

## Risks

- Cargo feature unification can accidentally pull gateway/storage dependencies
  into SDK builds.
- Moving types too early can freeze unstable contracts as public crate APIs.
- Compatibility re-exports can hide dependency regressions unless measured with
  `cargo tree`.
- Provider crate granularity can create release overhead if split too finely.
- A facade crate can become a second source of truth if ownership is unclear.

## Acceptance Criteria

- The RFC names proposed workspace crates and their ownership boundaries.
- It defines feature compatibility during migration.
- It defines public API and semver strategy.
- It defines compile-time and dependency-surface measurements.
- It defines non-breaking migration phases.
- It names prerequisite issues before implementation starts.
- It does not change Rust behavior.

## Open Questions

- Should the facade package keep gateway-oriented `default` features forever, or
  only through a compatibility window?
- Should the SDK surface become its own `litellm-sdk` crate, or should
  SDK-friendly APIs live in `litellm-core` plus facade re-exports?
- Which provider implementations have enough dependency weight to justify
  standalone crates in the first split?
- Should new crates publish independently from the first workspace PR, or stay
  unpublished until the facade migration is proven?
