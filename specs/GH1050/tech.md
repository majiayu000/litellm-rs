# Tech Spec

## Linked Issue

GH-1050 / #1050

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Domain status | `src/core/batch/types.rs` | serde uses Rust variant names; no stable text encoder/parser | B-001/B-002 contract gap |
| Processor persistence | `src/core/batch/processor/utils.rs`, `core.rs` | `format!("{:?}", status)` writes PascalCase/compound Debug names | B-003 root cause |
| Database update | `src/storage/database/seaorm_db/batch_ops.rs` | public method accepts arbitrary `&str` and matches timestamps by string | type-safety gap |
| Database list | `src/storage/database/seaorm_db/batch_ops.rs` | recognizes snake_case only; unknown including library-written Debug becomes Failed | B-004/B-005 silent corruption |
| Domain tests | `src/core/batch/types.rs` tests | explicitly expects `InProgress` JSON | current wire mismatch proof |

## 设计方案

1. 在 `BatchStatus` 上设置 `#[serde(rename_all = "snake_case")]`，实现 `as_str() -> &'static str` 作为唯一新写入编码；
   实现 strict persisted parser（`FromStr` 或等价 helper），闭集包含八个 canonical 值及八个 exact historical Debug 值。
2. historical aliases 只存在于 parser。`as_str`、serde serialization 和所有新 DB write 永远只输出 canonical snake_case。
3. 将 `SeaOrmDatabase::update_batch_status` 参数改为 `BatchStatus` 或 `&BatchStatus`。SQL value 使用 `as_str()`，timestamp
   match 使用 enum variant，移除任意 string branch。
4. BatchProcessor 的普通 update 与 cancel 路径直接传 typed enum；必要时 clone，禁止 `Debug`/case conversion。
5. create path 使用 `BatchStatus::Validating.as_str()`；list path 将每个 model 转换为 `Result<BatchRecord>` 并
   `collect::<Result<Vec<_>>>()`，unknown status 传播 `GatewayError::Storage/Validation` 而非返回 Failed/partial list。
6. 错误提供 batch status field/category context；不需要回显 raw unknown value。metadata/request count fallback 留给独立 issue。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | `BatchStatus::as_str` + parser | table-driven eight-variant canonical round-trip |
| B-002 | serde rename contract | exact JSON strings and deserialize round-trip |
| B-003 | typed DB update + processor callsites | compile-time signature; source search has no Debug persistence |
| B-004 | parser legacy aliases | table-driven eight historical spellings and SQLite rows |
| B-005 | fallible list collection | corrupt status row makes list `Err`, valid peer not returned as partial truth |
| B-006 | enum timestamp match | focused update tests assert corresponding timestamp and not-found behavior |
| B-007 | existing processor/domain/full suites | repository-wide tests and minimal scope diff |

## 数据流

`BatchStatus` domain variant → `as_str()` → DB canonical text；DB canonical/historical text → strict parser → domain variant；
domain variant → serde snake_case → API payload。任意不在闭集的 DB 值在进入 response 前失败，不能伪造成 Failed。

## 备选方案

- 对 Debug 文本调用 `to_lowercase()`：`InProgress` 会变为 `inprogress` 而非 `in_progress`，仍错误，拒绝。
- 扩展 list match 只接受 PascalCase：继续同时存在两个新写入 contract，且 API casing 不修，拒绝。
- unknown 继续返回 Failed 并记录 warning：仍伪造终态和 partial truth，违反 B-005。
- migration 立即重写历史 rows：增加部署风险且 typed read compatibility 已足够，超出本 issue。
- 用 serde JSON string 作为 DB status：会引入引号/JSON 层且不如 `as_str` 明确，拒绝。

## 风险

- Compatibility: JSON status 从 Rust casing 改为 OpenAI-compatible snake_case；这是目标行为，但依赖错误 casing 的客户端需调整。
- Historical data: parser 必须完整覆盖八个旧 Debug spelling，尤其 compound variants，避免现有 row 变不可读。
- Error propagation: list 必须原子失败，不能在 iterator 中丢 row 或返回已转换 prefix。
- Timestamp: typed match 不能改变哪个 variant 设置哪个 timestamp。

## 测试计划

- [ ] Red: current processor/DB test 写 `InProgress`、`Completed`、`Cancelling` 后 list，证明它们当前变为 Failed。
- [ ] Domain: exact eight canonical strings、serde serialization/deserialization round-trip。
- [ ] Compatibility: exact eight historical Debug strings parse to matching variants。
- [ ] DB: typed update stores canonical raw text，list restores variant and timestamp。
- [ ] Corruption: unknown/empty status returns redacted error and no partial list。
- [ ] Repository: format、all-target/all-feature check、strict Clippy、full serial tests。

## 回滚方案

不得恢复 Debug persistence 或 unknown → Failed fallback。若客户端兼容需要过渡，只可在 deserialization/read parser 增加
明确的 temporary alias；serialization 和新 DB writes 必须继续 canonical snake_case，并单独跟踪 alias removal。
