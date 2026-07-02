# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@f91e3cbd4feb`, 20 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is
`src/core/models/metrics/aggregates.rs` at 850 lines. It is a metrics aggregate
type module where production DTO definitions are below the ceiling and the
oversized portion is the inline unit test suite.

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

## Current Tranche: Metrics Aggregates Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Metrics aggregate definitions | `src/core/models/metrics/aggregates.rs` | Defines provider, model, system, usage, daily, endpoint, alert condition, and alert config aggregate DTOs. | Production type surface must keep the same names, fields, derives, serde attributes, and module path. |
| Inline tests | `src/core/models/metrics/aggregates.rs` | Contains DTO structure, clone, serialization, deserialization, default, and breakdown tests inline. | The test suite is what pushes the file over 800 lines. |
| Extracted tests | `src/core/models/metrics/aggregates_tests.rs` | New path-backed test module loaded from `aggregates.rs`. | Removes the oversized inline test block without changing production architecture. |

### Design

1. Keep `src/core/models/metrics/aggregates.rs` as the production metrics aggregate model module.
2. Preserve all public struct and enum definitions in place, including fields, derives, serde attributes,
   metadata flattening, chrono field types, Uuid fields, and enum variants.
3. Add only a `#[cfg(test)] #[path = "aggregates_tests.rs"] mod tests;` declaration at the end of `aggregates.rs`.
4. Move the original inline tests into `src/core/models/metrics/aggregates_tests.rs`.
5. Keep the test module name as `core::models::metrics::aggregates::tests` so focused test filters and historical paths continue to work.
6. Do not introduce a production facade or split the metrics aggregate DTO surface in this tranche.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/models/metrics/aggregates.rs` | Public metrics aggregate DTO definitions stay in the original module path. |
| P2 | `src/core/models/metrics/aggregates.rs` | Root delegates tests with `#[path = "aggregates_tests.rs"] mod tests;`. |
| P3 | `src/core/models/metrics/aggregates_tests.rs` | Original inline metrics aggregate tests move without assertion changes. |
| P4 | file size | `wc -l src/core/models/metrics/aggregates.rs src/core/models/metrics/aggregates_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::models::metrics::aggregates --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/models/metrics/aggregates.rs`. |

## Risks

- Test extraction must not rename the module to a path that changes focused filters from `core::models::metrics::aggregates::tests`.
- The extracted test file relies on `use super::*` to keep access to the production DTO definitions and imported `Metadata` path.
- Production metrics aggregate definitions should not be split in this tranche because they are already below 800 lines.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/models/metrics/aggregates.rs src/core/models/metrics/aggregates_tests.rs`
- [ ] `cargo test core::models::metrics::aggregates --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the metrics aggregate tests back into `src/core/models/metrics/aggregates.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
