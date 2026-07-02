# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@c3acd9ad`, 8 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/integrations/langfuse/types.rs`
at 818 lines. It is a Langfuse API type module where production DTOs, builders,
ingestion event factories, batch helpers, and response DTOs end at line 642 and
the oversized portion is inline type/serialization tests starting at line 643.

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

## Current Tranche: Langfuse Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Langfuse type production | `src/core/integrations/langfuse/types.rs` | Defines Trace, Level, Usage, Generation, Span, IngestionEvent, IngestionBatch, and ingestion response DTOs. | Public Langfuse DTO paths and serde contracts must remain unchanged for callers. |
| Inline tests | `src/core/integrations/langfuse/types.rs` | `#[cfg(test)] mod tests` starts at line 643 and contains type builder, error marking, batch, and serde serialization tests. | Moving these tests removes the U-16 violation without changing public DTO code. |
| Extracted tests | `src/core/integrations/langfuse/types_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/integrations/langfuse/types.rs` as the production owner for Langfuse DTOs, builder helpers, ingestion event factories, batch helpers, and response DTOs.
2. Replace the inline test module with `#[cfg(test)] #[path = "types_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/integrations/langfuse/types_tests.rs` without assertion, fixture, serde expected-value, or batch behavior changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create a Langfuse public-type facade hierarchy in this tranche because the production type implementation is already below the ceiling.
6. Do not edit Langfuse client, integration adapter, auth, batching runtime, or HTTP behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/integrations/langfuse/types.rs` | Root keeps production Langfuse DTOs/builders/event factories and delegates tests to `types_tests.rs`. |
| P2 | `src/core/integrations/langfuse/types_tests.rs` | Original test names and assertions remain present. |
| P3 | Langfuse type API | No public DTO, serde casing, generated ID, timestamp default, builder, error, event factory, or batch behavior changes. |
| P4 | file size | `wc -l src/core/integrations/langfuse/types.rs src/core/integrations/langfuse/types_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::integrations::langfuse::types --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/integrations/langfuse/types.rs`. |

## Risks

- Extracting tests changes the exact test source file while preserving the module path as `types::tests`, so the focused module filter remains `core::integrations::langfuse::types`.
- Tests cover private/public builder behavior through the parent module, so the extracted path-backed module must remain a child module of `types.rs`.
- Langfuse DTOs are integration API contract types, so this tranche must not alter serde attributes, event tags, field names, or response DTO shape.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/integrations/langfuse/types.rs src/core/integrations/langfuse/types_tests.rs`
- [ ] `cargo test core::integrations::langfuse::types --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the Langfuse type unit tests back into `src/core/integrations/langfuse/types.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
