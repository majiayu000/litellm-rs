# Task Plan

## Linked Issue

GH-961 / #961

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP961-T1` 在不改变旧 migration 输出的前提下提取 API-key 16 列 table helper，并将最终 entity relation 统一为 `RESTRICT`。Covers: B-001, B-002, B-004, B-008. Owner: coordinator. Dependencies: none. Done when: 旧 migration 仍生成 `SetNull` 并保留原索引创建块，新增 migration 后的最终 SeaORM schema/ORM 为 `RESTRICT`，且 verifier 无行为改动。Verify: `cargo check --all-targets --all-features --locked`。
- [ ] `SP961-T2` 追加 PostgreSQL 命名外键替换和 SQLite 表重建 migration，注册到 repository `Migrator::up/down` 的 SQLite outer transaction，并对齐 legacy bootstrap。Covers: B-001, B-002, B-004, B-005, B-006, B-007, B-009, B-010. Owner: coordinator. Dependencies: SP961-T1. Done when: 两个 SeaORM 后端和 legacy bootstrap 都建立 restrict 语义，SQLite schema/ledger 同事务提交，任何错误传播且不关闭 foreign keys。Verify: focused migration tests + `cargo check --all-targets --all-features --locked`。
- [ ] `SP961-T3` 增加 fresh/带数据 pre-GH961 -> current SQLite/PostgreSQL upgrade、ledger failure 与 PostgreSQL SQL builder 测试。Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-009, B-010. Owner: coordinator. Dependencies: SP961-T2. Done when: 完整 entity equality、owner delete failure、无 key user delete、global key、unique index、dangling/ledger rollback、真实 PostgreSQL upgrade、单步 down、重复 migrate、feature gating 与 legacy bootstrap 均有断言。Verify: `cargo test --lib owner_restrict --all-features --locked -- --nocapture`。
- [ ] `SP961-T4` 运行既有 owner 认证回归与全仓验证。Covers: B-004, B-008. Owner: coordinator. Dependencies: SP961-T1, SP961-T2, SP961-T3. Done when: missing/inactive owner 继续被拒绝，ownerless key 继续成功，fresh checks 全绿。Verify: tech spec 中列出的 focused 与 repository commands。

## 并行拆分

Rust migration、entity 与 migration tests 共享 schema contract，按 W-14 由 coordinator 串行修改。
只读 planner/architecture lane 可并行审查决策，但不拥有文件。最终 reviewer lane 只读审查当前
head。

## 验证

- `python3 checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH961"`
- `cargo fmt --all -- --check`
- `cargo check --all-targets --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --all-features --locked -- --test-threads=1`
- `bash scripts/guards/check_pr_scope.sh origin/main`
- `bash scripts/guards/check_pr_overlap.sh`
- independent current-head reviewer、GitHub CI、review threads、clean merge state、SpecRail
  required PR gate 与 runtime ledger gate 全部通过。

## Handoff Notes

- 维护决策：采用 `RESTRICT`；用户在当前对话明确放开 design/merge human gate，但未放开 CI、
  独立 reviewer、review-thread、merge-state 或 SpecRail gates。
- 历史 `user_id = NULL` provenance 不可恢复，必须原样保留，禁止猜测性清理。
- GH958 已提供 missing/inactive/ownerless runtime tests；本 issue 不重写 verifier。
- 根因复现：当前 `SetNull` schema 下删除 owner 后查询得到 `owned-key|NULL`。
