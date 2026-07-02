# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@f98985e1`, 7 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/core/providers/anthropic/client/tests.rs`
at 812 lines. It is a test-only Anthropic client suite covering client creation,
headers, error mapping, retry-after parsing, message/tool transforms, response
transforms, and request edge behavior.

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

## Current Tranche: Anthropic Client Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test facade | `src/core/providers/anthropic/client/tests.rs` | Currently contains all legacy Anthropic client tests directly. | This file can become a small facade while preserving the `client.rs` `mod tests;` entrypoint. |
| Extracted child tests | `src/core/providers/anthropic/client/tests/*.rs` | New behavior-domain test modules under the existing test facade. | Splitting by behavior domain reduces file size without changing production code. |
| Existing siblings | `request_tests.rs`, `compatible_tests.rs` | Existing focused sibling test modules declared directly from `client.rs`. | They remain untouched to keep ownership boundaries clear. |

### Design

1. Keep `src/core/providers/anthropic/client/tests.rs` as the test facade with the original shared imports and helper scope.
2. Split the original tests into child modules under `src/core/providers/anthropic/client/tests/`:
   - `setup_error_tests.rs` for client creation, headers, HTTP error mapping, and retry-after parsing.
   - `message_tool_tests.rs` for system message separation, Anthropic message conversion, tool choice, and tool transforms.
   - `response_tests.rs` for chat response conversion, usage/cache details, thinking blocks, tool use, and finish reasons.
   - `request_edge_tests.rs` for unsupported `n`, ignored unsupported params, configured unknown-model behavior, and default unknown-model rejection.
3. Each child module uses `use super::*;` to retain the same test-module access to Anthropic client internals.
4. Move tests without assertion, fixture model, fixture header, JSON expected-value, or error-message expectation changes.
5. Do not edit production `client.rs`, `request.rs`, `response.rs`, `usage.rs`, `config.rs`, registry, provider, `request_tests.rs`, or `compatible_tests.rs`.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/core/providers/anthropic/client/tests.rs` | Root test facade keeps shared imports and delegates to child modules. |
| P2 | `src/core/providers/anthropic/client/tests/*.rs` | Original test names and assertions remain present under behavior-domain modules. |
| P3 | Anthropic client behavior | No client creation, header, error mapping, retry-after, message/tool transform, response transform, cache accounting, or request edge behavior changes. |
| P4 | file size | `wc -l src/core/providers/anthropic/client/tests.rs src/core/providers/anthropic/client/tests/*.rs` shows every touched file below 800. |
| P5 | focused test suite | `cargo test core::providers::anthropic::client::tests --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/providers/anthropic/client/tests.rs`. |

## Risks

- Splitting a test-only file changes test module paths below `client::tests`, so focused filtering should use `core::providers::anthropic::client::tests`.
- Child modules must remain under the `tests.rs` facade so they keep access to the same private Anthropic client helpers through `super::*`.
- Anthropic request/response conversion is provider-critical; this tranche must not modify production transform code or weaken assertions.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/providers/anthropic/client/tests.rs src/core/providers/anthropic/client/tests/*.rs`
- [ ] `cargo test core::providers::anthropic::client::tests --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the Anthropic client test modules back into `src/core/providers/anthropic/client/tests.rs`
and revert the `specs/GH727` edits. No schema, persistence, or runtime behavior
changes are involved.
