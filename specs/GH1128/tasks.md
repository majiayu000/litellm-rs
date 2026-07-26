# Task Plan

## Linked Issue

GH-1128 / #1128

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1128-T0` Covers: B-001 ～ B-008. Owner: maintainer/spec owner. Dependencies: none. Done when: 最终 product/tech/tasks SHA 获得内容绑定批准，公开 security Issue 的范围保持在已披露事实内，并添加 `ready_to_implement`. Verify: SpecRail workflow/spec checks与 implement route gate。

## 实现任务

- [ ] `SP1128-T1` Covers: B-001, B-002, B-003, B-006. Owner: guardrail implementation owner. Dependencies: SP1128-T0. Done when: `content_text` 被 fallible owned fragment builder 替代，覆盖 message name、普通 content、legacy/modern function、tool result/use，并以稳定标签/边界组合. Verify: focused fragment order/boundary/JSON tests.
- [ ] `SP1128-T2` Covers: B-003, B-006, B-007. Owner: same implementation owner. Dependencies: SP1128-T1. Done when: 支持文本 MIME 的 document base64 解码为 UTF-8 正文；bad base64/UTF-8/unsupported MIME 在 input guardrail 开启时 fail-closed；无网络和二进制解析. Verify: document MIME/base64 table tests.
- [ ] `SP1128-T3` Covers: B-004, B-005, B-008. Owner: test owner. Dependencies: SP1128-T2. Done when: provider-before-block、modified/mask fail-closed、disabled guardrail DTO 兼容与无网络证据完整. Verify: focused async guardrail tests and mock provider boundary test.
- [ ] `SP1128-T4` Covers: B-001 ～ B-008. Owner: verification owner. Dependencies: SP1128-T3. Done when: Claude 保留 diff 已按批准规格审计/修正，完整 Rust/SpecRail gate 与安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。实现集中在 `src/server/guardrails.rs` 与同一组测试，独立 owner 串行处理。
现有 `.claude/worktrees/agent-a9318fa5b8e2ddbdd` 仅作为待审 diff，不视为已验证实现。

## 验证

- Product invariant 与 tasks `Covers:` 的并集必须都是 B-001 ～ B-008。
- 对每个结构化载体至少有 allow/block 两条 fixture。
- 测试必须扫描解码后的 document 文本，不能仅断言 base64/JSON 字符串被拼接。
- PR 在最终 slice 使用 `Fixes #1128`，需要 security 人工 review。

## Handoff Notes

- Claude 原 diff 未处理 document base64 的语义性解码，不能直接提交。
- 不支持的二进制 document 在启用 input guardrail 时按规格 fail-closed。
- 保留现有 mask/modified 非可变边界的显式错误。
- 不自动合并、不 force-push。
