# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@38b3140aeeca`, 28 tracked Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/auth/oauth/session.rs` at 869 lines. It is a
runtime module that mixes OAuth session model code, store trait/error contracts,
in-memory storage, Redis storage, and inline tests in one module.

## Architecture Principles

1. Facade compatibility: when splitting public type files, the original module path keeps
   re-exporting the same public names with `pub use`.
2. Runtime ownership: runtime files split by one responsibility at a time, such as request
   mapping, response mapping, operation handlers, storage helpers, or error conversion.
3. Test-suite ownership: test-only files split by behavior domain while keeping original
   assertions and focused test command coverage.
4. No silent degradation: moved code must preserve current error propagation and must not
   add warning-only fallbacks for previously failing states.
5. Bounded PRs: each tranche owns one file family and includes line-count proof plus focused
   tests.

## Queue Design

| Phase | Lane | Target examples | Verification pattern |
| --- | --- | --- | --- |
| P1 | Test suites | DataUtils tests, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: OAuth Session Runtime Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Facade root | `src/auth/oauth/session.rs` | Exposes all OAuth session public types today. | It must keep the original public import surface. |
| Session model | `src/auth/oauth/session/model.rs` | New child module for `OAuthSession`. | Keeps serde fields and builder methods isolated from storage code. |
| Store contract | `src/auth/oauth/session/store.rs` | New child module for `SessionStore` and `SessionError`. | Keeps trait and error contracts central and re-exported. |
| In-memory storage | `src/auth/oauth/session/memory_store.rs` | New child module for DashMap-backed storage. | Separates runtime cleanup/state logic from Redis code. |
| Redis storage | `src/auth/oauth/session/redis_store.rs` | New feature-gated child module. | Preserves Redis-only dependency usage and key/TTL behavior. |
| Tests | `src/auth/oauth/session/tests.rs` | New child test module. | Keeps existing focused session coverage under the same module path. |

### Design

1. Keep `src/auth/oauth/session.rs` as a facade with child module declarations and `pub use` re-exports for the original public names.
2. Move `OAuthSession` and its impl to `model.rs` without changing fields, serde attributes, or method signatures.
3. Move `SessionStore` and `SessionError` to `store.rs` without changing async trait method signatures or error variants.
4. Move `InMemorySessionStore`, its `Debug`, constructors, cleanup task, and trait implementation to `memory_store.rs`.
5. Move feature-gated `RedisSessionStore`, Redis key helpers, `Debug`, and trait implementation to `redis_store.rs`.
6. Move inline tests to `tests.rs` under the same parent module.
7. Do not edit OAuth handlers, middleware, client, provider discovery, or `src/auth/oauth/types.rs`.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `session.rs` | Root facade re-exports the same public OAuth session names. |
| P2 | `session/*.rs` | Runtime responsibilities are separated by model, contract, memory storage, Redis storage, and tests. |
| P3 | file size | `wc -l src/auth/oauth/session.rs src/auth/oauth/session/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test auth::oauth::session --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/auth/oauth/session.rs`. |

## Risks

- Root re-exports must preserve the original public paths used by `auth::oauth::mod.rs`.
- Redis code must stay behind the existing `redis` feature gate.
- State deletion must remain atomic in Redis through `get_del`.
- In-memory cleanup must continue removing expired sessions and expired OAuth states.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test auth::oauth::session --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/auth/oauth/session.rs` and `src/auth/oauth/session/*.rs`

## Rollback

Revert the OAuth session module split and `specs/GH727` edits. No schema changes
or runtime behavior changes are involved.
