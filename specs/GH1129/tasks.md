# Task Plan

## Linked Issue

GH-1129 / #1129

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1129-T0` Covers: B-001 ～ B-013. Owner: maintainer/spec owner. Dependencies: none. Done when: amendment 审查期间 Issue 移出 `ready_to_implement`；本 amendment exact head 的 raw-domain total、Vertex/direct-Gemini 全 parser（含 standard chat/completions/responses SSE 与 native SDK）、thoughts/cache 单次计费、Converse total、各 reservation 自有金额、standard terminal flush 及 output-then-read-error settlement 获得内容绑定批准并合并后，才恢复 `ready_to_implement`. Verify: SpecRail workflow/spec checks 与 implement route gate。

## 实现任务

- [ ] `SP1129-T1` Covers: B-002 ～ B-009, B-011. Owner: billing implementation owner. Dependencies: SP1129-T0. Done when: `src/core/providers/shared.rs` 提供唯一 crate-private normalizer，覆盖 raw `u64/u128` 校验、partial/nonzero/None、endpoint-specific total、缩窄/饱和，以及 optional cached raw 校验、`cached <= prompt` 与 details 映射. Verify: focused helper raw-domain/cache/boundary tests.
- [ ] `SP1129-T2` Covers: B-001 ～ B-009, B-011, B-013. Owner: same implementation owner. Dependencies: SP1129-T1. Done when: Azure、Azure AI、Vertex 两条、direct Gemini provider client、`gemini/streaming.rs` + `base/sse/gemini.rs` standard chat stream 与 native SDK unary/SSE shared parser（均含 malformed/overflow/thoughts/tool-use/cache）、Bedrock（含 Converse total）各模型族、Mistral parser 只解析已声明字段并全部调用 helper；standard Gemini/Vertex stream 在私有 accumulator 中保持 Missing/Valid/Invalid，每个 clone 创建独立 accumulator，剥离中间 usage，仅最终 Valid 发布合法 usage-only chunk，且在 EOF/read error 前处理残留 buffer；Vertex 使用四项 reported-total policy，direct Gemini 使用 prompt+candidates+thoughts policy；删除直接 `as u32` 与手写零 Usage；Google thoughts 仅包含在 completion、reasoning details 为空，`Usage` 路径的 `thinking_usage` 与所有 Gemini 路径 cached pricing 保留. Verify: provider/SSE-specific valid/malformed/all-zero/overflow/endpoint-total/cache tests, direct/Vertex Valid→Invalid/Missing/recovery/terminal-flush 与 clone 交错隔离 tests, public serialization no-marker/no-zero-usage assertions and Vertex/direct-Gemini exact-cost/no-double-charge assertions.
- [ ] `SP1129-T3` Covers: B-010, B-012 ～ B-013. Owner: spend implementation/test owner. Dependencies: SP1129-T2. Done when: common 与 native Gemini SDK no-usage 路径统一委托同一 helper；`None` 的 provider+key/provider-only/key-only/neither 四种 reservation 组合恰好终止一次；provider+key 金额不同时各按自身 reserved amount 结算，API-key usage 使用 key-first fallback cost；`gemini.rs` 所有 stream 终止分支传真实 `should_record_spend && saw_upstream_output`；公开 provider stream 与 standard chat/completions/responses 最终序列化不携带内部 marker/伪零 usage，Invalid 不复用旧 usage/tokens；route test 覆盖 output-then-read-error 与 error-before-output；测试证明不产生成功的零 token/$0 记录或 reservation 泄漏. Verify: focused common/native-SDK spend reservation/usage tests, public provider stream/standard route final serialization tests and runtime-provider stream-error route tests.
- [ ] `SP1129-T4` Covers: B-001 ～ B-013. Owner: verification owner. Dependencies: SP1129-T3. Done when: 最终 diff 全量审计，所有 provider 路径和完整 Rust/SpecRail gate 通过，计费安全人工 review 完成. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。共享 usage helper 与各 provider parser 有直接依赖，单一 owner 串行修改；
验证只在本 session 最终实现 worktree 单路运行。

## 验证

- 对 Issue 列出的每个具体 parser 返回点建立覆盖清单，禁止只测一个样本。
- Vertex `transformers.rs`、`client.rs`、direct Gemini `client.rs`、standard stream
  `gemini/streaming.rs` + `base/sse/gemini.rs` 与 native SDK `gemini/spend.rs` 必须使用
  共享严格 helper 并分别有 malformed/overflow/扩展 token/cache fixture；Vertex 四项
  与 direct Gemini 三项 reported-total fixture 都必须覆盖，thoughts 的 output cost
  恰好一次、reasoning cost 为零，cached pricing 与 `Usage.thinking_usage` 保持。
- `base/sse.rs`、公开 direct/Vertex provider stream 与 standard
  chat/completions/responses 路由必须覆盖
  Valid→Invalid、Valid→Missing、Valid→Invalid→Valid、EOF truncated 与 read-error
  residual buffer；两个 transformer clone 交错消费/终止时不得共享 usage 或
  Finalized 状态；三条真实路由的公开序列化不得携带 marker/伪零 usage，Invalid 后
  callback/settlement 不得复用旧 Valid。
- native Gemini `gemini.rs` 的 output-then-read-error 测试必须证明实际
  `saw_upstream_output` 传入 settlement、provider/key 各按自身 reservation 金额结算；
  error-before-output 不得伪造该状态。
- `u32::MAX + 1`、`u64::MAX` 与 total overflow 必须有新鲜输出。
- 不修改测试断言来接受 silent zero billing。
- 最终 PR 使用 `Fixes #1129`，需要 billing 人工 review。

## Handoff Notes

- Claude diff 改动约 600 行且未完成构建，不能把“代码已写”当作验证。
- 不新增 provider 字段 alias，不接受数字字符串猜测。
- `None` 的下游错误/结算语义必须通过端到端 focused test。
- 标准 chat-completions Gemini stream 与 native SDK SSE 是不同入口，二者不得因
  非流式/parser 修复而被误判为已覆盖。
- 不自动合并、不 force-push。
