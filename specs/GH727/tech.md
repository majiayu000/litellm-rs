# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@d7b1d26c0797`, 36 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Security Types Test Extraction

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Security type module | `src/core/security/types.rs` | Defines security DTOs/enums and contains a large inline test module. | The file is 948 lines, but production definitions are only about 220 lines. |
| Security module exports | `src/core/security/mod.rs` | Re-exports public names from `types`. | Production type paths and top-level re-exports must remain unchanged. |
| Security type tests | inline `#[cfg(test)] mod tests` | Covers PII patterns, moderation types/actions/severity, filters, GDPR/export/consent/anonymization DTOs. | This is a test-suite extraction tranche; assertions should move without behavior changes. |

### Design

1. Keep all production security types in `src/core/security/types.rs`.
2. Replace the inline test module with `#[cfg(test)] #[path = "types_tests.rs"] mod tests;`.
3. Move the original test module body to `src/core/security/types_tests.rs`.
4. Do not add a `types/` production facade directory in this tranche, because the production type block is already below U-16 after test extraction.
5. Do not edit `src/core/security/mod.rs`, filter runtime code, GDPR runtime code, patterns, or profanity filtering.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `types.rs` | Keeps production type definitions and top-level security re-exports unchanged. |
| P2 | `types_tests.rs` | `cargo test core::security::types --lib --all-features` runs the moved tests. |
| P3 | file size | `wc -l src/core/security/types.rs src/core/security/types_tests.rs` shows both files below 800. |
| P4 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/security/types.rs`. |

## Risks

- Moving tests one module deeper can break access to parent imports; focused tests must prove the path-backed module still sees `Regex`, `HashMap`, and security types.
- A production `types/` facade split would add churn without reducing risk in this tranche; avoid it unless compilation requires it.
- This tranche reduces one of the remaining 36 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::security::types --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/security/types.rs` and `src/core/security/types_tests.rs`

## Rollback

Revert the security types test extraction and `specs/GH727` edits. No migrations or
runtime behavior changes are involved.
