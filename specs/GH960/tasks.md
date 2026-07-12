# Task Plan

## Linked Issue

GH-960 / #960

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP960-T1` Owner: coordinator. Dependencies: none. Done when: GH960 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work、write-spec 与 implement route gates 为 allowed。Verify: packet and route-gate JSON。
- [x] `SP960-T2` Owner: auth-regression. Dependencies: T1. Done when: 真实关闭 database pool 后，回归测试证明旧 `AuthSystem` 把 API-key storage error 错误折叠成 failed `AuthResult`。Verify: focused red test artifact。
- [x] `SP960-T3` Owner: auth-implementation. Dependencies: T2. Done when: verifier `Err` 原样传播；middleware 与 keys route 记录内部详情并使用共享固定消息返回通用 500；invalid key 仍为 401，重复 outage 不触发 auth lockout。Verify: focused auth/middleware/keys tests。
- [ ] `SP960-T4` Owner: verification. Dependencies: T3. Done when: fmt、diff、all-features check、strict clippy、串行全量 tests、scope/overlap guards 与 packet validation 全部通过。Verify: commands below。
- [ ] `SP960-T5` Owner: coordinator. Dependencies: T4. Done when: final PR 使用 `Closes #960`，current-head 独立安全 reviewer、CI、review threads、PR gate、runtime gate、merge 与 issue closure 均远端确认。Verify: SpecRail PR evidence and closure audit。

## 并行拆分

error propagation、两个 HTTP mapping 与其测试共享认证 contract，按 W-14 由 coordinator 串行修改。planner/reviewer lane 只读；共享验证由 coordinator 独占。

## 验证

- closed-pool `AuthSystem` infrastructure-error propagation test
- middleware and keys generic-500 helper tests
- existing invalid API-key 401 middleware regression
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-features --locked -- --test-threads=1`
- `bash scripts/guards/check_pr_scope.sh origin/main`
- `bash scripts/guards/check_pr_overlap.sh`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH960"`

## Handoff Notes

- 不修改 JWT/session、#958 owner contract、#959 cache authority 或 #961 schema semantics。
- 原始认证错误只进入服务端日志；HTTP helpers 不接受该错误作为输入。
