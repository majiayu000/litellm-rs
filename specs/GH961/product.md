# Product Spec

## Linked Issue

GH-961 / #961

complexity: large

## 用户问题

user-owned API key 通过非空 `user_id` 记录 owner，但当前 owner 删除会把该字段置为
`NULL`。转换后的 key 与主动创建的 ownerless/global key 无法区分，可能在 owner 生命周期
结束后继续作为全局凭证认证。该状态转换丢失 provenance，之后的 runtime 校验无法补救。

## 目标

- 明确 owner 删除采用 fail-closed 的限制语义：仍有 user-owned key 时拒绝删除 owner。
- 保证 user-owned key 不会因 owner 删除被转换成 ownerless/global key。
- 保持主动创建的 ownerless/global key 和没有 key 的用户现有行为。
- 让新安装和存量数据库升级后的行为一致，并在迁移失败时保留原数据和 schema。
- 保留现有 non-active owner 的认证拒绝语义。

## 非目标

- 不新增公开的 canonical-user 删除 API、批量 revoke API 或审计 provenance 字段。
- 不自动删除、撤销或重新归属 user-owned key。
- 不猜测或改写升级前已经是 `user_id = NULL` 的历史记录。
- 不改变 key hash、权限、过期、缓存或 last-used 语义。
- 不收敛遗留 `deployment/scripts/init-db.sql` 与 SeaORM UUID schema 的类型、列或 global-key
  能力差异；本 issue 仅将该已文档化部署入口的 API-key owner 删除动作对齐为 `RESTRICT`。

## Behavior Invariants

1. `B-001`：当一个 user 仍被至少一个 user-owned API key 引用时，任何 canonical owner
   删除都必须失败；删除失败后 user 与所有关联 key 保持不变。
2. `B-002`：owner 删除不得把 API key 的 `user_id` 从非空转换为空；成功提交的持久化状态
   中不得出现由 owner 删除新产生的 ownerless key。
3. `B-003`：没有 user-owned API key 的 user 仍可删除；限制范围不得扩大到无依赖记录的
   owner。
4. `B-004`：主动创建且原本 `user_id = NULL` 的 ownerless/global key 在迁移、owner 删除
   失败或无关 user 删除后保持原值和现有认证语义。
5. `B-005`：存量数据库升级必须逐字段保留全部 API key 记录，并建立与新安装相同的 owner
   删除契约；无法满足约束的数据必须使升级失败，不得静默置空、删除或降级。
6. `B-006`：schema 迁移与 migration ledger 更新是原子的；创建替代表、复制数据、切换
   schema、重建索引或 ledger 写入任一步失败时，不得留下两者不一致的可运行状态。
7. `B-007`：并发 key 创建与 owner 删除不得产生 orphan key；最终只能是 key 创建成功且
   owner 删除失败，或 owner 删除成功且该 owner 下的 key 创建失败。
8. `B-008`：owner 为 inactive、pending、suspended、deleted 或查询不到时，已有认证入口继续
   fail closed；schema 修复不得把这些状态解释成 global key。
9. `B-009`：升级前已经为 `user_id = NULL` 的记录保持不变。由于现有 schema 已丢失
   provenance，本变更不得把其中任意记录猜测为历史 orphan 并删除或重新归属。
10. `B-010`：重复执行正常启动或在删除失败后重试，不得改变 key owner、生成重复 schema
    对象或把失败伪装成成功。

## 验收标准

- [ ] 新安装与 SQLite 存量升级后的 user-owned key 都阻止 owner 删除。
- [ ] PostgreSQL SeaORM migration 与已文档化 legacy bootstrap 都应用 `ON DELETE RESTRICT`。
- [ ] 升级前后的 owned/global key 数据逐字段一致，关键唯一索引仍生效。
- [ ] 删除无 key 的 user 成功；删除有 key 的 user 失败且不改变 user/key。
- [ ] 现有 missing/inactive owner 拒绝与 ownerless/global key 成功测试继续通过。
- [ ] 格式、check、strict clippy、全量测试、scope/overlap 与 SpecRail gates 通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-004, B-009；`user_id = NULL` 是有意的 global 状态，不按缺失 owner 处理 |
| 错误与失败路径 | covered: B-001, B-005, B-006, B-008 |
| 授权/权限 | N/A：本 issue 约束持久化完整性，不新增删除入口或授权策略 |
| 并发/竞态 | covered: B-007 |
| 重试/幂等 | covered: B-010 |
| 非法状态转换 | covered: B-002, B-008 |
| 兼容/迁移 | covered: B-004, B-005, B-009 |
| 降级/回退 | covered: B-005, B-006；约束无法建立时启动迁移失败，不允许成功降级 |
| 证据与审计完整性 | covered: B-005, B-009；不伪造已丢失的 owner provenance |
| 取消/中断 | covered: B-006 |

## 发布说明

这是数据库完整性收紧。升级后，删除仍拥有 API key 的 canonical user 会返回存储错误；维护者
必须先删除这些 key。原生 ownerless/global key 不受影响。历史上已经变成
`user_id = NULL` 的记录无法可靠区分，本次升级保持其原状。
