# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@63b6bf4e29bb`, 41 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: Vertex AI Client

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Vertex AI client | `src/core/providers/vertex_ai/client.rs` | Contains error mapping, provider construction, URL building, HTTP request execution, chat/embedding/image operations, OpenAI param mapping, response transformation, health check, and inline tests. | The file is 1456 lines and mixes independent responsibilities. |
| Vertex AI module tree | `src/core/providers/vertex_ai/` | Already has focused modules for auth, models, transformers, embeddings, count tokens, files, image generation, text-to-speech, and other operations. | Client child modules should fit this existing decomposition instead of changing public provider exports. |
| Existing test split pattern | `src/core/providers/vertex_ai/embeddings/mod.rs`, `src/core/providers/vertex_ai/mod.rs` | Uses path-backed test modules for large Vertex AI tests. | `client.rs` can use the same pattern with `#[path = "client_tests.rs"] mod tests;`. |

### Design

1. Keep `src/core/providers/vertex_ai/client.rs` as the public `VertexAIProvider` module.
2. Move `VertexAIErrorMapper` into `src/core/providers/vertex_ai/client/error_mapper.rs` and import it from `client.rs`.
3. Move URL construction helpers into `src/core/providers/vertex_ai/client/url.rs`:
   - `build_url`
   - `get_publisher_for_model`
4. Move `check_health` into `src/core/providers/vertex_ai/client/health.rs` as a `pub(super)` inherent method.
5. Move the inline `#[cfg(test)] mod tests` body into `src/core/providers/vertex_ai/client_tests.rs` and keep `client.rs` declaring `#[path = "client_tests.rs"] mod tests;`.
6. Do not change `VertexAIProvider` fields, trait methods, request/response transforms, URL strings, or error mapping match arms.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `client.rs` module declarations and `client_tests.rs` | `cargo test core::providers::vertex_ai::client --lib --all-features` discovers moved tests. |
| P2 | `client/error_mapper.rs` | Existing error mapper tests pass with unchanged assertions. |
| P3 | `client/url.rs` and `client/health.rs` | Focused client tests and all-features check compile the moved inherent methods. |
| P4 | file size | `wc -l src/core/providers/vertex_ai/client.rs src/core/providers/vertex_ai/client/*.rs src/core/providers/vertex_ai/client_tests.rs` shows all touched files below 800. |
| P5 | public surface | `git diff -- src/core/providers/vertex_ai/mod.rs` is empty. |

## Risks

- Inherent methods defined in child modules must use `pub(super)` when called from the parent `client.rs`.
- `VertexAIErrorMapper` must remain visible to `get_error_mapper` and moved tests without becoming a new public API commitment.
- Moving URL helpers must not change custom API base, global Imagen, partner publisher, custom endpoint, or streaming `alt=sse` behavior.
- This tranche reduces one of the remaining 41 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::providers::vertex_ai::client --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/providers/vertex_ai/client.rs`, `client/*.rs`, and `client_tests.rs`

## Rollback

Revert the Vertex AI client module split and `specs/GH727` edits. No migrations,
runtime config changes, or public API changes are involved.
