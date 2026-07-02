# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@0cd9ccd2fb6f`, 25 tracked Rust files remain over the U-16
800-line ceiling. The current largest file is
`src/core/providers/bedrock/provider_tests.rs` at 864 lines. It is a test-only
Bedrock provider module whose sections already define clean behavior domains.

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

## Current Tranche: Bedrock Provider Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test root | `src/core/providers/bedrock/provider_tests.rs` | Loaded by `src/core/providers/bedrock/mod.rs` as `#[cfg(test)] mod provider_tests;`. | Must keep the parent test suite entry point stable. |
| Creation/capability tests | `provider_tests/creation_capability_tests.rs` | Provider construction, capabilities, supported params, model list, embedding detection. | Groups provider identity and capability surface checks. |
| Prompt/param tests | `provider_tests/prompt_param_tests.rs` | Message prompt conversion and OpenAI parameter mapping tests. | Keeps prompt/input mapping behavior together. |
| Request transform tests | `provider_tests/request_transform_tests.rs` | Claude/Titan/Nova/Llama/Mistral/AI21/Cohere request transform and error tests. | Groups model-family request body transformation behavior. |
| Response transform tests | `provider_tests/response_transform_tests.rs` | Model-family response parse tests and invalid/unknown response errors. | Groups response conversion behavior. |
| Cost/access tests | `provider_tests/cost_and_access_tests.rs` | Cost calculation, error mapper, feature client accessors, capability constant, clone/debug. | Keeps miscellaneous provider surface checks out of transform modules. |

### Design

1. Keep `src/core/providers/bedrock/provider_tests.rs` as the test root with shared helper
   functions `create_test_config` and `create_test_provider`.
2. Declare behavior-domain child modules from the root test module.
3. Move each existing test block under the matching child module without changing assertions,
   fixtures, model ids, JSON payloads, or expected errors.
4. Add imports inside child modules for only the symbols they need, avoiding a new shared
   prelude or broad public test helper surface.
5. Do not edit Bedrock production modules such as `provider.rs`, `client.rs`, `config.rs`,
   `transformation.rs`, `model_config.rs`, `utils`, or `sigv4.rs`.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `provider_tests.rs` | Root contains shared helpers and `mod *_tests;` declarations. |
| P2 | `provider_tests/*.rs` | Original tests are grouped by provider creation/capability, prompt/params, request transform, response transform, and cost/access. |
| P3 | file size | `wc -l src/core/providers/bedrock/provider_tests.rs src/core/providers/bedrock/provider_tests/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test core::providers::bedrock::provider_tests --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/core/providers/bedrock/provider_tests.rs`. |

## Risks

- Child modules need explicit imports for trait methods such as `LLMProvider`; otherwise the
  tests may compile differently from the original monolithic module.
- Capability constant tests must import `BEDROCK_CAPABILITIES` from the provider module while
  keeping it private to tests.
- Request transform tests should not change model ids or JSON assertions while removing the
  oversized root file.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `git diff --check`
- [ ] `python3 /Users/apple/Desktop/code/AI/tool/specrail/checks/check_workflow.py --repo /Users/apple/Desktop/code/AI/tool/specrail --spec-dir "$PWD/specs/GH727"`
- [ ] `wc -l src/core/providers/bedrock/provider_tests.rs src/core/providers/bedrock/provider_tests/*.rs`
- [ ] `cargo test core::providers::bedrock::provider_tests --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`

## Rollback

Move the Bedrock provider tests back into `src/core/providers/bedrock/provider_tests.rs`
and revert the `specs/GH727` edits. No schema, persistence, or runtime behavior
changes are involved.
