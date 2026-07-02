# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@bcdf7245`, 10 tracked Rust files remain over the U-16
800-line ceiling. One current largest file is `src/core/providers/gemini/provider.rs`
at 821 lines. It is a Gemini provider module where production provider,
validation, trait implementation, health, and unsupported-feature behavior end
at line 398 and the oversized portion is inline unit tests starting at line 399.

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

## Current Tranche: Gemini Provider Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Gemini provider production | `src/core/providers/gemini/provider.rs` | Defines `GeminiProvider`, constructor, validation, `LLMProvider` methods, health, cost, and unsupported feature behavior. | Public provider path and trait behavior must remain unchanged for callers. |
| Inline tests | `src/core/providers/gemini/provider.rs` | `#[cfg(test)] mod tests` starts at line 399 and contains provider creation, capability, model, validation, param mapping, cost, and unsupported feature tests. | Moving these tests removes the U-16 violation without changing provider runtime code. |
| Extracted tests | `src/core/providers/gemini/provider_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and fixtures remain centralized against the same production module. |

### Design

1. Keep `src/core/providers/gemini/provider.rs` as the production owner for `GeminiProvider`, request validation, trait methods, health check, cost helper, and unsupported feature behavior.
2. Replace the inline test module with `#[cfg(test)] #[path = "provider_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/core/providers/gemini/provider_tests.rs` without assertion, fixture API key, model name, validation range, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create a Gemini provider runtime helper tree in this tranche because the production implementation is already below the ceiling.
6. Do not edit Gemini client, config, error mapping, model registry, streaming, factory, or API behavior beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/providers/gemini/provider.rs` | Root keeps production Gemini provider implementation and delegates tests to `provider_tests.rs`. |
| P2 | `src/core/providers/gemini/provider_tests.rs` | Original test names and assertions remain present. |
| P3 | Gemini provider API | No constructor, trait method, validation, param mapping, unsupported feature, error mapper, or cost behavior changes. |
| P4 | file size | `wc -l src/core/providers/gemini/provider.rs src/core/providers/gemini/provider_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test core::providers::gemini::provider --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/providers/gemini/provider.rs`. |

## Risks

- Extracting tests changes the exact test source file while preserving the module path as `provider::tests`, so the focused module filter remains `core::providers::gemini::provider`.
- Tests call private `validate_request`, so the extracted path-backed module must remain a child module of `provider.rs` rather than a sibling declared from `gemini/mod.rs`.
- Gemini provider runtime is customer-facing provider behavior, so this tranche must not alter client calls, error mapping, model registry lookup, streaming, or unsupported feature semantics.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/providers/gemini/provider.rs src/core/providers/gemini/provider_tests.rs`
- [ ] `cargo test core::providers::gemini::provider --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the Gemini provider unit tests back into `src/core/providers/gemini/provider.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
