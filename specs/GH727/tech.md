# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@a42b908a3e26`, 37 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Unified Provider Error Facade

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Unified provider facade | `src/core/providers/unified_provider.rs` | Defines `ProviderError`, its methods, shared HTTP mappers, and exported helper macros. | The file is 1014 lines and mixes error schema, behavior, mapper helpers, and macro definitions. |
| Provider module exports | `src/core/providers/mod.rs` | Re-exports `ProviderError` from `unified_provider`. | Root `unified_provider.rs` must preserve the canonical path for broad provider callers. |
| Error tests | `src/core/providers/unified_provider_tests.rs` | Covers factories, retry behavior, HTTP status mapping, contextual errors, display, clone, and conversions. | This is the focused verification surface for behavior-preserving decomposition. |

### Design

1. Keep `src/core/providers/unified_provider.rs` as the root documentation facade.
2. Move the `ProviderError` enum into sibling file `src/core/providers/unified_provider_error.rs`
   and load it through a private `#[path = "unified_provider_error.rs"] mod error;` child module.
3. Move the `impl ProviderError` block into sibling file `unified_provider_methods.rs`, importing the root facade's
   `ContextualError` and `ProviderError` so method behavior remains attached to the canonical type.
4. Move `default_http_error_mapper`, `parse_error_message_from_body`, and
   `extended_http_error_mapper` into sibling file `unified_provider_http_mapping.rs`, then re-export those helpers from the root.
5. Move `define_provider_error_helpers!`, `impl_provider_error_helpers!`,
   `define_standard_error_mapper!`, and `define_extended_error_mapper!` into sibling file `unified_provider_macros.rs`.
6. Do not add a top-level `src/core/providers/unified_provider/` directory, because provider lifecycle
   coverage treats every providers-root directory as a provider module.
7. Do not edit provider implementations, `provider_error_conversions.rs`, or provider dispatch/factory code.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | root `unified_provider.rs` | Re-exports `ProviderError` and mapper helpers from child modules. |
| P2 | `error.rs` and `methods.rs` | `cargo test core::providers::unified_provider_tests --lib --all-features` preserves factories, retry/status, display, and context behavior. |
| P3 | `http_mapping.rs` | All-features check proves downstream mapper macro/caller imports still compile. |
| P4 | `macros.rs` | All-features check proves exported macro definitions remain available to provider modules. |
| P5 | file size | `wc -l src/core/providers/unified_provider.rs src/core/providers/unified_provider_*.rs` shows all touched files below 800. |
| P6 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/providers/unified_provider.rs`. |

## Risks

- Missing root re-exports would break the broad existing `core::providers::unified_provider::*` import surface.
- `#[macro_export]` macro definitions must still compile after moving to a child module.
- HTTP mapper helpers reference sibling provider utilities; path updates must preserve the same retry-after parsing.
- This tranche reduces one of the remaining 37 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::providers::unified_provider_tests --lib --all-features`
- [ ] `cargo test lifecycle_covers_every_provider_directory --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/providers/unified_provider.rs` and sibling child files

## Rollback

Revert the unified provider facade split and `specs/GH727` edits. No migrations or
provider runtime behavior changes are involved.
