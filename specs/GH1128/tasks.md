# Task Plan

## Linked Issue

GH-1128 / #1128

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1128-T0` Covers: B-001 ～ B-009. Owner: maintainer/spec owner. Dependencies: none. Done when: 原规格已在 `user-2026-07-27-approve-all-specs` 获批；本 amendment 的 JSON/document semantic view、request-level function call、record-aware batch、256/2 MiB 上限与稳定 400 契约再次获得内容绑定批准，公开 security Issue 范围保持在已披露事实内，Issue 保持 `ready_to_implement`. Verify: SpecRail workflow/spec checks与 implement route gate。

## 实现任务

- [ ] `SP1128-T1` Covers: B-001, B-002, B-003, B-006, B-009. Owner: guardrail implementation owner. Dependencies: SP1128-T0. Done when: `content_text` 被 bounded fallible owned record builder 替代，覆盖 request/message legacy function、modern function、message name、普通 content、tool result/use，生成 adjacency 与 typed independent records，并对 JSON 做 raw+semantic 扫描；256 records/2 MiB 派生字节用 checked arithmetic 在外部调用前执行. Verify: focused fragment order/record-isolation/JSON/limit tests.
- [ ] `SP1128-T2` Covers: B-003, B-006, B-007. Owner: same implementation owner. Dependencies: SP1128-T1. Done when: 支持文本 MIME 的 document base64 解码为 UTF-8 正文；JSON MIME document 生成 raw+semantic records 且 invalid JSON 稳定 400；bad base64/UTF-8/unsupported MIME 在 input guardrail 开启时 fail-closed；无网络和二进制解析. Verify: document MIME/base64/JSON table tests.
- [ ] `SP1128-T3` Covers: B-004, B-005, B-006, B-008, B-009. Owner: engine/test owner. Dependencies: SP1128-T2. Done when: engine 提供 record-aware batch；本地 guardrails 保持边界；OpenAI moderation 使用一次 string-array 请求并验证 response count；provider-before-block、安全稳定 400、modified/mask fail-closed、`enabled: false`/`check_input: false` DTO 兼容与无网络证据完整. Verify: focused batch/external-call-count async tests and mock provider boundary test.
- [ ] `SP1128-T4` Covers: B-001 ～ B-009. Owner: verification owner. Dependencies: SP1128-T3. Done when: 实现 diff 已按最终 amendment 审计/修正，完整 Rust/SpecRail gate 与安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。实现集中在 `src/server/guardrails.rs` 与同一组测试，独立 owner 串行处理。
任何既有 Claude worktree 仅作为待审 diff，不视为已验证实现；验证只在本 session
最终实现 worktree 串行执行。

## 验证

- Product invariant 与 tasks `Covers:` 的并集必须都是 B-001 ～ B-009。
- 对每个结构化载体至少有 allow/block 两条 fixture；独立 records 还必须证明正则
  不能跨字段边界匹配，typed provenance 不进入扫描文本。
- moderation mock 必须证明 N 条非空 records 只产生 1 次远程请求；records/派生字节
  超限时 engine 与 provider 调用计数均为 0。
- 测试必须扫描解码后的 document 文本，不能仅断言 base64/JSON 字符串被拼接。
- PR 在最终 slice 使用 `Fixes #1128`，需要 security 人工 review。

## Handoff Notes

- Claude 原 diff 未处理 document base64 的语义性解码，不能直接提交。
- 不支持的二进制 document 在启用 input guardrail 时按规格 fail-closed。
- 保留现有 mask/modified 非可变边界的显式错误。
- 不自动合并、不 force-push。
