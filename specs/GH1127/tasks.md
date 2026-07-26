# Task Plan

## Linked Issue

GH-1127 / #1127

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1127-T0` Covers: B-001 ～ B-009, HD-1127-1. Owner: maintainer/spec owner. Dependencies: none. Done when: 维护者在 `full_buffer`、`per_event_cumulative`、`windowed_cumulative` 中选择一种并绑定最终 spec SHA；若选 windowed，确定 YAML 字段、默认字符数和合法范围；Issue 获得 `ready_to_implement`. Verify: `python3 checks/check_workflow.py --repo . --spec-dir=specs/GH1127`; `python3 checks/route_gate.py --repo . --route implement --issue 1127 --state ready_to_implement --duplicate-evidence <evidence> --json`.

## 实现任务

- [ ] `SP1127-T1` Covers: B-001, B-002, B-003, B-005, B-008, B-009. Owner: stream guardrail implementation owner. Dependencies: SP1127-T0. Done when: `src/server/guardrails.rs` 提供唯一纯文本 output enforcement 与共享 `StreamOutputGuard`，实现已批准模式、有限内存、UTF-8 安全和 disabled fast path. Verify: focused `server::guardrails` state-machine tests.
- [ ] `SP1127-T2` Covers: B-001, B-004, B-006, B-007. Owner: same implementation owner. Dependencies: SP1127-T1. Done when: chat/completions/Responses 三条 stream loop 在发送前接入共享状态机；blocked/error 不泄露原文、不发 `[DONE]`，非文本 event 顺序与 usage 保持. Verify: 三 handler focused stream tests.
- [ ] `SP1127-T3` Covers: B-003, B-004, B-005, B-007, B-009. Owner: test owner. Dependencies: SP1127-T2. Done when: 跨 chunk、UTF-8、guardrail timeout/error/malformed、provider error、disconnect 和所有 lifecycle exactly-once fixture 完整. Verify: focused route/lifecycle test filters.
- [ ] `SP1127-T4` Covers: B-001 ～ B-009. Owner: verification owner. Dependencies: SP1127-T3. Done when: diff 只覆盖已批准设计，完整 Rust/SpecRail gate 通过且有安全人工 review. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。三条 stream loop 与共享生命周期状态紧密耦合，由一个 implementation owner
串行完成；验证也在同一 worktree 单路运行，避免重复冷构建和共享文件冲突。

## 验证

- 先运行状态机和三端点 focused tests，再对最终 head 运行一次完整命令集。
- 对每条失败路径断言已发送 SSE bytes 不包含被拒文本。
- 人工安全 review 必须确认 `HD-1127-1` 的不可撤回前缀风险与实现一致。
- PR 使用 `Fixes #1127` 仅在全部 invariants 完成时关闭 Issue。

## Handoff Notes

- 当前没有 GH1127 实现；不要从 GH1128 的输入扫描工作推断输出审核模式。
- 禁止先发送后审核或逐独立 chunk 扫描。
- 维护者尚未批准 `HD-1127-1`，在此之前不得开始代码实现。
- 不自动合并、不 force-push。
