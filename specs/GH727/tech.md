# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@e7fd7a121a69`, 29 tracked Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/core/router/tests/strategy_impl_tests.rs` at
880 lines. It is a test-only suite that mixes routing context construction,
weighted random, least busy, lowest usage, lowest latency, lowest priority,
rate-limit-aware, round-robin, and consistency coverage in one module.

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

## Current Tranche: Router Strategy Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test root | `src/core/router/tests/strategy_impl_tests.rs` | Contains all strategy implementation tests inline. | It is 880 lines and mixes unrelated strategy domains. |
| Production strategy implementation | `src/core/router/strategy_impl.rs` | Implements routing strategy helpers. | Production selection code must remain untouched in this tranche. |
| Deployment model | `src/core/router/deployment.rs` | Provides deployment config/state used by test fixtures. | Fixtures remain test-only helpers in the root. |
| Module mount | `src/core/router/tests/mod.rs` | Mounts `mod strategy_impl_tests;`. | The focused test path must remain `core::router::tests::strategy_impl_tests`. |

### Design

1. Keep `src/core/router/tests/strategy_impl_tests.rs` as the shared test root with common imports, provider/deployment fixture helpers, and child module declarations only.
2. Add `context_tests.rs` for `build_routing_contexts` coverage.
3. Add `weighted_random_tests.rs`, `least_busy_tests.rs`, `lowest_usage_tests.rs`, `lowest_latency_tests.rs`, and `lowest_priority_tests.rs` for per-strategy scoring and empty-candidate coverage.
4. Add `rate_limit_aware_tests.rs` for TPM/RPM headroom and no-limit behavior.
5. Add `round_robin_tests.rs` for candidate cycling, model-scoped counters, wraparound, and context cycling.
6. Add `integration_tests.rs` for deterministic strategy consistency coverage.
7. Do not edit production router modules.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `strategy_impl_tests.rs` | Root keeps shared helpers and child module declarations only. |
| P2 | `strategy_impl_tests/*.rs` | Original test functions remain discoverable by name under strategy-domain modules. |
| P3 | file size | `wc -l src/core/router/tests/strategy_impl_tests.rs src/core/router/tests/strategy_impl_tests/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test core::router::tests::strategy_impl_tests --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/router/tests/strategy_impl_tests.rs`. |

## Risks

- Child modules must import shared parent helpers correctly; losing fixture helpers or `DashMap`/`AtomicUsize` imports breaks discovery.
- Round-robin tests rely on the same model-keyed counters; assertions and counter setup must remain unchanged.
- Splitting test modules must not change the router tests module mount path.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::router::tests::strategy_impl_tests --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/core/router/tests/strategy_impl_tests.rs` and `src/core/router/tests/strategy_impl_tests/*.rs`

## Rollback

Revert the Router strategy test-suite split and `specs/GH727` edits. No
production code, schema changes, or runtime behavior changes are involved.
