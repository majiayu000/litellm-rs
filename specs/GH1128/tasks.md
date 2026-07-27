# Task Plan

## Linked Issue

GH-1128 / #1128

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1128-T0` Covers: B-001 ～ B-009. Owner: maintainer/spec owner. Dependencies: none. Done when: 原规格已在 `user-2026-07-27-approve-all-specs` 获批；Issue 在本 amendment 审核期间移出 `ready_to_implement`，本 amendment 的 JSON/document semantic view、request-level function call、record-aware batch、不可吞掉的 response-integrity error、256/2 MiB 上限与稳定 400 契约获得 exact-head 内容绑定批准并合并后，才恢复 `ready_to_implement`. Verify: SpecRail workflow/spec checks、GitHub exact merged SHA 与 implement route gate。

## 实现任务

- [ ] `SP1128-T1` Covers: B-001, B-002, B-003, B-006, B-009. Owner: guardrail implementation owner. Dependencies: SP1128-T0. Done when: `content_text` 被 bounded fallible owned record builder 替代，覆盖 request/message legacy function、modern function、message name、普通 content、tool result/use，生成 adjacency 与 typed independent records，并对 JSON 做 raw+semantic 扫描；bounded duplicate-rejecting visitor 在构造 `Value` 前拒绝任意层级/escape-equivalent duplicate key，leading BOM 与 structured-looking argument 的 syntax/recursion/depth/resource 解析失败稳定 400，普通非 JSON 仍扫描 raw；256 records/2 MiB 派生字节用 checked arithmetic 在外部调用前执行. Verify: focused fragment order/record-isolation/JSON/duplicate-key/BOM/depth-limit/limit tests.
- [ ] `SP1128-T2` Covers: B-003, B-006, B-007. Owner: same implementation owner. Dependencies: SP1128-T1. Done when: plain/csv document base64 解码为 UTF-8 正文；JSON/`+json` document 生成 raw+semantic records 且 syntax/duplicate-key/BOM/depth/resource failure 稳定 400；Markdown（含 numeric/named entity）、HTML/XML/`+xml`/其他 `text/*`、bad base64/UTF-8 与其他 MIME 在 input guardrail 开启时 fail-closed；无网络、entity 或二进制解析. Verify: document MIME/base64/JSON/duplicate-key/BOM/depth/entity table tests.
- [ ] `SP1128-T3` Covers: B-004, B-005, B-006, B-008, B-009. Owner: engine/test owner. Dependencies: SP1128-T2. Done when: engine/traits/types 与 PII、prompt-injection、OpenAI moderation 提供 record-aware batch；trait 默认 batch adapter 让只实现旧 `check_input(&str)` 的 custom guardrail 逐 record 保持兼容；本地 guardrails 保持边界；OpenAI moderation 沿用 trim-empty eligibility、使用至多一次 string-array 请求，在发送前执行 eligible 原始字符串 32,768 UTF-8 bytes 上限并验证 response count；input limit 与 count mismatch 使用 engine 在 `fail_open` 判定前传播的 typed error，前者映射稳定 400；`Log` action merge 后继续，任何 `Mask` action 即使无 `modified_content` 也由 gateway fail-closed；provider-before-block、安全稳定 400、disabled path 不增加 guardrail-specific 拒绝、前置 malformed-base64 validator 兼容与无网络证据完整. Verify: focused custom-adapter/batch/byte-boundary/whitespace/log/mask-action/external-call-count/fail-open async tests and mock provider boundary test.
- [ ] `SP1128-T4` Covers: B-001 ～ B-009. Owner: verification owner. Dependencies: SP1128-T3. Done when: 实现 diff 已按最终 amendment 审计/修正，完整 Rust/SpecRail gate 与安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。实现由同一 owner 串行覆盖 `src/server/guardrails.rs`、core guardrail 的
`traits.rs`、`types.rs`、`engine.rs`、`openai_moderation.rs`、`pii.rs`、
`prompt_injection.rs` 及对应测试。任何既有 Claude worktree 仅作为待审 diff，
不视为已验证实现；验证只在本 session 最终实现 worktree 串行执行。

## 验证

- Product invariant 与 tasks `Covers:` 的并集必须都是 B-001 ～ B-009。
- 对每个结构化载体至少有 allow/block 两条 fixture；独立 records 还必须证明正则
  不能跨字段边界匹配，typed provenance 不进入扫描文本。
- moderation mock 必须证明 N 条 trim 后非空 records 只产生 1 次远程请求；
  mixed whitespace 只提交 eligible values，全 whitespace zero-call；records/派生
  字节超限时 engine 与 provider 调用计数均为 0；N-1/N+1 结果在 `fail_open`
  true/false 下都不可放行，`Log` action 仍允许 downstream model provider 调用，
  action-only `Mask` 则固定阻止该调用；32,768 eligible UTF-8 bytes 可提交，
  32,769 bytes 在 `fail_open` true/false 下 moderation/model provider 均 zero-call。
- custom guardrail fixture 只实现既有必需方法 `check_input(&str)`，证明无需新增 trait
  实现即可经默认 adapter 按顺序收到所有 records，且 block/Log/error 语义不变。
- 测试必须扫描解码后的 document 文本，不能仅断言 base64/JSON 字符串被拼接。
- PR 在最终 slice 使用 `Fixes #1128`，需要 security 人工 review。

## Handoff Notes

- Claude 原 diff 未处理 document base64 的语义性解码，不能直接提交。
- 不支持的二进制 document 在启用 input guardrail 时按规格 fail-closed。
- 保留现有 mask/modified 非可变边界的显式错误。
- 不自动合并、不 force-push。
