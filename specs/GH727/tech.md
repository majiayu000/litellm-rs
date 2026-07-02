# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@37c31ff5b2d2`, 34 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Cost Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Cost type module | `src/core/cost/types.rs` | Defines unified cost DTOs, tracker methods, result/error types, and inline tests. | The file is 934 lines, but production code is about 420 lines after test extraction. |
| Cost module exports | `src/core/cost/mod.rs` | Re-exports the public cost types from `types`. | Public type paths and top-level re-exports must remain unchanged. |
| Cost type tests | inline `#[cfg(test)] mod tests` | Covers usage conversion, pricing defaults/serde, breakdown totals, tracker summaries, result/error DTOs, and provider pricing. | Moving tests is enough to bring the public type module below U-16 without facade churn. |

### Design

1. Keep cost production types and impl blocks in `src/core/cost/types.rs`.
2. Replace the inline test module with `#[cfg(test)] #[path = "types_tests.rs"] mod tests;`.
3. Move the original test module body to `src/core/cost/types_tests.rs`.
4. Do not add a production `types/` facade split in this tranche, because test extraction alone brings the public type module below U-16.
5. Do not edit `src/core/cost/mod.rs`, cost calculator modules, provider cost modules, or pricing service modules.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `types.rs` | Keeps cost production types, impl blocks, and errors unchanged. |
| P2 | `types_tests.rs` | `cargo test core::cost::types --lib --all-features` runs the moved tests. |
| P3 | file size | `wc -l src/core/cost/types.rs src/core/cost/types_tests.rs` shows both files below 800. |
| P4 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/cost/types.rs`. |

## Risks

- Moving tests one module deeper can break access to private tracker fields or parent imports; focused tests must prove the path-backed module still sees parent private items.
- A production cost type facade split would add churn to a widely imported type module without being required for U-16.
- This tranche reduces one of the remaining 34 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::cost::types --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/cost/types.rs` and `src/core/cost/types_tests.rs`

## Rollback

Revert the cost types test extraction and `specs/GH727` edits. No migrations or
runtime behavior changes are involved.
