# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@c58aa820fead`, 43 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Cost Calculator Test Suite

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Cost calculator runtime | `src/core/cost/calculator.rs` | Provides `generic_cost_per_token`, pricing lookup, cost components, estimate, comparison, and fallback pricing behavior. | Runtime file is already below 800 lines; this tranche must not change it. |
| Cost calculator tests | `src/core/cost/calculator/tests.rs` | A single 1025-line test module covers pricing lookup, provider aliases, component costs, estimate/compare behavior, edge cases, and workflow consistency. | The file exceeds U-16 and mixes unrelated test domains. |
| Existing sibling test pattern | `src/core/cost/calculator/openai_current_tests.rs`, `gpt55_tests.rs`, `pricing_regression_tests.rs` | Calculator test files already live beside calculator submodules. | New child test modules should follow this local file layout instead of inventing a new harness. |

### Design

1. Keep `src/core/cost/calculator/tests.rs` as the parent test module.
2. Move shared helpers into the parent module:
   - `create_usage`
   - `assert_cost_eq`
3. Split tests into child modules under `src/core/cost/calculator/tests/`:
   - `pricing_lookup_tests.rs`: `generic_cost_per_token`, `get_model_pricing`, provider alias, and shared catalog lookup behavior.
   - `component_cost_tests.rs`: input, output, cache, audio, image, and reasoning component cost helpers.
   - `estimation_comparison_tests.rs`: `estimate_cost` and `compare_model_costs`.
   - `edge_case_tests.rs`: all-feature totals, large counts, case-insensitivity, new-model pricing, provider variants, cached-token saturation.
   - `workflow_tests.rs`: end-to-end cost workflow and estimate-vs-actual consistency.
4. Each child module imports from its parent module and `crate::core::cost::calculator::*`;
   no production code or public API changes are allowed.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `tests.rs` coordinator plus child modules | `cargo test core::cost::calculator --lib --all-features` discovers moved tests. |
| P2 | unchanged assertions | Focused tests pass without production changes. |
| P3 | file size | `wc -l src/core/cost/calculator/tests.rs src/core/cost/calculator/tests/*.rs` shows all touched files below 800. |
| P4 | no runtime behavior change | `git diff -- src/core/cost/calculator.rs` is empty. |

## Risks

- Child modules do not inherit parent imports automatically; each file must import the calculator API it uses.
- Shared helpers must stay in the parent module so every child can reuse the same token fixture and float assertion behavior.
- This tranche reduces only one of 43 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::cost::calculator --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/cost/calculator/tests.rs` and child modules

## Rollback

Revert the cost calculator test module split and `specs/GH727` edits. No migrations,
runtime config changes, or public API changes are involved.
