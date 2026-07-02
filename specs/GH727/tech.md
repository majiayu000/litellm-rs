# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@d45b427dfb7c`, 39 Rust files remain over the U-16 800-line ceiling.
The highest-risk files are not all equivalent: some are pure test suites, some are public
type facades, and some are runtime orchestrators. They need different split patterns.

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
| P1 | Test suites | cost calculator, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, security, cost, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | Vertex AI client, unified provider, teams route, SeaORM team repository, OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: Router Concurrency Test Suite

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Router concurrency suite | `src/core/router/tests/concurrency_edge_case_tests.rs` | Contains concurrent selection, model-list swapping, weighted random, EMA, cooldown, and additional edge-case tests. | The file is 1079 lines and already has clear behavior-section boundaries. |
| Router test module | `src/core/router/tests/mod.rs` | Includes `mod concurrency_edge_case_tests;`. | The root module path should remain stable; the split can happen under a child directory. |
| Shared helpers | `router_tests::create_test_deployment`, router config/deployment/strategy imports | Shared by multiple test groups. | Keep shared imports in the root test module and have child modules import `super::*`. |

### Design

1. Keep `src/core/router/tests/concurrency_edge_case_tests.rs` as the root test module.
2. Move each existing behavior section into a child file under `src/core/router/tests/concurrency_edge_case_tests/`:
   - `concurrent_selection_tests.rs`
   - `model_list_swap_tests.rs`
   - `weighted_random_tests.rs`
   - `ema_latency_tests.rs`
   - `cooldown_expiry_tests.rs`
   - `additional_edge_case_tests.rs`
3. Root module keeps the doc comment, `#![allow(deprecated)]`, shared imports, and `mod` declarations.
4. Child modules use `use super::*;` and keep original assertions unchanged.
5. Do not edit router runtime code or `src/core/router/tests/mod.rs`.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | root `concurrency_edge_case_tests.rs` | Keeps shared imports and discovers all child modules. |
| P2 | child test modules | `cargo test core::router::tests::concurrency_edge_case_tests --lib --all-features` runs the moved tests. |
| P3 | file size | `wc -l src/core/router/tests/concurrency_edge_case_tests.rs src/core/router/tests/concurrency_edge_case_tests/*.rs` shows all touched files below 800. |
| P4 | queue count | `git ls-files '*.rs' | xargs wc -l | awk '$1 > 800 && $2 != "total" { print $1 " " $2 }' | sort -nr` shows the remaining queue no longer includes `concurrency_edge_case_tests.rs`. |

## Risks

- Test names move one module deeper, but the root filter remains stable. PR body should call out the path change.
- Shared imports must stay available to child modules without introducing a broad test prelude outside this file family.
- Weighted-random tests are statistical; assertions must not be weakened in this layout-only tranche.
- This tranche reduces one of the remaining 39 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::router::tests::concurrency_edge_case_tests --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/router/tests/concurrency_edge_case_tests.rs` and child files

## Rollback

Revert the router concurrency test split and `specs/GH727` edits. No migrations
or runtime code changes are involved.
