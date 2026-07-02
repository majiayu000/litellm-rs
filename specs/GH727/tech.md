# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@750bbe437884`, 38 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Analytics Types Facade

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Analytics type facade | `src/core/analytics/types.rs` | Defines request/provider, user usage, cost, and budget DTOs plus inline tests. | The file is 1071 lines and is a Lane B public type facade. |
| Analytics module exports | `src/core/analytics/mod.rs` | Re-exports all analytics DTOs from `types`. | Root `types.rs` must keep the original public names so downstream imports remain compatible. |
| Analytics consumers | `collector.rs`, `engine.rs`, `optimizer.rs` | Import DTOs through `super::types`. | The split can be private child modules with facade re-exports; no runtime modules should need edits. |

### Design

1. Keep `src/core/analytics/types.rs` as the root facade.
2. Move public DTOs into private child modules under `src/core/analytics/types/`:
   - `request.rs`: `AnalyticsRequestMetrics`, `ProviderMetrics`
   - `usage.rs`: `UserMetrics`, `TokenUsage`, `ModelUsage`, `UsagePatterns`,
     `RequestSizeDistribution`, `SeasonalTrend`
   - `cost.rs`: `CostBreakdown`, `DailyCost`, `CostMetrics`, `CostTrend`,
     `BudgetUtilization`
3. Root facade declares the child modules and `pub use`s every original public type name.
4. Move original inline tests into `src/core/analytics/types_tests/`:
   - `request_tests.rs`
   - `usage_tests.rs`
   - `cost_tests.rs`
   - `workflow_tests.rs`
5. Do not edit analytics runtime modules or `src/core/analytics/mod.rs` unless compilation proves a facade compatibility gap.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | root `types.rs` | Re-exports every original analytics public type name. |
| P2 | child type modules | `cargo check --all-features --locked` proves downstream imports still compile. |
| P3 | child test modules | `cargo test core::analytics::types --lib --all-features` runs the moved tests. |
| P4 | file size | `wc -l src/core/analytics/types.rs src/core/analytics/types/*.rs src/core/analytics/types_tests.rs src/core/analytics/types_tests/*.rs` shows all touched files below 800. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/analytics/types.rs`. |

## Risks

- Facade re-export omissions would break callers that import from `core::analytics::types::*`.
- Multiple modules contain similarly named DTOs in other domains; this tranche must only move analytics DTOs.
- Inline tests move one module deeper, but the root filter remains stable.
- This tranche reduces one of the remaining 38 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::analytics::types --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/analytics/types.rs`, `types/*.rs`, and `types_tests/*.rs`

## Rollback

Revert the analytics type/test split and `specs/GH727` edits. No migrations or
runtime code changes are involved.
