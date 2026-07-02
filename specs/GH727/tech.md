# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@e2d7495e`, 9 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/integrations/manager.rs`
at 821 lines. It is an integration manager module where production registration,
event dispatch, parallel/sequential handling, shutdown, and error aggregation end
at line 545 and the oversized portion is inline async unit tests starting at line 546.

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

## Current Tranche: Integration Manager Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Integration manager production | `src/core/integrations/manager.rs` | Defines manager config, registration APIs, event dispatch APIs, flush, shutdown, and dispatch helpers. | Integration runtime dispatch behavior must remain unchanged for callers. |
| Inline tests | `src/core/integrations/manager.rs` | `#[cfg(test)] mod tests` starts at line 546 and contains mock integration, registration, dispatch, fail-fast, sequential, and empty-manager tests. | Moving these tests removes the U-16 violation without changing runtime manager code. |
| Extracted tests | `src/core/integrations/manager_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/integrations/manager.rs` as the production owner for config builders, registration methods, event notification methods, flush/shutdown, and dispatch helpers.
2. Replace the inline test module with `#[cfg(test)] #[path = "manager_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/integrations/manager_tests.rs` without assertion, mock integration, counter, event fixture, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create an integration manager production helper tree in this tranche because the production implementation is already below the ceiling.
6. Do not edit integration trait contracts, event definitions, Langfuse/OpenTelemetry adapters, or API behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/integrations/manager.rs` | Root keeps production integration manager implementation and delegates tests to `manager_tests.rs`. |
| P2 | `src/core/integrations/manager_tests.rs` | Original test names and assertions remain present. |
| P3 | integration manager API | No config builder, registration, dispatch, fail-fast, log-errors, timeout, flush, or shutdown behavior changes. |
| P4 | file size | `wc -l src/core/integrations/manager.rs src/core/integrations/manager_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::integrations::manager --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/integrations/manager.rs`. |

## Risks

- Extracting tests changes the exact test source file while preserving the module path as `manager::tests`, so the focused module filter remains `core::integrations::manager`.
- Tests define a mock integration and inspect dispatch side effects through counters, so the extracted path-backed module must remain a child module of `manager.rs`.
- Integration dispatch is cross-cutting observability/runtime behavior, so this tranche must not alter trait contracts, concrete adapters, event types, or error propagation semantics.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/integrations/manager.rs src/core/integrations/manager_tests.rs`
- [ ] `cargo test core::integrations::manager --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the integration manager unit tests back into `src/core/integrations/manager.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
