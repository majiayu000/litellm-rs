# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@c0aa3a6f`, 14 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/teams/manager.rs` at
830 lines. It is a team business-logic manager where production definitions end
at line 444 and the oversized portion is inline async unit tests.

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

## Current Tranche: Team Manager Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Team manager production | `src/core/teams/manager.rs` | Defines `TeamManager`, request DTOs, usage stats, team/member operations, role checks, and name validation. | Public manager path and method signatures must remain unchanged for callers. |
| Inline tests | `src/core/teams/manager.rs` | `#[cfg(test)] mod tests` starts at line 445 and contains async tests backed by `InMemoryTeamRepository`. | Moving these tests removes the U-16 violation without changing business logic. |
| Extracted tests | `src/core/teams/manager_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and in-memory repository fixture remain centralized against the same production module. |

### Design

1. Keep `src/core/teams/manager.rs` as the production owner for all current public team manager DTOs, methods, and validation helper.
2. Replace the inline test module with `#[cfg(test)] #[path = "manager_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/teams/manager_tests.rs` without assertion, fixture, UUID setup, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create operation submodules in this tranche because the production manager is already below the ceiling.
6. Do not edit repository trait/implementation, team models, storage, auth, billing, or usage behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/teams/manager.rs` | Root keeps production manager definitions and delegates tests to `manager_tests.rs`. |
| P2 | `src/core/teams/manager_tests.rs` | Original test names and assertions remain present. |
| P3 | team manager public API | No public type, field, method signature, validation string, repository call order, or error propagation changes. |
| P4 | file size | `wc -l src/core/teams/manager.rs src/core/teams/manager_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::teams::manager --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/teams/manager.rs`. |

## Risks

- Extracting tests changes the exact test module path from inline `manager::tests` to path-backed `manager::tests`, but the focused module filter remains `core::teams::manager`.
- `TeamManager` is a runtime business-logic boundary, so this tranche must not change method signatures, repository call order, validation strings, or error propagation.
- Last-owner protection and role checks must move unchanged because they protect team authorization semantics.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/teams/manager.rs src/core/teams/manager_tests.rs`
- [ ] `cargo test core::teams::manager --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the team manager unit tests back into `src/core/teams/manager.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
