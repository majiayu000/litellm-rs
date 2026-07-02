# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@33028f3d`, 11 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/virtual_keys/types.rs`
at 825 lines. It is a virtual key DTO/type module where production DTO,
permission, rate-limit, and key generation definitions end at line 147 and the
oversized portion is inline unit tests starting at line 148.

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

## Current Tranche: Virtual Key Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Virtual key production | `src/core/virtual_keys/types.rs` | Defines `VirtualKey`, `RateLimits`, `Permission`, `RateLimitState`, and `KeyGenerationSettings`. | Public virtual key type paths and serde behavior must remain unchanged for callers. |
| Inline tests | `src/core/virtual_keys/types.rs` | `#[cfg(test)] mod tests` starts at line 148 and contains DTO, permission, rate-limit, default-setting, and simulation tests. | Moving these tests removes the U-16 violation without changing public types. |
| Extracted tests | `src/core/virtual_keys/types_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/virtual_keys/types.rs` as the production owner for all current public virtual key DTOs, permission enums, rate-limit state, and key generation defaults.
2. Replace the inline test module with `#[cfg(test)] #[path = "types_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/virtual_keys/types_tests.rs` without assertion, fixture, timestamp setup, permission variant, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create a virtual key type facade tree in this tranche because the production definitions are already below the ceiling.
6. Do not edit virtual key manager, request DTOs, auth, storage, budget runtime, or API behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/virtual_keys/types.rs` | Root keeps production virtual key type definitions and delegates tests to `types_tests.rs`. |
| P2 | `src/core/virtual_keys/types_tests.rs` | Original test names and assertions remain present. |
| P3 | virtual key public API | No public type, field, enum variant, serde shape, default, or helper simulation behavior changes. |
| P4 | file size | `wc -l src/core/virtual_keys/types.rs src/core/virtual_keys/types_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::virtual_keys::types --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/virtual_keys/types.rs`. |

## Risks

- Extracting tests changes the exact test source file while preserving the module path as `types::tests`, so the focused module filter remains `core::virtual_keys::types`.
- Public virtual key types are re-exported from `src/core/virtual_keys/mod.rs`, so this tranche must not change names, enum variants, serde derives, or field visibility.
- Budget/rate-limit validity simulations must move unchanged because they describe enforcement-adjacent expectations even though this file only owns DTOs.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/virtual_keys/types.rs src/core/virtual_keys/types_tests.rs`
- [ ] `cargo test core::virtual_keys::types --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the virtual key type unit tests back into `src/core/virtual_keys/types.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
