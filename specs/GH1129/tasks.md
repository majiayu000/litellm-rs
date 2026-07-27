# Task Plan

## Linked Issue

GH-1129 / #1129

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1129-T0` Covers: B-001 ～ B-012. Owner: maintainer/spec owner. Dependencies: none. Done when: amendment 审查期间 Issue 移出 `ready_to_implement`；本 amendment exact head 的 raw-domain total、Vertex 双 parser 与 thoughts 单次 output 计费、Converse total、key-only settlement 获得内容绑定批准并合并后，才恢复 `ready_to_implement`. Verify: SpecRail workflow/spec checks 与 implement route gate。

## 实现任务

- [ ] `SP1129-T1` Covers: B-002 ～ B-009. Owner: billing implementation owner. Dependencies: SP1129-T0. Done when: `src/core/providers/shared.rs` 提供唯一 crate-private normalizer，覆盖 raw `u64/u128` 校验、partial/nonzero/None、缩窄与 total 饱和. Verify: focused helper raw-domain/boundary tests.
- [ ] `SP1129-T2` Covers: B-001 ～ B-009, B-011. Owner: same implementation owner. Dependencies: SP1129-T1. Done when: Azure、Azure AI、Vertex 两条与 direct Gemini 非流式 parser（含 thoughts/tool-use）、Bedrock（含 Converse total）各模型族、Mistral parser 只解析已声明字段并全部调用 helper；Vertex 使用四项 reported-total policy，direct Gemini 使用 prompt+candidates+thoughts policy；删除直接 `as u32` 与手写零 Usage；Google thoughts 仅包含在 completion、不填 reasoning/thinking details. Verify: provider-specific valid/malformed/all-zero/endpoint-total tests and Vertex/direct-Gemini exact-cost/no-double-charge assertions.
- [ ] `SP1129-T3` Covers: B-010, B-012. Owner: spend implementation/test owner. Dependencies: SP1129-T2. Done when: `None` 的 provider+key/provider-only/key-only/neither 四种 reservation 组合恰好终止一次；provider+key 金额不同时各按自身 reserved amount 结算，API-key usage 使用 key-first fallback cost；测试证明不产生成功的零 token/$0 记录或 reservation 泄漏. Verify: focused spend reservation/usage tests.
- [ ] `SP1129-T4` Covers: B-001 ～ B-012. Owner: verification owner. Dependencies: SP1129-T3. Done when: 最终 diff 全量审计，所有 provider 路径和完整 Rust/SpecRail gate 通过，计费安全人工 review 完成. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。共享 usage helper 与各 provider parser 有直接依赖，单一 owner 串行修改；
验证只在本 session 最终实现 worktree 单路运行。

## 验证

- 对 Issue 列出的每个具体 parser 返回点建立覆盖清单，禁止只测一个样本。
- Vertex `transformers.rs`、`client.rs` 与 direct Gemini `client.rs` 三条路径必须
  使用共享严格 helper 并分别有扩展 token fixture；Vertex 四项与 direct Gemini
  三项 reported-total fixture 都必须覆盖，thoughts 的 output cost 恰好一次、
  reasoning cost 为零。
- `u32::MAX + 1`、`u64::MAX` 与 total overflow 必须有新鲜输出。
- 不修改测试断言来接受 silent zero billing。
- 最终 PR 使用 `Fixes #1129`，需要 billing 人工 review。

## Handoff Notes

- Claude diff 改动约 600 行且未完成构建，不能把“代码已写”当作验证。
- 不新增 provider 字段 alias，不接受数字字符串猜测。
- `None` 的下游错误/结算语义必须通过端到端 focused test。
- 不自动合并、不 force-push。
