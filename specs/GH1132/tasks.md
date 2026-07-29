# Task Plan

## Linked Issue

GH-1132 / #1132

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [x] `SP1132-T0` Covers: B-001 ～ B-008. Owner: maintainer/spec owner. Dependencies: none. Done when: 维护者在 `user-2026-07-27-approve-all-specs` 批准 dev-only embedded source + explicit zero fallback 行为，Issue 已添加 `ready_to_implement`. Verify: SpecRail workflow/spec checks与 implement route gate。

## 实现任务

- [x] `SP1132-T1` Covers: B-001 ～ B-005. Owner: config implementation owner. Dependencies: SP1132-T0. Done when: dev example 使用 embedded source、`allow_unpriced`、`0.0` fallback、`allow_degraded: false`，production example 不变且注释限定 dev scope. Verify: focused YAML field assertions and production regression.
- [x] `SP1132-T2` Covers: B-006, B-007, B-008. Owner: config test owner. Dependencies: SP1132-T1. Done when: 两个 shipped examples 都从 manifest-stable path parse/deny unknown/validate，所有 enabled dev models 满足 priced-or-explicit-fallback. Verify: focused config conformance tests from non-repo cwd.
- [x] `SP1132-T3` Covers: B-002, B-003, B-004. Owner: pricing test owner. Dependencies: SP1132-T2. Done when: dev vLLM 使用显式 zero fallback 而非 `model_not_priced`，embedded 已知模型使用真实 price，加载失败不静默归零. Verify: focused spend/pricing service tests without network.
- [ ] `SP1132-T4` Covers: B-001 ～ B-008. Owner: verification owner. Dependencies: SP1132-T3. Done when: Claude 保留 diff 按规格修正，完整 Rust/SpecRail gate 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。配置、conformance helper 和 pricing regression 共享相同示例事实，由一个
owner 在 `.claude/worktrees/agent-a15f4c7ae5c967721` 串行完成。

## 验证

- 不只断言 `UnpricedModelPolicy` enum；必须遍历所有 enabled models。
- 不访问网络，不依赖执行 cwd。
- 不削弱 production example 的 `reject` 断言。
- mixed PR 最终使用 `Fixes #1132`。

## Handoff Notes

- 实现已通过 production reservation helper 覆盖实际 unpriced zero fallback 与
  known-model non-fallback；最终 head 仍须保留 exact-head 全量验证证据。
- 不把 `allow_degraded` 当成 missing model price 的修复。
- 不自动合并、不 force-push。
