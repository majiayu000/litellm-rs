# Product Spec

## Linked Issue

GH-1050 / #1050

complexity: medium

## 用户问题

内部 BatchProcessor 使用 Rust `Debug` 文本持久化状态，因此 `InProgress`、`Completed`、`Cancelling` 等值被写入
数据库；读取端只识别 `in_progress`、`completed`、`cancelling` 等 snake_case 值，所有不匹配值又被静默合成为
`Failed`。结果是库自身完成的正常状态更新在重新 list 后变成失败状态。

同一个 `BatchStatus` 的 JSON 序列化也输出 Rust variant casing，而不是 OpenAI-compatible snake_case 状态词汇。
Debug 输出不是稳定的持久化或 API contract，未知值更不能伪造一个终态。

## 目标

- 所有 `BatchStatus` 使用唯一 canonical snake_case 编码作为新持久化与 JSON contract。
- 数据库更新边界接收 typed `BatchStatus`，禁止任意字符串与 `Debug` 格式化。
- 读取 canonical 值时精确恢复原 variant。
- 兼容读取当前实现已产生的历史 Debug spellings，但不得继续写入这些值。
- 真正未知的 persisted status 显式失败，不得变成 `Failed`。
- status-specific timestamp 更新和既有状态转换策略保持不变。

## 非目标

- 不增加 schema migration 或批量重写历史 row。
- 不实现当前缺失的 batch request/result 持久化。
- 不改变 batch metadata、request counts 或 completion-window 行为。
- 不新增或删除 `BatchStatus` variant。
- 不重设计 batch 状态机或允许/禁止的 transition。

## Behavior Invariants

1. B-001 每个声明的 `BatchStatus` 都有唯一 canonical snake_case 文本：`validating`、`failed`、`in_progress`、`finalizing`、`completed`、`expired`、`cancelling`、`cancelled`。
2. B-002 `BatchStatus` 的 JSON serialization/deserialization 使用 canonical snake_case，API response 不输出 Rust Debug variant casing。
3. B-003 create/update persistence 只通过 typed status 的 canonical encoder 写入；`update_batch_status` 不接受任意 `&str`，生产代码不使用 `format!("{:?}", status)`。
4. B-004 list/read 对 canonical 值和当前缺陷已生成的八个 historical Debug spellings 都恢复相同 domain variant；historical 兼容只读、不产生新 legacy 写入。
5. B-005 任何 canonical/historical 闭集之外的 persisted status 返回 typed `GatewayError`，不得映射为 `Failed`、跳过 row 或返回 partial list。
6. B-006 status update 仍只设置对应 variant 的 timestamp；合法状态写入、事务、not-found 与 ordering 行为保持不变。
7. B-007 existing batch domain/API consumers 对合法状态继续工作，唯一可见变化是正确的 snake_case wire value 和不再伪造失败状态。

## 验收标准

- [ ] 八个合法状态分别通过 canonical persistence 和 JSON 精确 round-trip。
- [ ] BatchProcessor 的 `InProgress`、`Completed`、`Cancelling` 更新写入 snake_case 并在 list 时保持原 variant。
- [ ] 八个 historical Debug values 均可读取并映射正确。
- [ ] unknown persisted status 使 list 返回错误且错误不把该 batch 标记为 Failed。
- [ ] production source 不再以 Debug/任意字符串持久化 batch status。
- [ ] status timestamp、missing batch、分页排序与其他合法行为保持。
- [ ] focused SQLite/domain tests、格式、全特性编译、strict Clippy 和全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-005；空 status 是 unknown persisted value，必须失败。 |
| 错误与失败路径 | covered: B-005, B-006；unknown 显式 error，真实 Failed 仍合法。 |
| 授权/权限 | N/A；batch lifecycle 状态不参与 auth/RBAC。 |
| 并发/竞态 | covered: B-006；既有 status transaction 保持，不改变锁或事务边界。 |
| 重试/幂等 | covered: B-001, B-003；重复写同一 typed status 产生相同 canonical 值。 |
| 非法状态转换 | covered: B-003, B-005；字符串绕过被类型边界移除，unknown row 不进入 domain。 |
| 兼容/迁移 | covered: B-004；无需 migration 即可读取历史 Debug 值，新写入收敛到 canonical。 |
| 降级/回退 | covered: B-005；禁止 unknown → Failed fallback 和 partial list。 |
| 证据与审计完整性 | covered: B-004, B-005；真实 persisted lifecycle 不被伪造。 |
| 取消/中断 | covered: B-004, B-006；Cancelling/Cancelled 精确编码，事务语义保持。 |

## 发布说明

内部 batch 状态现在以 OpenAI-compatible snake_case 稳定编码；正常的 processing/completed/cancelling 状态不再在读取时错误显示为 Failed。
