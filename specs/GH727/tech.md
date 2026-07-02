# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@ee283a3cb683`, 35 Rust files remain over the U-16 800-line ceiling.
The highest-risk files are not all equivalent: some are pure test suites, some are public
type facades, and some are runtime orchestrators. They need different split patterns.

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
| P1 | Test suites | cost calculator, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, security, cost, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | Vertex AI client, unified provider, teams route, SeaORM team repository, OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: Teams Route Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Teams route module | `src/server/routes/teams.rs` | Defines teams HTTP DTOs, auth helpers, handlers, route registration, and inline tests. | The file is 934 lines and mixes runtime route code with focused route/helper tests. |
| Route registration | `src/server/http.rs`, `src/server/routes/mod.rs` | Calls `routes::teams::configure_routes` and exports the module. | The tranche must preserve route module names and route wiring. |
| Teams route tests | inline `#[cfg(test)] mod tests` | Covers DTO serde, invitation resolution, request caller resolution, and team access checks. | Moving tests is enough to bring production route code below U-16 without handler churn. |

### Design

1. Keep teams route DTOs, auth helpers, handlers, and `configure_routes` in `src/server/routes/teams.rs`.
2. Replace the inline test module with `#[cfg(test)] #[path = "teams_tests.rs"] mod tests;`.
3. Move the original test module body to `src/server/routes/teams_tests.rs`.
4. Do not split production route handlers in this tranche, because test extraction alone brings the route module below U-16 and avoids route wiring churn.
5. Do not edit `src/server/http.rs`, `src/server/routes/mod.rs`, team manager, or team repository modules.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `teams.rs` | Keeps handler names, DTOs, auth helpers, and route registration unchanged. |
| P2 | `teams_tests.rs` | `cargo test server::routes::teams --lib --all-features` runs the moved tests. |
| P3 | file size | `wc -l src/server/routes/teams.rs src/server/routes/teams_tests.rs` shows both files below 800. |
| P4 | queue count | tracked-file scan shows the remaining queue no longer includes `src/server/routes/teams.rs`. |

## Risks

- Moving tests one module deeper can break access to private auth helper functions; focused tests must prove the path-backed module still sees parent private items.
- Splitting runtime handlers now would increase review scope and route wiring risk without being required for U-16.
- This tranche reduces one of the remaining 35 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test server::routes::teams --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/server/routes/teams.rs` and `src/server/routes/teams_tests.rs`

## Rollback

Revert the teams route test extraction and `specs/GH727` edits. No migrations or
runtime behavior changes are involved.
