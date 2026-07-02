# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@5a4c42c15b86`, 31 tracked Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/core/integrations/observability/opentelemetry.rs` at
921 lines. It combines configuration DTOs, span data types, OTLP export helpers,
`OpenTelemetryIntegration`, and its unit tests in one module.

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
| P1 | Test suites | DataUtils tests, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: OpenTelemetry Runtime Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Facade | `src/core/integrations/observability/opentelemetry.rs` | Exposes config, integration, span/event/status/kind, and attribute value names. | Public import paths must remain compatible. |
| Config | `opentelemetry/config.rs` | Owns serde defaults and `OpenTelemetryConfig::default`. | Config defaults are a stable API surface. |
| Span model | `opentelemetry/span.rs` | Owns span/event/status/kind/attribute values and ID generation. | Span lifecycle and tests depend on these helpers. |
| Exporter | `opentelemetry/exporter.rs` | Owns OTLP payload mapping and HTTP export. | Export semantics and error propagation must not silently degrade. |
| Integration | `opentelemetry/integration_impl.rs` | Owns active span state, pending span batch, and `Integration` trait implementation. | Runtime state transitions need to remain local and testable. |
| Tests | `opentelemetry/tests.rs` | Contains the moved OpenTelemetry unit tests. | Focused tests should remain under the original module path. |

### Design

1. Keep `opentelemetry.rs` as a root facade with only child module declarations and `pub use` exports.
2. Move `OpenTelemetryConfig` and its default helper functions into `config.rs` without changing serde attributes or defaults.
3. Move `SpanStatus`, `SpanKind`, `Span`, `SpanEvent`, `AttributeValue`, conversions, and ID generation into `span.rs`.
4. Move `export_spans` and `build_otlp_payload` into `exporter.rs`; keep them module-internal because only the integration and tests need them.
5. Move `ActiveSpan`, `SpanBatch`, `OpenTelemetryIntegration`, and `impl Integration for OpenTelemetryIntegration` into `integration_impl.rs`.
6. Move the existing unit tests into `tests.rs` and import internal helpers through sibling modules only for test coverage.
7. Do not change endpoint defaults, payload field names, span attributes, sampling, batch flush conditions, or shutdown behavior.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `opentelemetry.rs` | Root facade declares focused modules and re-exports the original public names. |
| P2 | `config.rs`, `span.rs`, `exporter.rs`, `integration_impl.rs` | Responsibilities are separated without changing public DTO fields or trait method signatures. |
| P3 | file size | `wc -l src/core/integrations/observability/opentelemetry.rs src/core/integrations/observability/opentelemetry/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test core::integrations::observability::opentelemetry --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/integrations/observability/opentelemetry.rs`. |

## Risks

- `#[async_trait]` and derive attributes must stay attached to the moved impl/types; losing them changes compilation and trait compatibility.
- Internal exporter/span helpers need narrow visibility for sibling tests without expanding the public API.
- Export failures must continue returning errors from `flush`; background batch export may still log failures as before.
- Root facade re-exports must keep downstream imports compiling.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::integrations::observability::opentelemetry --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/core/integrations/observability/opentelemetry.rs` and `src/core/integrations/observability/opentelemetry/*.rs`

## Rollback

Revert the OpenTelemetry module split and `specs/GH727` edits. No schema changes or
intended runtime behavior changes are involved.
