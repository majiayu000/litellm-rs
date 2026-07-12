# Task Plan

## Linked Issue

GH-958 / #958

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP958-T1` Owner: coordinator. Dependencies: none. Done when: GH958 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work、write-spec 与 implement route gates 为 allowed。Verify: packet and route-gate JSON。
- [x] `SP958-T2` Owner: auth-contract. Dependencies: T1. Done when: 回归测试先证明 missing owner 与 inactive owner 在 live/detailed 两入口均被错误接受或语义漂移，并固定 ownerless 成功语义。Verify: focused failing tests。
- [x] `SP958-T3` Owner: auth-implementation. Dependencies: T2. Done when: 两条验证入口在 last-used 更新前调用同一 owner predicate；missing/non-active 拒绝，active/ownerless 通过，repository error 不被吞掉。Verify: focused auth API-key tests。
- [x] `SP958-T4` Owner: verification. Dependencies: T3. Done when: fmt、diff、all-features check、strict clippy、串行全量 tests、scope/overlap guards与 packet validation 全部通过。Verify: commands below。
- [ ] `SP958-T5` Owner: coordinator. Dependencies: T4. Done when: final PR 使用 `Closes #958`，current-head 独立安全 reviewer、10/10 CI、review threads、PR gate、runtime gate、merge 与 issue closure 均远端确认。Verify: SpecRail PR evidence and closure audit。

## 并行拆分

owner tests 与 helper/call sites 位于同一 `creation.rs`，按 W-14 串行修改。planner/reviewer lane 只读；共享验证由 coordinator 独占。

## 验证

- `cargo fmt --all -- --check`
- `git diff --check`
- focused missing/inactive/ownerless live/detailed tests
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-features --locked -- --test-threads=1`
- `bash scripts/guards/check_pr_scope.sh origin/main`
- `bash scripts/guards/check_pr_overlap.sh`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH958"`

## Handoff Notes

- 不修改 Redis authorization cache；见 #959。
- 不修改 user deletion 或 API-key FK；见 #961。
- 普通认证结果保持通用 invalid key，不暴露 owner missing/inactive。
