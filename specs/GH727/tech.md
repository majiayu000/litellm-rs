# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Azure responses transformation module | `src/core/providers/azure/responses/transformation.rs` | Contains runtime code plus a large inline `#[cfg(test)] mod tests`. | Inline tests make the file exceed the U-16 ceiling. |
| Existing split-test pattern | `src/core/providers/azure/chat.rs`, `src/core/providers/azure/chat_tests.rs`, `src/core/providers/azure/assistants.rs`, `src/core/providers/azure/assistants_tests.rs` | Runtime files use `#[path = "..."] mod tests;`. | Same pattern preserves test module identity while keeping the test file next to the module. |
| Azure responses transformation tests | `src/core/providers/azure/responses/transformation.rs` inline tests | Tests cover transform config defaults, response-format handling, metadata stripping, content filters, field normalization, custom mappings, nested field behavior, and edge cases. | Suitable for a mechanical move to `transformation_tests.rs` using `use super::*;`. |

## 设计方案

1. Move the body of `#[cfg(test)] mod tests` from `azure/responses/transformation.rs` into `azure/responses/transformation_tests.rs`.
2. Replace the inline module with `#[cfg(test)] #[path = "transformation_tests.rs"] mod tests;`.
3. Keep `use super::*;` and the existing fully qualified `serde_json::json!` calls in the moved file.
4. Do not edit production Azure responses transformation code.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `azure/responses/transformation.rs` test module declaration | `cargo test core::providers::azure::responses::transformation --lib --all-features` discovers all moved tests. |
| P2 | moved test module body | Focused Azure responses transformation tests pass with unchanged assertions. |
| P3 | file size | `wc -l src/core/providers/azure/responses/transformation.rs src/core/providers/azure/responses/transformation_tests.rs` shows both files below 800. |
| P4 | no runtime behavior change | `git diff --stat` and focused tests show only test layout movement plus SpecRail docs. |

## 风险

- Module path remains `azure::responses::transformation::tests` because `azure/responses/transformation.rs` still declares `mod tests`; only the file backing the module changes.
- Mechanical moves can drop an import or break access to private helper methods; focused test compilation catches this.
- This does not eliminate the full #727 backlog; the issue remains a tracker after this tranche.

## 测试计划

- [ ] SpecRail packet validation.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::providers::azure::responses::transformation --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/providers/azure/responses/transformation.rs` and `transformation_tests.rs`

## 回滚方案

Revert the Azure responses transformation test module split and `specs/GH727` edits. No migrations or runtime config changes are involved.
