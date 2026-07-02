# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@68a17074`, 6 tracked Rust files remain over the U-16
800-line ceiling. One current largest file is `tests/moderations_routes.rs`
at 809 lines. It is a gated moderation route integration-test suite covering
mock upstream capture, provider selection, auth/validation, budget rejection,
fallback routing, wildcard/default-model behavior, and upstream proxy assertions.

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

## Current Tranche: Moderation Routes Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test facade | `tests/moderations_routes.rs` | Currently contains the feature gate, shared mock upstream/app-state helpers, and all moderation route behavior tests directly. | This file can keep shared integration-test setup while delegating behavior tests. |
| Extracted child tests | `tests/tests/moderations_routes_*.rs` | New behavior-domain test modules under the existing gated test facade. | Splitting by route behavior reduces file size without changing runtime code. |
| Runtime routes | `src/server/routes/**`, provider router, auth, budget state | Production moderation route behavior. | Must not be edited in this tranche. |

### Design

1. Keep `tests/moderations_routes.rs` as the gated integration-test facade with the original imports, mock upstream structs, server lifecycle helpers, app-state builders, provider factory helpers, and auth fixture helper.
2. Split the original tests into child modules under `tests/tests/` with `moderations_routes_` filename prefixes:
   - `moderations_routes_proxy_selection_tests.rs` for fail-closed/no-provider, upstream proxy/header capture, root alias, and default-model provider selection.
   - `moderations_routes_auth_validation_tests.rs` for anonymous/authenticated API key behavior plus invalid request and unconfigured-model rejection.
   - `moderations_routes_budget_fallback_tests.rs` for provider/model budget rejection, router budget fallback, native OpenAI default-model fallback, provider-name wildcard fallback, and wildcard provider fallback.
3. Each child module uses `use super::*;` to retain the same access to shared mock/app/provider helpers.
4. Move tests without assertion, fixture provider/model/header, JSON body, route URI, status code, or error-message expectation changes.
5. Do not edit production route, router, auth, budget, storage, provider, or upstream client code.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `tests/moderations_routes.rs` | Root gated test facade keeps shared mock/app/provider helpers and delegates route tests to child modules. |
| P2 | `tests/tests/moderations_routes_*.rs` | Original test names and assertions remain present under behavior-domain modules. |
| P3 | moderation route behavior | No route URI, auth, validation, upstream proxy/header/body capture, budget rejection, fallback, wildcard, or default-model behavior changes. |
| P4 | file size | `wc -l tests/moderations_routes.rs tests/tests/moderations_routes_*.rs` shows every touched file below 800. |
| P5 | focused test suite | `cargo test --test moderations_routes --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `tests/moderations_routes.rs`. |

## Risks

- Splitting a gated integration-test file changes internal module paths, so the focused command should target the integration test crate: `cargo test --test moderations_routes --all-features`.
- Child modules must remain under the gated parent `mod tests` so they share mock upstream and app-state helpers through `super::*`.
- Moderation routing is externally visible API behavior; this tranche must not modify production routes or weaken status/header/body/error assertions.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l tests/moderations_routes.rs tests/tests/moderations_routes_*.rs`
- [ ] `cargo test --test moderations_routes --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the moderation route test modules back into `tests/moderations_routes.rs`
and revert the `specs/GH727` edits. No schema, persistence, or runtime behavior
changes are involved.
