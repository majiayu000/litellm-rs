# Task Plan

## Linked Issue

GH-727 / #727

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP727-T1` Owner: coordinator. Done when: `specs/GH727/product.md`, `tech.md`, and `tasks.md` exist and pass SpecRail packet validation. Verify: from this repository with a local SpecRail checkout, `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH727"`.
- [x] `SP727-T2` Owner: coordinator. Done when: `src/core/providers/thinking/tests.rs` is split into `tests/mod.rs` and provider-specific child modules. Verify: `git diff --stat`; `wc -l src/core/providers/thinking/tests/*.rs`.
- [x] `SP727-T3` Owner: coordinator. Done when: moved tests compile and pass with unchanged assertions. Verify: `cargo test core::providers::thinking --lib`.
- [x] `SP727-T4` Owner: verification owner. Done when: formatting, all-features check, PR CI, and review-thread gate pass. Verify: `cargo fmt --all -- --check`; `cargo check --all-features --locked`; GitHub PR CI and review-thread query.
- [x] `SP727-T5` Owner: coordinator. Done when: `src/core/providers/azure/assistants.rs` delegates tests with `#[path = "assistants_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/azure/assistants.rs`.
- [x] `SP727-T6` Owner: coordinator. Done when: original inline Azure Assistants tests are moved to `src/core/providers/azure/assistants_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/azure/assistants_tests.rs`.
- [x] `SP727-T7` Owner: verification owner. Done when: both Azure Assistants files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/azure/assistants.rs src/core/providers/azure/assistants_tests.rs`; `cargo test core::providers::azure::assistants --lib --all-features`.
- [x] `SP727-T8` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Azure Assistants tranche. Verify: `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH727"`; `cargo fmt --all -- --check`; `cargo check --all-features --locked`; GitHub PR CI and review-thread query.
- [x] `SP727-T9` Owner: coordinator. Done when: `src/core/providers/azure/batches/mod.rs` delegates tests with `#[path = "batches_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/azure/batches/mod.rs`.
- [x] `SP727-T10` Owner: coordinator. Done when: original inline Azure Batch tests are moved to `src/core/providers/azure/batches/batches_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/azure/batches/batches_tests.rs`.
- [x] `SP727-T11` Owner: verification owner. Done when: both Azure Batch files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/azure/batches/mod.rs src/core/providers/azure/batches/batches_tests.rs`; `cargo test core::providers::azure::batches --lib --all-features`.
- [x] `SP727-T12` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Azure Batch tranche. Verify: #805 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T13` Owner: coordinator. Done when: `src/core/providers/jina/mod.rs` delegates tests with `#[path = "tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/jina/mod.rs`.
- [x] `SP727-T14` Owner: coordinator. Done when: original inline Jina tests are moved to `src/core/providers/jina/tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/jina/tests.rs`.
- [x] `SP727-T15` Owner: verification owner. Done when: both Jina files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/jina/mod.rs src/core/providers/jina/tests.rs`; `cargo test core::providers::jina --lib --all-features`.
- [x] `SP727-T16` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Jina tranche. Verify: #806 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T17` Owner: coordinator. Done when: `src/core/providers/vertex_ai/embeddings/mod.rs` delegates tests with `#[path = "tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/vertex_ai/embeddings/mod.rs`.
- [x] `SP727-T18` Owner: coordinator. Done when: original inline Vertex AI embeddings tests are moved to `src/core/providers/vertex_ai/embeddings/tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/vertex_ai/embeddings/tests.rs`.
- [x] `SP727-T19` Owner: verification owner. Done when: both Vertex AI embeddings files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/vertex_ai/embeddings/mod.rs src/core/providers/vertex_ai/embeddings/tests.rs`; `cargo test core::providers::vertex_ai::embeddings --lib --all-features`.
- [x] `SP727-T20` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Vertex AI embeddings tranche. Verify: #807 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T21` Owner: coordinator. Done when: `src/core/providers/azure/responses/transformation.rs` delegates tests with `#[path = "transformation_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/azure/responses/transformation.rs`.
- [x] `SP727-T22` Owner: coordinator. Done when: original inline Azure responses transformation tests are moved to `src/core/providers/azure/responses/transformation_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/azure/responses/transformation_tests.rs`.
- [x] `SP727-T23` Owner: verification owner. Done when: both Azure responses transformation files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/azure/responses/transformation.rs src/core/providers/azure/responses/transformation_tests.rs`; `cargo test core::providers::azure::responses::transformation --lib --all-features`.
- [x] `SP727-T24` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Azure responses transformation tranche. Verify: #808 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T25` Owner: coordinator. Done when: `src/core/providers/azure/responses/utils.rs` delegates tests with `#[path = "utils_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/azure/responses/utils.rs`.
- [x] `SP727-T26` Owner: coordinator. Done when: original inline Azure responses utils tests are moved to `src/core/providers/azure/responses/utils_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/azure/responses/utils_tests.rs`.
- [x] `SP727-T27` Owner: verification owner. Done when: both Azure responses utils files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/azure/responses/utils.rs src/core/providers/azure/responses/utils_tests.rs`; `cargo test core::providers::azure::responses::utils --lib --all-features`.
- [x] `SP727-T28` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Azure responses utils tranche. Verify: #809 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T29` Owner: coordinator. Done when: `src/core/providers/azure/responses/processor.rs` delegates tests with `#[path = "processor_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/azure/responses/processor.rs`.
- [x] `SP727-T30` Owner: coordinator. Done when: original inline Azure responses processor tests are moved to `src/core/providers/azure/responses/processor_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/azure/responses/processor_tests.rs`.
- [x] `SP727-T31` Owner: verification owner. Done when: both Azure responses processor files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/azure/responses/processor.rs src/core/providers/azure/responses/processor_tests.rs`; `cargo test core::providers::azure::responses::processor --lib --all-features`.
- [x] `SP727-T32` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Azure responses processor tranche. Verify: #810 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T33` Owner: coordinator. Done when: `src/core/providers/bedrock/utils/cost.rs` delegates tests with `#[path = "cost_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/bedrock/utils/cost.rs`.
- [x] `SP727-T34` Owner: coordinator. Done when: original inline Bedrock cost tests are moved to `src/core/providers/bedrock/utils/cost_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/bedrock/utils/cost_tests.rs`.
- [x] `SP727-T35` Owner: verification owner. Done when: both Bedrock cost files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/bedrock/utils/cost.rs src/core/providers/bedrock/utils/cost_tests.rs`; `cargo test core::providers::bedrock::utils::cost --lib --all-features`.
- [x] `SP727-T36` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Bedrock cost tranche. Verify: #811 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T37` Owner: coordinator. Done when: `src/core/providers/cloudflare/provider.rs` delegates tests with `#[path = "provider_tests.rs"] mod tests;`. Verify: `git diff -- src/core/providers/cloudflare/provider.rs`.
- [x] `SP727-T38` Owner: coordinator. Done when: original inline Cloudflare provider tests are moved to `src/core/providers/cloudflare/provider_tests.rs` without changing assertions. Verify: `rg -n "fn test_" src/core/providers/cloudflare/provider_tests.rs`.
- [x] `SP727-T39` Owner: verification owner. Done when: both Cloudflare provider files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/cloudflare/provider.rs src/core/providers/cloudflare/provider_tests.rs`; `cargo test core::providers::cloudflare::provider --lib --all-features`.
- [x] `SP727-T40` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Cloudflare provider tranche. Verify: #812 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T41` Owner: coordinator. Done when: #727 has a full remaining-file decoupling design covering test suites, public type facades, runtime orchestrators, and shared utilities. Verify: `git diff -- specs/GH727/product.md specs/GH727/tech.md`.
- [x] `SP727-T42` Owner: coordinator. Done when: `src/core/cost/calculator/tests.rs` keeps only shared helpers plus child module declarations. Verify: `sed -n '1,120p' src/core/cost/calculator/tests.rs`.
- [x] `SP727-T43` Owner: coordinator. Done when: pricing lookup, provider alias, and shared catalog lookup tests move to `src/core/cost/calculator/tests/pricing_lookup_tests.rs` without changing assertions. Verify: `rg -n "test_generic_cost_per_token|test_get_.*pricing|provider_variants|shared" src/core/cost/calculator/tests/pricing_lookup_tests.rs`.
- [x] `SP727-T44` Owner: coordinator. Done when: component cost tests move to `component_cost_tests.rs`, estimate/compare tests move to `estimation_comparison_tests.rs`, edge-case tests move to `edge_case_tests.rs`, and workflow tests move to `workflow_tests.rs`. Verify: `rg -n "test_calculate_|test_estimate_|test_compare_|test_large_|test_cost_calculation_workflow" src/core/cost/calculator/tests/*.rs`.
- [x] `SP727-T45` Owner: verification owner. Done when: all cost calculator test files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/cost/calculator/tests.rs src/core/cost/calculator/tests/*.rs`; `cargo test core::cost::calculator --lib --all-features`.
- [x] `SP727-T46` Owner: verification owner. Done when: formatting, all-features check, PR CI, and review-thread gate pass for the cost calculator test-suite tranche. Verify: #813 PR body, green PR CI, and GraphQL review-thread query.
- [x] `SP727-T47` Owner: coordinator. Done when: after this tranche merges, the next #727 tranche is selected from the remaining queue with fresh line-count evidence. Verify: `rg --files -g '*.rs' src tests | xargs wc -l | awk '$1 > 800 && $2 != "total" { print $1 " " $2 }' | sort -nr`.
- [x] `SP727-T48` Owner: coordinator. Done when: `src/core/providers/vertex_ai/client.rs` declares focused `error_mapper`, `url`, `health`, and path-backed `tests` modules. Verify: `sed -n '1,80p' src/core/providers/vertex_ai/client.rs`.
- [x] `SP727-T49` Owner: coordinator. Done when: `VertexAIErrorMapper` moves to `src/core/providers/vertex_ai/client/error_mapper.rs` with unchanged mapping arms. Verify: `rg -n "VertexAIErrorMapper|INVALID_ARGUMENT|RESOURCE_EXHAUSTED|map_network_error" src/core/providers/vertex_ai/client/error_mapper.rs`.
- [x] `SP727-T50` Owner: coordinator. Done when: URL construction moves to `src/core/providers/vertex_ai/client/url.rs` and health check moves to `src/core/providers/vertex_ai/client/health.rs`. Verify: `rg -n "build_url|get_publisher_for_model|check_health" src/core/providers/vertex_ai/client/*.rs`.
- [x] `SP727-T51` Owner: coordinator. Done when: original inline Vertex AI client tests move to `src/core/providers/vertex_ai/client_tests.rs` without changing assertions. Verify: `rg -n "test_error_mapper_http_400|test_url_format_standard_location|test_vertex_ai_error_api_error" src/core/providers/vertex_ai/client_tests.rs`.
- [x] `SP727-T52` Owner: verification owner. Done when: touched Vertex AI client files are below U-16's 800-line ceiling and focused tests pass. Verify: `wc -l src/core/providers/vertex_ai/client.rs src/core/providers/vertex_ai/client/*.rs src/core/providers/vertex_ai/client_tests.rs`; `cargo test core::providers::vertex_ai::client --lib --all-features`.
- [ ] `SP727-T53` Owner: verification owner. Done when: formatting, all-features check, PR CI, and review-thread gate pass for the Vertex AI client tranche. Verify: `cargo fmt --all -- --check`; `cargo check --all-features --locked`; GitHub PR CI and review-thread query.
- [ ] `SP727-T54` Owner: coordinator. Done when: after this tranche merges, the next #727 tranche is selected from the remaining queue with fresh line-count evidence. Verify: `rg --files -g '*.rs' src tests | xargs wc -l | awk '$1 > 800 && $2 != "total" { print $1 " " $2 }' | sort -nr`.

## 并行拆分

This is a serial writable lane for the Vertex AI client file family. Other #727 large-file tranches may be planned read-only in parallel, but they must not edit this branch.

Writable ownership for this lane:

- `specs/GH727/`
- `src/core/providers/vertex_ai/client.rs`
- `src/core/providers/vertex_ai/client/*.rs`
- `src/core/providers/vertex_ai/client_tests.rs`

## 验证

- SpecRail packet review.
- `cargo fmt --all -- --check`
- `cargo test core::providers::vertex_ai::client --lib --all-features`
- `cargo check --all-features --locked`
- PR CI and GraphQL review-thread gate before merge.

## Handoff Notes

This PR is the next #727 maintenance tranche and should use `Refs #727`, not `Closes #727`.
The issue should remain open until the final scan shows no Rust files over the U-16 ceiling.
