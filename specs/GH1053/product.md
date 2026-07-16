# Product Spec

## Linked Issue

GH-1053 / #1053

complexity: low

## 用户问题

canonical team repository 为兼容旧数据，会在查询前枚举并同步 `um_teams`。当前枚举逻辑遇到无法反序列化的
`um_teams.data` 时只记录 warning 并跳过该行，随后继续返回成功结果。因此持久化损坏会被伪装成“团队不存在”，
并使 list、count、name lookup 与 user-team 查询返回不完整数据。

同一存储层的单条 legacy team 读取和 legacy `list_teams` 已经会传播反序列化错误；canonical compatibility path
的 silent skip 形成了不一致的读契约。

## 目标

- legacy team 枚举采用 all-or-error 语义，任何不可解码 row 都中止查询。
- 所有依赖该枚举的 canonical team 查询传播同一错误，不返回 partial/default 结果。
- 错误指出 persisted `um_teams.data` contract 损坏，但不回显原始 payload。
- 合法 legacy/canonical team 的同步、分页、计数和成员行为保持不变。

## 非目标

- 不增加 schema migration、row quarantine、自动修复或删除损坏数据。
- 不改变 inactive/invalid legacy member 的转换规则。
- 不重设计 legacy/canonical 双写或同步架构。
- 不修改其他 JSON-backed entity 的错误策略。
- 不改变对合法 team name conflict 的既有处理。

## Behavior Invariants

1. B-001 `list_legacy_um_teams` 对全部 selected rows 成功反序列化后才返回 teams；任一 `data` row 不合法时返回 `Err`，不得 warning + skip。
2. B-002 canonical `list`、`count`、`get_by_name`、legacy synchronization 和 `get_user_teams` 必须传播枚举错误，不得返回已处理 prefix、空列表或错误 total。
3. B-003 混合 valid 与 corrupt legacy rows 时仍整体失败；valid peer 不得掩盖 corruption 或形成 partial truth。
4. B-004 corruption error 使用 typed `GatewayError` 并包含 `um_teams.data` field context，但不得包含 raw persisted payload。
5. B-005 valid legacy rows 继续同步为 canonical teams/members，合法 canonical pagination、count、name lookup 与 user-team 查询结果不变。
6. B-006 单条 `get_legacy_um_team` 的既有 fail-closed 行为保持；本次不扩展或收紧 member conversion、name-conflict、repair 与 migration 语义。

## 验收标准

- [ ] 一个 corrupt legacy row 使 canonical list 返回错误，而不是成功空列表。
- [ ] valid + corrupt 混合 rows 仍返回错误且不暴露 partial list/total。
- [ ] count、name lookup 与 user-team synchronization 传播相同 corruption failure。
- [ ] 错误包含 `um_teams.data` context 且不包含测试用 raw corrupt payload。
- [ ] valid legacy/canonical team repository tests 保持通过。
- [ ] 格式、全 target/feature 编译、strict Clippy 与全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-004；空 `data` 是不可解码 persisted row，必须报错。 |
| 错误与失败路径 | covered: B-001 至 B-004；corruption 显式传播且禁止 partial truth。 |
| 授权/权限 | N/A；本 issue 只收紧 repository decode contract，不改变 RBAC。 |
| 并发/竞态 | covered: B-005；不改变同步顺序、事务或并发冲突处理。 |
| 重试/幂等 | covered: B-001, B-003；损坏未修复前重复查询稳定失败。 |
| 非法状态转换 | N/A；没有 lifecycle state transition。 |
| 兼容/迁移 | covered: B-005, B-006；合法旧 row 继续兼容，不执行 migration/repair。 |
| 降级/回退 | covered: B-001 至 B-003；明确禁止 skip、空列表和 partial fallback。 |
| 证据与审计完整性 | covered: B-003, B-004；调用者能区分真实空数据与持久化损坏。 |
| 取消/中断 | N/A；同步查询没有独立取消状态。 |

## 发布说明

团队兼容查询遇到损坏的 legacy JSON 时现在会显式失败，不再成功返回缺失团队或错误计数。
