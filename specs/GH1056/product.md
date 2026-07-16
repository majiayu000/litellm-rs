# Product Spec

## Linked Issue

GH-1056 / #1056

complexity: low

## 用户问题

`SeaOrmTeamRepository::update` 丢弃 SQL `UPDATE` 的执行结果。目标 ID 不存在时数据库返回 zero affected rows，repository
却继续执行 legacy sync 并返回 `Ok(Team)`，使调用方认为更新已持久化。内存 repository 对相同输入已经返回
`GatewayError::NotFound`，两种实现 contract 不一致。

TeamManager 虽然通常先读取 team，但 read 与 update 之间存在并发删除窗口；因此 write boundary 必须验证自己的执行结果。

## 目标

- canonical team update 在 zero affected rows 时返回 `NotFound`。
- zero-row update 不执行 legacy synchronization，也不返回未持久化 team。
- exactly-one-row update 保持既有 metadata touch、持久化和 legacy sync 行为。
- SeaORM 与 in-memory `TeamRepository::update` 的 missing-team contract 一致。

## 非目标

- 不增加 transaction、version compare 或完整 optimistic locking。
- 不改变 create/delete/member operations。
- 不修改 legacy `user_management::update_team`。
- 不改变 name conflict、soft-delete 或 API route policy。
- 不重设计 manager 的 read-modify-write 流程。

## Behavior Invariants

1. B-001 canonical `UPDATE teams ... WHERE id = ?` 影响 zero rows 时，SeaORM repository 返回 `GatewayError::NotFound`，不得返回输入 team。
2. B-002 zero-row update 后 canonical `teams` 与 legacy `um_teams` 均不得出现该 ID；legacy synchronization 不得产生副作用。
3. B-003 exactly one matched row 时，更新后的 name/data 被持久化，metadata touch 与返回值保持既有行为。
4. B-004 exactly one matched row 后继续执行既有 canonical-to-legacy synchronization，organization/settings preservation contract 不变。
5. B-005 SQL execution error 继续映射为既有 typed database/storage error；不得转换成 `NotFound` 或成功。
6. B-006 missing-team behavior 与 `InMemoryTeamRepository` 一致，并关闭 manager 预读后 row 被删除所产生的 false success 窗口。

## 验收标准

- [ ] 从未 create 的 team 调用 SeaORM `update` 返回 `GatewayError::NotFound`。
- [ ] missing ID 在 canonical 与 legacy table 中仍不存在。
- [ ] existing team update 继续持久化字段并返回更新对象。
- [ ] existing legacy synchronization/preservation tests 保持通过。
- [ ] 格式、全 target/feature 编译、strict Clippy 与全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-002；missing ID 是本 issue 的核心输入。 |
| 错误与失败路径 | covered: B-001, B-005；zero row 与 SQL error 明确区分。 |
| 授权/权限 | N/A；repository persistence contract 不改变 route/RBAC。 |
| 并发/竞态 | covered: B-006；write result 关闭 read 后并发删除的 false success。 |
| 重试/幂等 | covered: B-001, B-003；missing update 稳定 NotFound，existing update 维持既有语义。 |
| 非法状态转换 | N/A；不改变 team lifecycle transition policy。 |
| 兼容/迁移 | covered: B-003, B-004；无 schema/data migration，合法 update 保持。 |
| 降级/回退 | covered: B-001；禁止 zero-row no-op 降级为成功。 |
| 证据与审计完整性 | covered: B-002, B-006；成功响应必须对应真实 persisted row。 |
| 取消/中断 | N/A；没有独立取消状态。 |

## 发布说明

团队更新现在会在目标已不存在时返回 NotFound，不再确认一个未写入数据库的成功结果。
