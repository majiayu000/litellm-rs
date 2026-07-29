# Task Plan

## Linked Issue

GH-1131 / #1131

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1131-T0` Covers: B-001 ～ B-007. Owner: maintainer/spec owner. Dependencies: none. Done when: 文案范围与当前 runtime 事实绑定最终 spec SHA 获批，Issue 添加 `ready_to_implement`. Verify: SpecRail workflow/spec checks与 implement route gate。

## 实现任务

- [ ] `SP1131-T1` Covers: B-001 ～ B-007. Owner: docs implementation owner. Dependencies: SP1131-T0. Done when: example cache 注释和 validation 注释准确覆盖 enabled/ttl、chat/embedding、bypass 与 semantic cache，且无运行时逻辑变化. Verify: `git diff --word-diff`; cache wiring 代码路径逐项核对.
- [ ] `SP1131-T2` Covers: B-001 ～ B-007. Owner: verification owner. Dependencies: SP1131-T1. Done when: YAML parse/validate、cache warning tests、格式和 check 通过，diff 保持 docs/comment-only. Verify: focused config/cache tests; `cargo fmt --check`; `cargo check`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。两处文案必须由同一 owner 对照同一运行时事实，验证在
`.claude/worktrees/agent-ab891b554a7c27682` 串行完成。

## 验证

- 现有 staged diff 必须先 unstaged/read-only 审查，不丢失用户内容。
- 关键否定事实测试不能锁定整段自然语言。
- mixed PR 最终使用 `Fixes #1131`；不声称 semantic cache 已实现。

## Handoff Notes

- Claude 已确认 `cargo fmt` 与 `cargo check --all-features`，但这些是旧 agent 输出；
  本会话必须重新运行才能作为完成证据。
- 不顺带增加 bypass 日志或修改 cache behavior。
- 不自动合并、不 force-push。
