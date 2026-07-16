# Product Spec

## Linked Issue

GH-1059 / #1059

complexity: medium

## 用户问题

`SeaOrmTeamRepository::delete` 忽略 canonical `teams` 与 compatibility `um_teams` 两个 delete 的 affected-row
结果。完全不存在的 ID 会提交 no-op transaction 并返回 `Ok(())`，而 in-memory repository 返回
`GatewayError::NotFound`。

SeaORM bridge 还支持删除仅存在于 `um_teams` 的 legacy-only team，因此不能只用 canonical row 判断存在性；同时
member rows 先于 team rows 删除，missing-team 判定必须回滚该清理，避免错误请求产生副作用。

## 目标

- canonical 与 legacy representation 均不存在时，delete 返回 `NotFound`。
- canonical 或 legacy 任一 representation 存在时，delete 继续成功。
- missing delete 回滚 transaction 内 member cleanup，不修改 legacy user membership。
- bridged、canonical-only、legacy-only 删除和既有 membership cleanup 行为保持。
- SeaORM 与 in-memory missing-delete contract 一致。

## 非目标

- 不增加 schema/foreign-key migration。
- 不改变 delete 为幂等成功，也不引入 soft delete。
- 不修复或自动删除 pre-existing orphan member rows。
- 不改变 create/update/member operations。
- 不重设计 legacy/canonical bridge 或 user-membership 模型。

## Behavior Invariants

1. B-001 canonical `teams` 与 legacy `um_teams` delete 都影响 zero rows 时，repository 回滚 transaction 并返回 `GatewayError::NotFound`。
2. B-002 missing-team transaction 中先执行的 `team_members` cleanup 必须回滚；pre-existing orphan member row 不得因本次请求被删除。
3. B-003 missing delete 不得执行 transaction 后的 legacy user-membership cleanup，用户 `teams` 数据保持不变。
4. B-004 canonical-only、legacy-only 或 bridged team 任一 representation 被删除时 operation 成功并提交相应 team/member 删除。
5. B-005 successful delete 后既有 legacy user-membership cleanup 继续执行，合法成员不保留被删 team ID。
6. B-006 SQL/transaction/rollback error 继续传播 typed database/storage error，不得伪装成 NotFound 或成功。
7. B-007 missing behavior 与 `InMemoryTeamRepository` 一致，并关闭 manager 预读后并发删除产生的 false success。

## 验收标准

- [ ] 完全不存在的 UUID delete 返回 `GatewayError::NotFound`。
- [ ] missing delete 后 orphan member 与 legacy user membership 未被修改。
- [ ] canonical/bridged team delete 继续删除 team、members 与 legacy membership。
- [ ] legacy-only team delete 继续成功。
- [ ] 格式、全 target/feature 编译、strict Clippy 与全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001；missing UUID 是核心输入。 |
| 错误与失败路径 | covered: B-001, B-006；NotFound 与 database failure 区分。 |
| 授权/权限 | N/A；repository contract 不改变 API/RBAC。 |
| 并发/竞态 | covered: B-007；write boundary 检测 read 后并发删除。 |
| 重试/幂等 | covered: B-001, B-004；首次合法删除成功，再次删除 NotFound。 |
| 非法状态转换 | N/A；不改变 lifecycle state。 |
| 兼容/迁移 | covered: B-004, B-005；legacy-only 与 bridged delete 保持，无 migration。 |
| 降级/回退 | covered: B-001；禁止 missing no-op 降级为成功。 |
| 证据与审计完整性 | covered: B-002, B-003, B-007；失败响应对应零已提交副作用。 |
| 取消/中断 | covered: B-001, B-006；rollback failure 显式传播。 |

## 发布说明

团队删除现在会在 canonical 与 legacy 两侧都不存在目标时返回 NotFound，并回滚预先执行的成员清理。
