# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@620c7a07`, 12 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/budget/alerts.rs`
at 828 lines. It is a budget alert manager module where production alert,
storage, webhook, and stats code stays under the ceiling and the oversized
portion is inline async unit tests starting at line 530.

## Architecture Principles

1. Facade compatibility: when splitting public type files, the original module path keeps
   re-exporting the same public names with `pub use`.
2. Runtime ownership: runtime files split by one responsibility at a time, such as request
   mapping, response mapping, operation handlers, storage helpers, or error conversion.
3. Test-suite ownership: test-only content moves by behavior domain while keeping original
   assertions and focused test command coverage.
4. Minimal public-type churn: if a public type file is only oversized because of inline
   tests, extract tests first instead of inventing a facade hierarchy.
5. No silent degradation: moved code must preserve current error propagation and must not
   add warning-only fallbacks for previously failing states.
6. Bounded PRs: each tranche owns one file family and includes line-count proof plus focused
   tests.

## Queue Design

| Phase | Lane | Target examples | Verification pattern |
| --- | --- | --- | --- |
| P1 | Test suites | DataUtils tests, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | choose test extraction when tests cause the oversize; otherwise use facade + `pub use` |
| P3 | Runtime orchestrators | OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: Budget Alerts Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Alert production | `src/core/budget/alerts.rs` | Defines `BudgetAlertManager`, in-memory alert storage, webhook config, alert config, and stats DTOs. | Public budget alert paths and runtime behavior must remain unchanged for callers. |
| Inline tests | `src/core/budget/alerts.rs` | `#[cfg(test)] mod tests` starts at line 530 and contains async alert creation, acknowledgement, stats, webhook, config, and history tests. | Moving these tests removes the U-16 violation without changing runtime alert code. |
| Extracted tests | `src/core/budget/alerts_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/budget/alerts.rs` as the production owner for the alert manager, storage helper, webhook config, alert config, and stats DTO.
2. Replace the inline test module with `#[cfg(test)] #[path = "alerts_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/budget/alerts_tests.rs` without assertion, fixture, spend result, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API and private fields where the original tests already did.
5. Do not create a budget alert production subtree in this tranche because the production implementation is already below the ceiling.
6. Do not edit budget tracker, manager, middleware, provider limits, persistence, or API behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/budget/alerts.rs` | Root keeps production alert manager implementation and delegates tests to `alerts_tests.rs`. |
| P2 | `src/core/budget/alerts_tests.rs` | Original test names and assertions remain present. |
| P3 | budget alert API | No public type, field, method signature, alert state, or webhook config behavior changes. |
| P4 | file size | `wc -l src/core/budget/alerts.rs src/core/budget/alerts_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::budget::alerts --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/budget/alerts.rs`. |

## Risks

- Extracting tests changes the exact test source file while preserving the module path as `alerts::tests`, so the focused module filter remains `core::budget::alerts`.
- Tests currently read private `webhooks`, so the extracted path-backed module must remain a child module of `alerts.rs` rather than a sibling declared from `budget/mod.rs`.
- Alert thresholds, severity mapping, acknowledgement, history limits, and stats aggregation must move unchanged because they are billing-adjacent controls.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/budget/alerts.rs src/core/budget/alerts_tests.rs`
- [ ] `cargo test core::budget::alerts --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the budget alert unit tests back into `src/core/budget/alerts.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
