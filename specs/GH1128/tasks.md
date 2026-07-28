# Task Plan

## Linked Issue

GH-1128 / #1128

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1128-T0` Covers: B-001 ～ B-009. Owner: maintainer/spec owner. Dependencies: none. Done when: 原规格已在 `user-2026-07-27-approve-all-specs` 获批；Issue 在本 amendment 审核期间移出 `ready_to_implement`，本 amendment 的 JSON/document semantic view、request-level function call、record-aware batch、不可吞掉的 response-integrity error、256/2 MiB 上限与稳定 400 契约获得 exact-head 内容绑定批准并合并后，才恢复 `ready_to_implement`. Verify: SpecRail workflow/spec checks、GitHub exact merged SHA 与 implement route gate。

## 实现任务

- [ ] `SP1128-T1` Covers: B-001, B-002, B-003, B-006, B-009. Owner: guardrail implementation owner. Dependencies: SP1128-T0. Done when: `content_text` 被 bounded fallible owned record builder 替代，覆盖 request/message legacy function、modern function、message name、普通 content、tool result/use；每条 message 的普通 text leaves 过滤其间非 text parts 后按固定顺序生成空串/单空格/换行三种 provider projections 并局部去重，全请求普通 text leaves 另生成 legacy newline view，其他字段生成 typed independent records；JSON 做 raw+semantic 扫描；bounded duplicate-rejecting visitor 在构造 `Value` 前拒绝任意层级/escape-equivalent duplicate key，leading BOM 与 structured-looking argument 的 syntax/recursion/depth/resource 解析失败稳定 400，普通非 JSON 仍扫描 raw；全部 projections/records 纳入 256/2 MiB checked accounting. Verify: focused filtered-part/three-projection/cross-message/provider-transform/record-isolation/JSON/duplicate-key/BOM/depth-limit/limit tests.
- [ ] `SP1128-T2` Covers: B-003, B-006, B-007. Owner: same implementation owner. Dependencies: SP1128-T1. Done when: plain/csv document base64 解码为 UTF-8 正文；JSON/`+json` document 生成 raw+semantic records 且 syntax/duplicate-key/BOM/depth/resource failure 稳定 400；Markdown（含 numeric/named entity）、HTML/XML/`+xml`/其他 `text/*`、bad base64/UTF-8 与其他 MIME 在 input guardrail 开启时 fail-closed；无网络、entity 或二进制解析. Verify: document MIME/base64/JSON/duplicate-key/BOM/depth/entity table tests.
- [ ] `SP1128-T3` Covers: B-004, B-005, B-006, B-008, B-009. Owner: engine/test owner. Dependencies: SP1128-T2. Done when: `engine.rs` 以 crate-private `EngineGuardrailEntry` 和 `InputBatchFailure` 提供 record-aware batch；公开 `Guardrail` trait、公开且可穷举匹配的 `GuardrailError` enum、`GuardrailEngine::check_input` 签名与 `add_guardrail` 方式保持不变；内部 legacy adapter 让只实现旧 `check_input(&str)` 的 custom guardrail 逐 record 保持兼容；PII/prompt-injection 保持本地 record 边界；OpenAI moderation 沿用 trim-empty eligibility、使用至多一次 string-array 请求，在发送前执行 eligible 原始字符串 32,768 UTF-8 bytes 上限并验证 response count；internal input-limit 与 response-integrity failures 在 `fail_open` 判定前传播，前者映射稳定 400，公开单字符串入口只映射到现有 `GuardrailError::Internal` 安全消息；`Log` action merge 后继续，任何 `Mask` action 即使无 `modified_content` 也由 gateway fail-closed；Responses background 在 queued persist/task/200 前执行完整 input check，失败 zero-state/zero-provider，通过后仅 lifecycle 可调用的 post-input entrypoint 保证 exactly-once；provider-before-block、安全稳定 400、disabled path 不增加 guardrail-specific 拒绝、前置 malformed-base64 validator 兼容与无网络证据完整. Verify: focused internal-custom-adapter/public-API-compile/batch/byte-boundary/whitespace/log/mask-action/background-prequeue/exactly-once/source-boundary/external-call-count/fail-open async tests.
- [ ] `SP1128-T4` Covers: B-001 ～ B-009. Owner: verification owner. Dependencies: SP1128-T3. Done when: 实现 diff 已按最终 amendment 审计/修正，完整 Rust/SpecRail gate 与安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。实现由同一 owner 串行覆盖 `src/server/guardrails.rs`、`src/server/routes/ai/chat.rs`、
`src/server/routes/ai/responses.rs`、`src/server/routes/ai/responses/lifecycle.rs`、
`src/server/routes/ai/responses/lifecycle_tests.rs`、core guardrail 的
`engine.rs`、`openai_moderation.rs`、`pii.rs`、
`prompt_injection.rs` 及对应测试。任何既有 Claude worktree 仅作为待审 diff，
不视为已验证实现；验证只在本 session 最终实现 worktree 串行执行。

## 验证

- Product invariant 与 tasks `Covers:` 的并集必须都是 B-001 ～ B-009。
- 对每个结构化载体至少有 allow/block 两条 fixture；独立 records 还必须证明正则
  不能跨字段边界匹配，typed provenance 不进入扫描文本。
- provider-boundary fixtures 必须证明 Bedrock space、Ollama newline、Gemini/Vertex
  empty join 都被对应 projection 覆盖；image-separated 与 split-message
  `ignore` + `all previous instructions` 在 provider 转换/调用前命中，alternative
  views 不互相跨 record 匹配。
- background Responses 必须在 queued persist/task spawn/200 前返回 normalization/
  guardrail 错误；失败 store/task/provider zero-call，通过 input check exactly-once，
  post-input unchecked entrypoint 的 source-boundary 只允许 lifecycle 调用。
- moderation mock 必须证明 N 条 trim 后非空 records 只产生 1 次远程请求；
  mixed whitespace 只提交 eligible values，全 whitespace zero-call；records/派生
  字节超限时 engine 与 provider 调用计数均为 0；N-1/N+1 结果在 `fail_open`
  true/false 下都不可放行，`Log` action 仍允许 downstream model provider 调用，
  action-only `Mask` 则固定阻止该调用；32,768 eligible UTF-8 bytes 可提交，
  32,769 bytes 在 `fail_open` true/false 下 moderation/model provider 均 zero-call。
- custom guardrail fixture 只实现既有必需方法 `check_input(&str)`，证明无需新增 trait
  实现即可经 engine-internal legacy adapter 按顺序收到所有 records，且
  block/Log/error 语义不变；另有下游式 compile fixture 对现有五个
  `GuardrailError` variants 穷举匹配，证明公开 enum/trait 未扩大。
- 测试必须扫描解码后的 document 文本，不能仅断言 base64/JSON 字符串被拼接。
- PR 在最终 slice 使用 `Fixes #1128`，需要 security 人工 review。

## Handoff Notes

- Claude 原 diff 未处理 document base64 的语义性解码，不能直接提交。
- 不支持的二进制 document 在启用 input guardrail 时按规格 fail-closed。
- 保留现有 mask/modified 非可变边界的显式错误。
- 不自动合并、不 force-push。
