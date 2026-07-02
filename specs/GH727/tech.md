# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@6e81da76d9a4`, 41 Rust files remain over the U-16 800-line ceiling.
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

## Current Tranche: SDK Types Facade

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| SDK types facade | `src/sdk/types.rs` | Contains all SDK chat/message/tool/usage DTOs plus a large inline test suite. | The file is 1163 lines and combines multiple DTO domains with test coverage. |
| SDK client imports | `src/sdk/client/*.rs` | Imports SDK types from `crate::sdk::types::{...}` and `super::types` for client-internal types. | Root re-exports must preserve the existing `crate::sdk::types::*` API. |
| Existing Rust module pattern | `src/core/types/*.rs`, `src/core/cost/calculator/tests/*.rs` | Uses focused child modules plus root re-exports or test submodules. | SDK types can use the same facade-compatible split without changing runtime behavior. |

### Design

1. Keep `src/sdk/types.rs` as the public root facade.
2. Move message and multimodal DTOs into `src/sdk/types/message.rs`:
   - `Role`
   - `Content`
   - `ContentPart`
   - `ImageUrl`
   - `AudioData`
   - `Message`
   - `MessageDelta`
3. Move tool DTOs into `src/sdk/types/tool.rs`:
   - `ToolCall`
   - `Function`
   - `Tool`
   - `ToolChoice`
4. Move chat request/response DTOs into `src/sdk/types/chat.rs`:
   - `SdkChatRequest`
   - `ChatOptions`
   - `ChatResponse`
   - `ChatChoice`
   - `ChatChunk`
   - `ChunkChoice`
5. Move usage and cost DTOs into `src/sdk/types/usage.rs`:
   - `Usage`
   - `Cost`
   - `CostBreakdown`
6. Re-export every original public type from `types.rs` with `pub use` so `crate::sdk::types::*` remains compatible.
7. Split the inline `#[cfg(test)] mod tests` body into `src/sdk/types_tests/{message_tests.rs,tool_tests.rs,chat_tests.rs,streaming_usage_tests.rs}` and keep the test tree rooted under `sdk::types::tests`.
8. Do not change fields, derives, serde attributes, enum variants, assertions, or SDK client behavior.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `types.rs` root facade and child module declarations | `cargo test sdk::types --lib --all-features` discovers moved tests under the same root test tree. |
| P2 | `types/message.rs`, `types/tool.rs`, `types/chat.rs`, `types/usage.rs` | `cargo check --all-features --locked` compiles existing SDK imports through root re-exports. |
| P3 | `types_tests/*.rs` | Focused SDK type tests pass with unchanged assertions. |
| P4 | file size | `wc -l src/sdk/types.rs src/sdk/types/*.rs src/sdk/types_tests/*.rs` shows all touched files below 800. |
| P5 | public surface | `rg -n "pub use .*Role|pub use .*SdkChatRequest|pub use .*Usage" src/sdk/types.rs` confirms original root exports. |

## Risks

- Type dependencies cross domains: message DTOs depend on `ToolCall`, and chat DTOs depend on message, tool, and usage DTOs. Child modules should use explicit `super::{...}` imports rather than a broad prelude.
- Private child modules plus root `pub use` preserve the existing public API without committing new module paths such as `sdk::types::chat`.
- Test modules must import through the root facade to exercise the preserved public path.
- This tranche reduces one of the remaining 41 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test sdk::types --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/sdk/types.rs`, `src/sdk/types/*.rs`, and `src/sdk/types_tests/*.rs`

## Rollback

Revert the SDK types module split and `specs/GH727` edits. No migrations,
runtime config changes, or public API changes are involved.
