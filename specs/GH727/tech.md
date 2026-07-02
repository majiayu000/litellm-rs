# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@464a1be4`, 15 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is `src/config/models/server.rs` at
839 lines. It is a server configuration model/helper module where production
definitions end at line 305 and the oversized portion is inline unit tests.

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

## Current Tranche: Server Config Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Server config models | `src/config/models/server.rs` | Defines `ServerConfig`, `TlsConfig`, `CorsConfig`, defaults, merge helpers, and validation logic. | Public module path must remain unchanged for config loading and server startup callers. |
| Inline tests | `src/config/models/server.rs` | `#[cfg(test)] mod tests` starts at line 306 and contains default, merge, validation, serde, and trusted proxy unit tests. | Moving these tests removes the U-16 violation without introducing facade churn. |
| Extracted tests | `src/config/models/server_tests.rs` | New path-backed test module keeps the original tests under `super::*`. | Assertions and JSON samples remain centralized against the same production module. |

### Design

1. Keep `src/config/models/server.rs` as the production owner for all current public server config types and helper functions.
2. Replace the inline test module with `#[cfg(test)] #[path = "server_tests.rs"] mod tests;`.
3. Move the original inline test body into `src/config/models/server_tests.rs` without assertion, fixture, JSON sample, or expected-value changes.
4. Keep `use super::*;` in the extracted test module so tests validate the same parent module API.
5. Do not create a `types/` facade tree in this tranche because the production definitions are already below the ceiling.
6. Do not edit config loading, HTTP startup, TLS, CORS, trusted proxy runtime behavior, or serialization attributes beyond the mechanical test move.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `src/config/models/server.rs` | Root keeps production model/helper definitions and delegates tests to `server_tests.rs`. |
| P2 | `src/config/models/server_tests.rs` | Original test names and assertions remain present. |
| P3 | server config public API | No public type, field, serde attribute, helper signature, merge rule, or validation error string changes. |
| P4 | file size | `wc -l src/config/models/server.rs src/config/models/server_tests.rs` shows both files below 800. |
| P5 | focused test suite | `cargo test config::models::server --lib --all-features` runs the moved tests. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/config/models/server.rs`. |

## Risks

- Extracting tests changes the exact test module path from inline `server::tests` to path-backed `server::tests`, but the focused module filter remains `config::models::server`.
- Public server config models are used by config loading and server startup, so this tranche must not change fields, serde annotations, defaults, merge behavior, or helper signatures.
- CORS credential validation and TLS file validation error strings must move unchanged because callers and tests rely on those semantics.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/config/models/server.rs src/config/models/server_tests.rs`
- [ ] `cargo test config::models::server --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the server config unit tests back into `src/config/models/server.rs` and revert
the `specs/GH727` edits. No schema, persistence, or runtime behavior changes
are involved.
