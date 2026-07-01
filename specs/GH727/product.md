# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

当前 main 仍有大量 Rust 文件超过 U-16 的 800 行硬上限。一次性拆完会形成不可 review 的大 PR；
#727 要求用小 PR tranche 逐步拆分，并且每个 PR 只拥有一个文件或紧密文件家族。

## 本 tranche 目标

- 拆分 `src/core/providers/vertex_ai/embeddings/mod.rs`，它当前 1139 行，是 #727 当前 top offenders 之一。
- 按现有 `#[path = "..."] mod tests;` 模式，把 inline tests 移动到 `tests.rs`。
- 保持 Vertex AI embeddings runtime 代码、测试断言、fixtures 和 public API 不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 Vertex AI embeddings runtime 行为。
- 不重构 Vertex AI embedding model parsing、request/response transformation、batch handler 或 serialization rules。
- 不在本 PR 中处理其余大文件。
- 不关闭 #727，除非 issue owner 决定一个 tranche PR 足以满足 tracker。

## Behavior Invariants

1. 所有原有 Vertex AI embeddings tests 仍由 `core::providers::vertex_ai::embeddings::tests::*` 测试树运行。
2. 测试移动只能改变 module file location，不改变断言、fixtures 或 production code。
3. `src/core/providers/vertex_ai/embeddings/mod.rs` 和 `src/core/providers/vertex_ai/embeddings/tests.rs` 必须低于 800 行。
4. `cargo test core::providers::vertex_ai::embeddings --lib --all-features` 必须通过。

## 验收标准

- [ ] `src/core/providers/vertex_ai/embeddings/mod.rs` 使用 `#[path = "tests.rs"] mod tests;` 引入测试。
- [ ] `src/core/providers/vertex_ai/embeddings/tests.rs` 包含原 inline test module body，且低于 800 行。
- [ ] Focused Vertex AI embeddings tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 next tranche，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a Vertex AI embeddings test layout maintenance split for U-16 compliance.
