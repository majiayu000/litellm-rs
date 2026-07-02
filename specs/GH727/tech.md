# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@2d35b72e1031`, 27 tracked Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/utils/data/validation/request_validator.rs` at
868 lines. It is a runtime validator module that mixes chat request validation,
message/content-part validation, name/media helper validation, and inline tests
in one module.

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

## Current Tranche: Request Validator Runtime Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Facade root | `src/utils/data/validation/request_validator.rs` | Exposes `RequestValidator` today. | It must keep the original public import surface. |
| Chat validation | `src/utils/data/validation/request_validator/chat.rs` | New child module for public chat request validation and message/content helpers. | Keeps request flow and message-role/content semantics together. |
| Name validation | `src/utils/data/validation/request_validator/names.rs` | New child module for model and function name validation. | Isolates regex-backed scalar validation and internal regex errors. |
| Media validation | `src/utils/data/validation/request_validator/media.rs` | New child module for image URL/base64 and audio validation. | Keeps URL parsing, base64 decoding, and supported format checks together. |
| Tests | `src/utils/data/validation/request_validator/tests.rs` | New child test module. | Keeps existing focused request validator coverage under the same module path. |

### Design

1. Keep `src/utils/data/validation/request_validator.rs` as a facade with child module declarations and the original public `RequestValidator` type.
2. Move `validate_chat_completion_request`, message role checks, message content checks, and content-part checks to `chat.rs`.
3. Move `validate_model_name` and `validate_function_name` to `names.rs`, keeping regex patterns and error mapping unchanged.
4. Move `validate_image_url`, `validate_base64_image`, `validate_audio_data`, `validate_base64_payload`, and `validate_audio_format` to `media.rs`.
5. Widen moved helper methods only to `pub(super)` so sibling child modules and tests can call them without making them public crate API.
6. Move inline tests to `tests.rs` under the same parent module without changing assertions.
7. Do not edit request/response model definitions, route handlers, `api_validator.rs`, or `data_validator.rs`.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `request_validator.rs` | Root facade keeps the same public `RequestValidator` type and module path. |
| P2 | `request_validator/*.rs` | Runtime responsibilities are separated by chat/message, scalar names, media/base64/audio, and tests. |
| P3 | file size | `wc -l src/utils/data/validation/request_validator.rs src/utils/data/validation/request_validator/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test utils::data::validation::request_validator --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/utils/data/validation/request_validator.rs`. |

## Risks

- `RequestValidator` helper methods were private before; moved helper methods must stay `pub(super)`, not public API.
- `validate_chat_completion_request` must keep validation order so user-facing error precedence does not drift.
- Regex patterns and media/audio validation literals must not change.
- Moved tests must still compile against helper methods without exposing them outside the parent module.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test utils::data::validation::request_validator --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/utils/data/validation/request_validator.rs` and `src/utils/data/validation/request_validator/*.rs`

## Rollback

Revert the request validator module split and `specs/GH727` edits. No schema
changes or runtime behavior changes are involved.
