# Product Spec

## Linked Issue

GH-1044 / #1044

complexity: medium

## 用户问题

authoritative `api_keys` 表把 permissions、rate limits、usage stats 和 metadata 保存为 JSON text。当前读取转换会把损坏的非空 JSON 静默变成空值或默认值，其中损坏的 key-specific rate limit 会变成 `None`，随后请求限流回退到更宽的 gateway default，或在没有 default 时不应用 RPM 限制。

损坏的 usage stats 还会被当作零值参与 read-modify-write，并在下一次 usage update 时覆盖原始损坏数据，造成计量丢失且隐藏数据完整性问题。

## 目标

- authoritative API-key row 的非空 JSON 字段必须严格解析，损坏时显式失败。
- `NULL` optional field 与“非空但损坏”保持不同语义。
- lookup/list/authentication source 和 usage update 都传播转换错误。
- 错误提供字段上下文但不泄露原 JSON、key hash 或 secret。
- 有效 API-key 的创建、读取、更新、列举和 rate-limit precedence 保持不变。

## 非目标

- 不增加跨 SQLite/PostgreSQL 的 JSON CHECK constraint 或数据修复 migration。
- 不自动修复、清空或删除已有损坏行。
- 不改变有效 key-specific rate limit 与 gateway default 的优先级。
- 不修改 Redis/local rate limiter 的 failure mode。

## Behavior Invariants

1. B-001 非空 `permissions`、`rate_limits`、`usage_stats`、`extra` 必须解析为声明类型；任何 malformed/type mismatch 都返回 typed `GatewayError`，不得使用空/默认 domain value。
2. B-002 optional `rate_limits`/`extra` 只有数据库值为 `NULL` 时才转换为 `None`/empty metadata；非空损坏值不得与缺失值等价。
3. B-003 authoritative hash/id lookup 遇到损坏 row 必须返回错误而不是 `Ok(Some(ApiKey))`，authentication 因此不能接收丢失 key-specific policy 的有效 key。
4. B-004 user/team/global list 遇到任一损坏 row 必须传播错误，不得丢弃该 row、返回部分列表或默认字段。
5. B-005 `update_api_key_usage` 遇到损坏 persisted usage/policy 必须回滚并保持原 row 不变，不得以零 counters 覆盖损坏数据。
6. B-006 domain-to-entity JSON serialization 必须传播错误，不得写入 `[]`、`{}` 或 `NULL` fallback；正常 valid values 保持 round-trip。
7. B-007 conversion error 只包含 field context 与 parser category，不包含 raw JSON、key hash、key prefix 或 secret；有效 create/read/update/list 行为不变。

## 验收标准

- [ ] malformed non-null rate limits 的 hash/id lookup 显式失败，不能形成 `rate_limits: None` 的 ApiKey。
- [ ] malformed permissions、usage stats、extra 均显式失败。
- [ ] list 查询遇到损坏 row 不返回 partial/default result。
- [ ] usage update 遇到损坏 row 返回错误且数据库原值未改变。
- [ ] 错误断言证明包含字段名但不包含 fixture 原文或 key identity/secret。
- [ ] valid `NULL` optionals 与 valid JSON round-trip 保持原行为。
- [ ] focused DB/auth tests、格式、全特性编译、strict Clippy 和全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-002；required 空字符串损坏，optional NULL 保持缺失语义。 |
| 错误与失败路径 | covered: B-001 至 B-006；所有 conversion consumer 传播同一错误。 |
| 授权/权限 | covered: B-003；损坏 policy 不得形成可认证 domain key。 |
| 并发/竞态 | covered: B-005；既有 transaction/optimistic lock 保留，转换失败先于写入。 |
| 重试/幂等 | covered: B-005；重复读取持续失败且不修改损坏 row。 |
| 非法状态转换 | covered: B-003, B-005；corrupt persisted state 不转成 valid/default domain state。 |
| 兼容/迁移 | covered: B-002, B-007；valid/null rows 不需迁移，损坏 rows 从静默降级变显式错误。 |
| 降级/回退 | covered: B-001 至 B-006；禁止 JSON fallback 与 partial list。 |
| 证据与审计完整性 | covered: B-005, B-007；损坏原值保留，错误不泄密。 |
| 取消/中断 | covered: B-005；transaction 在 conversion error 时不提交。 |

## 发布说明

API-key 持久化 policy 或 usage JSON 损坏时现在会显式失败；key-specific 限流、权限和计量数据不再被静默置空或重置。
