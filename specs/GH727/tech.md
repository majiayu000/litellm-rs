# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@8078679f`, 18 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/utils/net/client/utils.rs`
at 846 lines. It is a shared HTTP client helper module where production
ClientUtils definitions are below the ceiling and the oversized portion is the
inline unit test suite.

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

## Current Tranche: Net Client Utils Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Net client helper definitions | `src/utils/net/client/utils.rs` | Defines ClientUtils helper methods for HTTP client creation, proxy discovery, retry/backoff, provider defaults, URL validation, retry-after parsing, provider headers, connection testing, and content-type parsing. | Production helper surface must keep the same names, signatures, error messages, provider defaults, and module path. |
| Inline tests | `src/utils/net/client/utils.rs` | Contains retry, delay, provider timeout, httpx timeout, user agent, path, URL, content-type, default header, retry-after, HTTP client creation, provider client, and proxy smoke tests inline. | The test suite is what pushes the file over 800 lines. |
| Extracted tests | `src/utils/net/client/utils_tests.rs` | New path-backed test module loaded from `utils.rs`. | Removes the oversized inline test block without changing production architecture. |

### Design

1. Keep `src/utils/net/client/utils.rs` as the production HTTP client helper module.
2. Preserve all public helper definitions in place, including method signatures, provider defaults,
   retry/backoff logic, URL validation, retry-after parsing, header maps, proxy discovery, and error strings.
3. Add only a `#[cfg(test)] #[path = "utils_tests.rs"] mod tests;` declaration at the end of `utils.rs`.
4. Move the original inline tests into `src/utils/net/client/utils_tests.rs`.
5. Keep the test module name as `utils::net::client::utils::tests` so focused test filters and historical paths continue to work.
6. Do not introduce a production facade or split the net client helper surface in this tranche.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/utils/net/client/utils.rs` | Public net client helper definitions stay in the original module path. |
| P2 | `src/utils/net/client/utils.rs` | Root delegates tests with `#[path = "utils_tests.rs"] mod tests;`. |
| P3 | `src/utils/net/client/utils_tests.rs` | Original inline net client helper tests move without assertion changes. |
| P4 | file size | `wc -l src/utils/net/client/utils.rs src/utils/net/client/utils_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test utils::net::client::utils --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/utils/net/client/utils.rs`. |

## Risks

- Test extraction must not rename the module to a path that changes focused filters from `utils::net::client::utils::tests`.
- The extracted test file relies on `use super::*` to keep access to ClientUtils plus `HttpClientConfig` and `RetryConfig` imports already visible in the parent module.
- The proxy smoke test reads process env; this tranche must preserve its no-panic behavior rather than asserting machine-specific proxy state.
- Production net client helper definitions should not be split in this tranche because they are already below 800 lines.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/utils/net/client/utils.rs src/utils/net/client/utils_tests.rs`
- [ ] `cargo test utils::net::client::utils --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the net client helper tests back into `src/utils/net/client/utils.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
