# Task Plan

## Linked Issue

GH-967 / #967

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP967-T1` Owner: coordinator. Dependencies: none. Done when: GH967 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work 与 write-spec/implement route gates 为 allowed。Verify: packet and route-gate JSON。
- [x] `SP967-T2` Owner: catalog-contract. Dependencies: T1. Done when: `ProviderDefinition` 持有显式 static capability slice，所有 Tier-1 entry 选择命名 profile，alias 仍解析到 canonical definition。Verify: full-catalog conformance tests。
- [x] `SP967-T3` Owner: runtime-instance. Dependencies: T2. Done when: OpenAI-like instance 保存 exact slice，constructor 拒绝空、重复或不属于当前 profile 的 capability，三条 catalog 构造路径传递声明，direct constructor 使用保守 profile，显式 `openai_compatible` 使用 proxy profile。Verify: provider/factory/default-router tests。
- [x] `SP967-T4` Owner: route-conformance. Dependencies: T3. Done when: catalog deployment 的 chat route 可选，`ImageEdit`、`ImageVariation`、`Moderation` 在执行前返回 `UnsupportedCapability`；显式 proxy route 保持可用；每个 declared capability 都通过 method-surface conformance。Verify: focused router 与 image/moderation integration tests。
- [x] `SP967-T5` Owner: verification. Dependencies: T2-T4. Done when: fmt、diff、all-features check、strict clippy、scope/overlap guards、串行全量 tests 和 packet validation 全部通过。Verify: commands below。
- [ ] `SP967-T6` Owner: coordinator. Dependencies: T5. Done when: one implementation PR 使用 `Closes #967`，current-head CI、独立 reviewer、review threads、PR gate 与 runtime gate 全部通过并远端确认合并。Verify: SpecRail PR evidence and closure audit。

## 并行拆分

T2-T4 共享 definition -> factory -> instance -> route contract，按 W-14 串行实现。planner/reviewer lane
只读；共享全量验证由 coordinator 独占。

## 验证

- `cargo fmt --all -- --check`
- `git diff --check`
- focused catalog/provider/factory/router capability tests
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `bash scripts/guards/check_pr_scope.sh`
- `bash scripts/guards/check_pr_overlap.sh`
- `cargo test --all-features --locked -- --test-threads=1`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH967"`

## Handoff Notes

- 不修改全局 trait lifetime，不使用 `Any`/downcast。
- support matrix 是 public route surface，不替代 provider method capability。
- 不根据第三方文档猜测 image/moderation support；当前 profile 只声明 gateway 已实现的 surface。
