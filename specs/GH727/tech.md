# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@e98c0357`, 17 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is
`tests/integration/auth_middleware_tests.rs` at 844 lines. It is a test-only
auth middleware integration suite where shared fixtures/helpers are below the
ceiling and the oversized portion is behavior tests.

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

## Current Tranche: Auth Middleware Integration Test Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Suite entry | `tests/integration/auth_middleware_tests.rs` | Current single file declares a `#[cfg(test)] mod tests` with imports, shared fixtures, and all behavior tests. | The entry point should stay discoverable from `tests/integration/mod.rs` while delegating the large suite. |
| Shared fixtures | `tests/integration/auth_middleware_tests_parts/mod.rs` | New suite module keeps imports, seeded principal setup, auth probe route, state builders, and helper functions. | Child modules need these helpers through `super::*` without duplicating setup. |
| Rejected/rate-limit tests | `tests/integration/auth_middleware_tests_parts/rejection_rate_limit.rs` | Missing/invalid auth, gateway rate limit, requests_per_minute alias, valid-auth reservation release, and API-key rpm behavior. | These tests share rate-limit setup and status-code expectations. |
| Authenticated permission/context tests | `tests/integration/auth_middleware_tests_parts/permissions_context.rs` | Valid auth context propagation, legacy permission, denied operation/endpoint policy, admin-owned key restriction, and budget id context. | These tests share seeded principal setup and auth probe assertions. |
| Disabled-auth tests | `tests/integration/auth_middleware_tests_parts/disabled_auth.rs` | Auth-disabled fail-closed and allow-anonymous context behavior. | These tests cover the disabled-auth mode boundary. |

### Design

1. Replace the oversized root file body with `#[cfg(test)] #[path = "auth_middleware_tests_parts/mod.rs"] mod tests;`.
2. Move shared imports, structs, constants, route handler, state builders, and seed helpers into `tests/integration/auth_middleware_tests_parts/mod.rs`.
3. Declare behavior child modules from `mod.rs`: `rejection_rate_limit`, `permissions_context`, and `disabled_auth`.
4. Move original tests into the child modules without changing assertions, request setup, seeded metadata, route paths, peer addresses, or expected status codes.
5. Use `use super::*;` in child modules so helper ownership remains centralized in `mod.rs`.
6. Do not edit auth/rate-limit/storage/server production code in this tranche.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `tests/integration/auth_middleware_tests.rs` | Root delegates to `auth_middleware_tests_parts/mod.rs`. |
| P2 | `tests/integration/auth_middleware_tests_parts/mod.rs` | Shared fixtures/helpers remain centralized and child modules are declared. |
| P3 | `tests/integration/auth_middleware_tests_parts/*.rs` | Original behavior tests move by domain without assertion changes. |
| P4 | file size | `wc -l tests/integration/auth_middleware_tests.rs tests/integration/auth_middleware_tests_parts/*.rs` shows all files below 800. |
| P5 | focused test suite | `cargo test --all-features auth_middleware_tests` runs the moved tests from `tests/lib.rs`. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `tests/integration/auth_middleware_tests.rs`. |

## Risks

- Child modules change the exact per-test module path, but the focused suite filter `integration::auth_middleware_tests` remains stable.
- Helper functions and imports must stay in `mod.rs`; duplicating AppState/seed setup across child modules would increase drift risk.
- Rate-limit tests depend on per-test state and fixed peer addresses; move them without reordering assertions or changing addresses.
- Disabled-auth tests must remain behavior-only and not weaken fail-closed assertions.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l tests/integration/auth_middleware_tests.rs tests/integration/auth_middleware_tests_parts/*.rs`
- [ ] `cargo test --all-features auth_middleware_tests`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the auth middleware integration tests back into `tests/integration/auth_middleware_tests.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
