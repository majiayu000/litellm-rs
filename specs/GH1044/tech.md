# Tech Spec

## Linked Issue

GH-1044 / #1044

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Entity conversion | `src/storage/database/entities/api_key.rs` | `.ok()`/default fallbacks erase JSON parse and serialization failures | B-001/B-002/B-006 root cause |
| DB operations | `src/storage/database/seaorm_db/api_key_ops.rs` | lookup/list/update usage call an infallible converter | B-003/B-004/B-005 propagation gap |
| Auth source | `src/auth/api_key/creation.rs` | verification intentionally reads authoritative DB | B-003 security boundary |
| RPM selection | `src/server/middleware/rate_limit_key_policy.rs` | absent key RPM falls back to gateway default | malformed rate limit becomes policy bypass/weakening |
| DB schema | `src/storage/database/migration/m20240101_000005_create_api_keys_table.rs` | JSON values are TEXT, optional fields may be NULL | Must distinguish NULL from corrupt non-null text |

## 设计方案

1. 将 `api_key::Model::to_domain_api_key` 改为 `Result<ApiKey>`。用单一 field-aware helper 严格解析每个 JSON field，并把 serde error 映射为 `GatewayError::Serialization` 或 `Validation`，消息只包含稳定字段名与 serde category/line-column，不拼接原值。
2. optional fields 使用 `Option::map(parse).transpose()`：数据库 `NULL` 保持 `None`，non-null parse error 直接返回；valid empty object/array 保持合法值。
3. 将 `from_domain_api_key` 改为 `Result<ActiveModel>`，permissions、rate limits、usage stats、extra 全部使用 `?`/`transpose()`，删除序列化 fallback。
4. 在 `api_key_ops.rs` 的 create、hash/id lookup、user/team/global list、update usage 中传播 `?`：single optional 使用 `map(...).transpose()`，list 使用 iterator `collect::<Result<Vec<_>>>()`。
5. usage update 保持现有 transaction 和 optimistic lock；conversion 在任何 counter mutation/UPDATE 之前失败，使 transaction drop/rollback 且 row 原值不变。
6. 新建独立 `api_key_corruption_tests.rs`（避免继续扩张 558 行的 `api_key_ops.rs`），复用内存 SQLite migration，插入 valid key 后用 entity update 注入 malformed fields，验证 lookup/list/update fail closed 与 row preservation。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | strict entity parser | per-field malformed/type-mismatch conversion tests |
| B-002 | optional `map(...).transpose()` | NULL optional valid test vs non-null corrupt error test |
| B-003 | hash/id lookup propagation | in-memory DB corrupt rate-limit lookup returns error; no ApiKey produced |
| B-004 | list iterator result collection | list with corrupt row returns error, not partial Vec |
| B-005 | usage transaction conversion order | corrupt usage update errors; raw column remains byte-for-byte unchanged |
| B-006 | fallible domain-to-entity conversion | valid round-trip plus source/diff assertion that fallback calls are absent |
| B-007 | error construction and valid regression suite | field-name present; sentinel JSON/key data absent; existing CRUD/full tests pass |

## 数据流

Database row → strict field parser → `Result<ApiKey>` → lookup/list/auth consumer. Only a fully valid row crosses into the domain/auth boundary. Domain write flow serializes all fields before creating an `ActiveModel`; any failure aborts before SQL. Usage update reads and validates the full row before mutating counters and executing UPDATE.

## 备选方案

- malformed rate limit 继续当 `None`，只写 warning：仍允许请求使用更宽 fallback，违反 B-003，拒绝。
- 只严格解析 rate limits：permissions/usage/extra 仍隐藏同一持久化损坏根因，违反 B-001，拒绝。
- 在 migration 中自动清空损坏 JSON：造成不可逆数据丢失且跨数据库约束复杂，超出本 issue，拒绝。
- list 跳过损坏 row：返回不完整管理视图并隐藏数据，违反 B-004，拒绝。

## 风险

- Compatibility: 已损坏的 rows 从可读默认值变为显式错误；这是预期 fail-closed 行为。
- Availability: 单个损坏 row 会使包含它的 list 失败，避免 partial truth；管理员需在数据库层修复。
- Security: error message 不能包含 raw JSON、hash/prefix/secret；测试使用 sentinel 验证。
- Transaction: usage update 必须在 SQL UPDATE 前转换，避免错误后仍写入。

## 测试计划

- [ ] Entity/DB: malformed permissions/rate_limits/usage_stats/extra each fail with redacted field context。
- [ ] Optional: NULL rate_limits/extra remain valid; malformed non-null values fail。
- [ ] Auth source: hash/id lookup on corrupt rate limits returns error, not domain key。
- [ ] List: corrupt row makes user/team/global list error without partial result。
- [ ] Usage: corrupt usage update returns error and raw column remains unchanged。
- [ ] Regression: valid create/read/update/list round-trip plus format/check/clippy/full tests。

## 回滚方案

不得恢复 malformed JSON 的 permissive fallback。若兼容问题暴露历史坏数据，应提供独立的只读诊断或显式 repair 工具；紧急 forward-fix 可缩小错误上下文，但必须继续阻止损坏 row 进入 auth/domain boundary。
