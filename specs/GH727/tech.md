# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@2a20ad60fd7d`, 32 Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/utils/data/utils/tests.rs` at 931 lines. It is a pure
test suite for an already-split `DataUtils` production module.

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
| P1 | Test suites | DataUtils tests, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: DataUtils Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test root | `src/utils/data/utils/tests.rs` | Contains all DataUtils tests inline. | It is 931 lines and mixes unrelated behavior domains. |
| Production facade | `src/utils/data/utils/mod.rs` | Exposes `DataUtils` and mounts `#[cfg(test)] mod tests;`. | The mount path should remain unchanged. |
| Production helpers | `base64_ops.rs`, `json_ops.rs`, `serialization.rs`, `string_ops.rs`, `uuid_ops.rs` | Already split by utility responsibility. | No production split is needed in this tranche. |

### Design

1. Keep `src/utils/data/utils/tests.rs` as the test root with child module declarations only.
2. Add `src/utils/data/utils/tests/base64_tests.rs` for base64 encode/decode and detection cases.
3. Add `json_conversion_tests.rs` for object/list conversion and `jsonify_tools`.
4. Add `json_cleanup_tests.rs` for shallow/deep cleanup of null values.
5. Add `uuid_tests.rs` for UUID and short-id coverage.
6. Add `json_merge_nested_tests.rs` for merge, nested extract, and nested set coverage.
7. Add `json_flatten_schema_tests.rs` for flattening and schema validation coverage.
8. Add `string_tests.rs` and `string_json_extraction_tests.rs` for string utilities, URL extraction, and JSON extraction.
9. Add `serialization_tests.rs` for pretty/compact/hash/size/clone coverage.
10. Do not edit production utility modules.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `tests.rs` | Root declares child modules only. |
| P2 | `tests/*.rs` | Original test functions remain discoverable by name. |
| P3 | file size | `wc -l src/utils/data/utils/tests.rs src/utils/data/utils/tests/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test utils::data::utils::tests --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/utils/data/utils/tests.rs`. |

## Risks

- Test module discovery can change when moving from a file module into child modules; the focused test command must prove discovery.
- Child modules need local imports to avoid relying on a broad parent prelude and to keep clippy/lint output clean.
- The split must not alter sample values, including Unicode and JSON edge cases.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test utils::data::utils::tests --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/utils/data/utils/tests.rs` and `src/utils/data/utils/tests/*.rs`

## Rollback

Revert the DataUtils test-suite split and `specs/GH727` edits. No production code,
schema changes, or runtime behavior changes are involved.
