# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@ea1a47c3286b`, 26 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/monitoring/types.rs` at 867
lines. It is a public monitoring type module where the production definitions
are already compact and the oversized portion is the inline unit test suite.

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

## Current Tranche: Monitoring Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Type definitions | `src/monitoring/types.rs` | Defines public monitoring metric and alert DTOs. | Production surface must keep the same names, fields, derives, serde behavior, and module path. |
| Inline tests | `src/monitoring/types.rs` | Contains the monitoring type serialization/display/clone test suite inline. | The test suite is what pushes the file over 800 lines. |
| Extracted tests | `src/monitoring/types_tests.rs` | New path-backed test module loaded from `types.rs`. | Removes the oversized inline test block without changing runtime architecture. |

### Design

1. Keep `src/monitoring/types.rs` as the production monitoring type definition file.
2. Preserve all public struct and enum definitions in place, including fields, derives,
   serde attributes, and the `AlertSeverity` `Display` implementation.
3. Add only a `#[cfg(test)] #[path = "types_tests.rs"] mod tests;` declaration at the end
   of `types.rs`.
4. Move the original inline tests into `src/monitoring/types_tests.rs`, adding the imports
   they previously inherited from the parent module.
5. Keep the test module name as `monitoring::types::tests` so focused test filters and
   historical paths continue to work.
6. Do not edit monitoring runtime modules, metric collectors, alert processors, or
   observability integration code.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/monitoring/types.rs` | Public monitoring type definitions stay in the original module path. |
| P2 | `src/monitoring/types.rs` | Root delegates tests with `#[path = "types_tests.rs"] mod tests;`. |
| P3 | `src/monitoring/types_tests.rs` | Original inline monitoring type tests move without assertion changes. |
| P4 | file size | `wc -l src/monitoring/types.rs src/monitoring/types_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test monitoring::types --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/monitoring/types.rs`. |

## Risks

- Test extraction must not rename the module to a path that changes focused filters from
  `monitoring::types::tests`.
- The test file needs explicit imports for `Utc` and `HashMap` that were previously in
  scope only because the tests were inline.
- Production type definitions should not be split in this tranche because they are already
  compact; splitting them now would add compatibility risk without reducing meaningful
  complexity.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/monitoring/types.rs src/monitoring/types_tests.rs`
- [ ] `cargo test monitoring::types --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the monitoring type tests back into `src/monitoring/types.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes are
involved.
