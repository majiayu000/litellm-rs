# Task Plan

## Linked Issue

GH-1127 / #1127

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1127-T0` Covers: B-001 ～ B-016, HD-1127-1. Owner: maintainer/spec owner. Dependencies: none. Done when: amendment 审查期间 Issue 移出 `ready_to_implement`；维护者对本 amendment exact head 批准 `windowed_cumulative`、YAML 字段 `guardrails.stream_output_check_chars`、默认 `256` 与合法范围 `1..=4096`，且 amendment 合并后才恢复 `ready_to_implement`. Verify: `python3 checks/check_workflow.py --repo . --spec-dir=specs/GH1127`; `python3 checks/route_gate.py --repo . --route implement --issue 1127 --state ready_to_implement --duplicate-evidence <evidence> --json`.

## 实现任务

- [ ] `SP1127-T1` Covers: B-001, B-002, B-003, B-005, B-008, B-012, B-013, B-014, B-015. Owner: stream guardrail implementation owner. Dependencies: SP1127-T0. Done when: config 与 `check_output_text`、共享 `StreamOutputGuard` 完成 fixed windowed cumulative、256/1..=4096、8 MiB 双上限与全部客户端可见文本 surface；Chat/Completion 共用单一 `LogprobSurfaceAccumulator`，按 choice 跨 event 维护且不可逆物化 text/chosen alias，top candidates 按 `(choice.index, candidate_index)` 跨 position/event 连续累计且不做逐位置 chosen skip；Responses/reasoning 去重、Chat parallel tool-call 按 `(choice.index, tool_call.index, field)` 维护连续 surface、UTF-8 安全和 disabled fast path. Verify: focused config/guardrail/shared-logprob/state-machine tests.
- [ ] `SP1127-T2` Covers: B-001, B-004, B-006, B-007, B-009, B-010, B-011, B-012, B-013, B-016. Owner: same implementation owner. Dependencies: SP1127-T1. Done when: chat/completions/Responses 三条 stream loop 在每个窗口发送前接入累计检查；Responses initial name 在 item-added 前扫描、late name 在 state update 时不扫描且只在 output-item-done 首次发布前扫描；blocked/error 不泄露当前 pending，发送稳定 error + 一个 `[DONE]`，事件顺序、usage、lease/callback/settlement/persistence 保持. Verify: 三 handler focused stream/lifecycle tests.
- [ ] `SP1127-T3` Covers: B-002 ～ B-016. Owner: test owner. Dependencies: SP1127-T2. Done when: 单/多窗口、后续窗口违规、跨 chunk、UTF-8、Chat content/thinking/reasoning/audio transcript/logprobs/tool/function、parallel tool calls 交错 `call0:sec`/`call1:x`/`call0:ret` 的连续与隔离、Completion text/echo/logprobs token/refusal、Responses output/reasoning/function surface、Responses initial/late/repeated name、late-name update 后 provider error 不扫描、late name 在 output-item-done 前检查、state 已存在时 accepted arguments、pre-ID dropped arguments 不扫描、done/snapshot 去重；共享 logprob unit tests 与 Chat/Completion 各自 integration tests 都覆盖 chosen tokens 跨 token 拼词、前一 event `text/chosen=sec` 后一 event chosen-only `ret` 的不可逆 alias 物化、top candidate 同一 rank 跨 position/event 的 `sec`/`ret`（含首位置 top 等于 chosen 后分叉）；另覆盖 guardrail timeout/error/fail_open、overflow、pending 后 provider error/idle 不泄露、disconnect 和 lifecycle exactly-once；不得用不可从 canonical provider pipeline 到达的 synthetic Chat refusal 声称覆盖. Verify: focused shared-logprob/route/lifecycle test filters.
- [ ] `SP1127-T4` Covers: B-001 ～ B-016. Owner: verification owner. Dependencies: SP1127-T3. Done when: diff 只覆盖已批准设计，完整 Rust/SpecRail gate 通过且有安全人工 review. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。三条 stream loop 与共享生命周期状态紧密耦合，由一个 implementation owner
串行完成；验证也在同一 worktree 单路运行，避免重复冷构建和共享文件冲突。

## 验证

- 先运行状态机和三端点 focused tests，再对最终 head 运行一次完整命令集。
- 对每条失败路径断言已发送 SSE bytes 不包含被拒文本。
- 人工安全 review 必须确认 `HD-1127-1` 的不可撤回前缀风险、当前 pending 不泄露、
  累计而非独立窗口检查与实现一致。
- PR 使用 `Fixes #1127` 仅在全部 invariants 完成时关闭 Issue。

## Handoff Notes

- 当前没有 GH1127 实现；不要从 GH1128 的输入扫描工作推断输出审核模式。
- 禁止先发送后审核或逐独立 chunk 扫描。
- `HD-1127-1` 已批准为 `windowed_cumulative`；不得改成其他模式或扩大配置范围。
- 不自动合并、不 force-push。
