# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@a26d350c`, 21 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/providers/v0/mod.rs`
at 852 lines. It is a V0 provider module where production definitions are below
the ceiling and the oversized portion is the inline unit test suite.

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

## Current Tranche: V0 Provider Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Provider definitions | `src/core/providers/v0/mod.rs` | Defines V0 config/model/provider, OpenAI-compatible parameter mapping, request/response transform, health check, cost calculation, and provider metadata. | Production surface must keep the same names, trait implementations, helper behavior, and module path. |
| Inline tests | `src/core/providers/v0/mod.rs` | Contains config, ProviderConfig, model parsing, provider helper, metadata, cost, and parameter mapping tests inline. | The test suite is what pushes the file over 800 lines. |
| Extracted tests | `src/core/providers/v0/tests.rs` | New path-backed test module loaded from `mod.rs`. | Removes the oversized inline test block without changing runtime architecture. |

### Design

1. Keep `src/core/providers/v0/mod.rs` as the production V0 provider module.
2. Preserve all public struct, enum, helper function, and trait implementation definitions in place,
   including fields, derives, defaults, request/response transforms, cost calculation, and error mapper selection.
3. Add only a `#[cfg(test)] #[path = "tests.rs"] mod tests;` declaration at the end of `mod.rs`.
4. Move the original inline tests into `src/core/providers/v0/tests.rs`.
5. Keep the test module name as `core::providers::v0::tests` so focused test filters and historical paths continue to work.
6. Do not edit `src/core/providers/v0/chat.rs` or any shared provider/runtime module.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/providers/v0/mod.rs` | Public V0 provider definitions stay in the original module path. |
| P2 | `src/core/providers/v0/mod.rs` | Root delegates tests with `#[path = "tests.rs"] mod tests;`. |
| P3 | `src/core/providers/v0/tests.rs` | Original inline V0 provider tests move without assertion changes. |
| P4 | file size | `wc -l src/core/providers/v0/mod.rs src/core/providers/v0/tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::providers::v0 --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/providers/v0/mod.rs`. |

## Risks

- Test extraction must not rename the module to a path that changes focused filters from `core::providers::v0::tests`.
- The extracted test file relies on `use super::*` to keep access to private helper methods like `get_endpoint` and `create_headers`.
- Production V0 provider definitions should not be split in this tranche because they are already below 800 lines.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/providers/v0/mod.rs src/core/providers/v0/tests.rs`
- [ ] `cargo test core::providers::v0 --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the V0 provider tests back into `src/core/providers/v0/mod.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
