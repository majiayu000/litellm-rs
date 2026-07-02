# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@720c2532`, 13 tracked Rust files remain over the U-16
800-line ceiling. One current largest file is `src/core/models/user/types.rs`
at 828 lines. It is a public user type/helper module where production
definitions end at line 274 and the oversized portion is inline unit tests.

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

## Current Tranche: User Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| User type production | `src/core/models/user/types.rs` | Defines `User`, roles, status, rate limits, profile, and user helper methods. | Public user model path and serde behavior must remain unchanged for callers. |
| Inline tests | `src/core/models/user/types.rs` | `#[cfg(test)] mod tests` starts at line 275 and contains role/status/profile/user behavior unit tests. | Moving these tests removes the U-16 violation without changing public types. |
| Extracted tests | `src/core/models/user/types_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/models/user/types.rs` as the production owner for all current public user types, enums, serde attributes, and helper methods.
2. Replace the inline test module with `#[cfg(test)] #[path = "types_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/models/user/types_tests.rs` without assertion, fixture, UUID setup, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create a user type facade tree in this tranche because the production definitions are already below the ceiling.
6. Do not edit user preferences/session/activity modules, storage, auth, team, billing, or API behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/models/user/types.rs` | Root keeps production user type definitions and delegates tests to `types_tests.rs`. |
| P2 | `src/core/models/user/types_tests.rs` | Original test names and assertions remain present. |
| P3 | user public API | No public type, field, serde attribute, method signature, role string, or password redaction behavior changes. |
| P4 | file size | `wc -l src/core/models/user/types.rs src/core/models/user/types_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::models::user::types --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/models/user/types.rs`. |

## Risks

- Extracting tests changes the exact test module path from inline `types::tests` to path-backed `types::tests`, but the focused module filter remains `core::models::user::types`.
- Public user model types are consumed broadly, so this tranche must not change serde names, field visibility, password redaction, helper signatures, or role hierarchy behavior.
- Metadata touch and usage accumulation expectations must move unchanged because they affect persistence and billing-adjacent state.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/models/user/types.rs src/core/models/user/types_tests.rs`
- [ ] `cargo test core::models::user::types --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the user type unit tests back into `src/core/models/user/types.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
