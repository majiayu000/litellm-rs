# Tech Spec

## Linked Issue

GH-1056 / #1056

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| SeaORM update | `src/storage/database/seaorm_db/team_repository/repository_impl.rs` | discards `ExecResult`, always syncs and returns team | B-001/B-002 root cause |
| In-memory contract | `src/core/teams/repository.rs` | checks ID and returns `NotFound` | expected parity reference |
| Manager | `src/core/teams/manager.rs` | reads then calls repository update | concurrent delete leaves write race |
| SeaORM tests | `src/storage/database/seaorm_db/team_repository_tests.rs` | covers successful updates, not missing update | regression gap |

## 设计方案

1. 保存 `self.db.db.execute(stmt)` 返回的 `ExecResult`，继续用既有 `GatewayError::from` 映射 SQL execution failure。
2. 在任何 legacy sync 之前检查 `rows_affected()`。值为 `0` 时返回
   `GatewayError::NotFound(format!("Team {} not found", team.id()))`，与 in-memory repository 的语义/文案类别一致。
3. 值为 `1` 时执行现有 `sync_legacy_team_from_canonical` 并返回 touched team，不改变成功路径顺序。
4. `teams.id` 是主键，因此单条 ID update 不会合法影响多行；实现不增加不可达 recovery 分支。若未来 schema 破坏该约束，数据库本身应先失败。
5. 在 SeaORM repository tests 中直接 update 未创建 team，断言 `NotFound`；随后通过 `get` 与 legacy `get_team` 断言没有副作用。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | ExecResult zero-row check | missing update returns exact error variant |
| B-002 | early return before legacy sync | canonical and legacy lookup remain `None` |
| B-003 | unchanged one-row path | existing successful canonical update tests |
| B-004 | unchanged sync call | organization/settings preservation tests |
| B-005 | existing execute error mapping | source inspection plus repository/full tests |
| B-006 | parity with in-memory behavior | missing-update regression and contract comparison |

## 数据流

Touched `Team` → parameterized SQL update → `ExecResult.rows_affected()`；`0` → `NotFound` early return；`1` → existing
canonical-to-legacy sync → returned persisted team。SQL error 在 row-count branch 之前传播。

## 备选方案

- 依赖 TeamManager 预读：无法关闭 read/update 间并发删除窗口，且 repository 可被直接调用，拒绝。
- update 前再 SELECT：增加一次 round trip 且仍存在 TOCTOU，拒绝。
- zero rows 时 INSERT/upsert：会把 update 变成 create 并绕过 create validation，拒绝。
- zero rows 继续 warning + success：仍确认未持久化写入，拒绝。
- 本次引入 optimistic version：影响 schema/API 与所有 writers，超出 issue。

## 风险

- Contract tightening: 依赖错误 no-op success 的直接 repository caller 将收到 NotFound；这是目标修复。
- Backend semantics: SQLite/PostgreSQL 对按主键匹配的 update 都提供 affected row count；focused SQLite 与全量编译覆盖实现。
- Side effects: check 必须位于 legacy sync 之前，防止 missing update 触发任何兼容写入。
- Scope drift: legacy user-management update 仍有独立 zero-row behavior，本 issue 不触碰。

## 测试计划

- [ ] Red: current SeaORM repository update 未创建 team 返回 `Ok(Team)`。
- [ ] Missing: after fix 返回 `GatewayError::NotFound`。
- [ ] No side effect: canonical `get` 与 legacy `get_team` 对 ID 都为 `None`。
- [ ] Success: existing update/preservation tests pass。
- [ ] Repository: complete team repository module、format、all-target/all-feature check、strict Clippy、full serial tests。

## 回滚方案

不得恢复 zero-row success。若 affected-row semantics 在某 backend 出现差异，应增加 backend-specific regression 并修正检测方式；不可通过忽略
`ExecResult` 回退。
