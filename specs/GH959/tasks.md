# Task Plan

## Linked Issue

GH-959 / #959

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP959-T1` Owner: coordinator. Dependencies: none. Done when: GH959 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work、write-spec 与 implement route gates 为 allowed。Verify: packet and route-gate JSON。
- [x] `SP959-T2` Owner: auth-regression. Dependencies: T1. Done when: Redis ACL 测试真实拒绝 cache `DEL`，数据库撤销成功且 readable stale active snapshot 让旧认证实现失败。Verify: focused red test artifact。
- [x] `SP959-T3` Owner: auth-implementation. Dependencies: T2. Done when: live/detailed verification 只读取数据库权威 key 记录，cache 不再参与认证读取或填充，现有 owner/expiry/last-used 语义保持。Verify: focused API-key tests。
- [x] `SP959-T4` Owner: verification. Dependencies: T3. Done when: fmt、diff、all-features check、strict clippy、串行全量 tests、scope/overlap guards 与 packet validation 全部通过。Verify: commands below。
- [ ] `SP959-T5` Owner: coordinator. Dependencies: T4. Done when: final PR 使用 `Closes #959`，current-head 独立安全 reviewer、CI、review threads、PR gate、runtime gate、merge 与 issue closure 均远端确认。Verify: SpecRail PR evidence and closure audit。

## 并行拆分

认证 lookup 与回归测试共享 API-key module，按 W-14 由 coordinator 串行修改。planner/reviewer lane 只读；共享验证由 coordinator 独占。

## 验证

- `REDIS_URL=redis://127.0.0.1:<isolated-port> cargo test --all-features --locked --lib auth::api_key::tests::revoked_key_is_rejected_when_cache_delete_fails -- --exact --nocapture`
- focused API-key module tests
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-features --locked -- --test-threads=1`
- `bash scripts/guards/check_pr_scope.sh origin/main`
- `bash scripts/guards/check_pr_overlap.sh`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH959"`

## Handoff Notes

- 不修改 #958 owner contract、#960 public infrastructure error mapping 或 #961 schema semantics。
- Redis cache invalidation 继续用于兼容性清理，但不再是认证安全保证。
- 所有旧 cache-first 实例必须排空后，才能依赖即时撤销保证。
