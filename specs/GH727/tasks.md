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
- [ ] `SP727-T20` Owner: verification owner. Done when: SpecRail, formatting, all-features check, PR CI, and review-thread gate pass for the Vertex AI embeddings tranche. Verify: `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH727"`; `cargo fmt --all -- --check`; `cargo check --all-features --locked`; GitHub PR CI and review-thread query.

## 并行拆分

This is a serial writable lane for one Vertex AI embeddings test file family. Other #727 large-file tranches may be planned read-only in parallel, but they must not edit this branch.

Writable ownership for this lane:

- `specs/GH727/`
- `src/core/providers/vertex_ai/embeddings/mod.rs`
- `src/core/providers/vertex_ai/embeddings/tests.rs`

## 验证

- SpecRail packet validation.
- `cargo fmt --all -- --check`
- `cargo test core::providers::vertex_ai::embeddings --lib --all-features`
- `cargo check --all-features --locked`
- PR CI and GraphQL review-thread gate before merge.

## Handoff Notes

This PR is the next #727 maintenance tranche and should not use `Closes #727`.
The issue should remain open until enough large-file tranches are completed or the tracker is explicitly closed.
