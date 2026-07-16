# Tech Spec

## Linked Issue

GH-1053 / #1053

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Legacy enumeration | `src/storage/database/seaorm_db/team_repository/legacy_sync.rs` | loops rows, warns and skips `from_json` errors | B-001 root cause |
| Canonical queries | `src/storage/database/seaorm_db/team_repository/repository_impl.rs` | list/count/name/user-team paths call enumeration/sync and trust success | B-002 propagation surface |
| Direct legacy API | `src/storage/database/seaorm_db/user_management_ops.rs` | `list_teams` already collects fallible deserialization | reference fail-closed contract |
| Repository tests | `src/storage/database/seaorm_db/team_repository_tests.rs` | covers valid bridge behavior but no corrupt row | regression gap |

## 设计方案

1. 将 `list_legacy_um_teams` 的 row conversion 改为 fallible collection：每行先读取 `data`，再严格调用 `from_json`；
   任一转换失败立即返回 `Err`，不保留已转换 prefix。
2. 将反序列化错误映射为稳定的 `GatewayError::Storage`（或同等 typed variant），message 指明
   `um_teams.data` 无效，但不拼接 raw JSON。SQL/column extraction errors 继续使用既有 database mapping。
3. 移除该枚举路径的 `warn + skip`。调用方已使用 `?`，保持现有签名即可让 list/count/get_by_name/sync/get_user_teams
   自动传播错误，不在上层增加 fallback。
4. 在现有 team repository test module 增加参数化 SQLite regression：创建 valid legacy row 与 corrupt row，通过
   parameterized statement 修改 `data`，断言 canonical operations 返回错误、message 有 field context 且无 raw payload。
5. 不修改 `persist_legacy_team` 的 conversion/name-conflict 策略，也不修改 legacy member skip 行为；这些需要独立 issue。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | fallible `list_legacy_um_teams` iterator | single corrupt row makes enumeration-backed list fail |
| B-002 | existing `?` propagation in repository methods | focused list/count/get_by_name/get_user_teams assertions |
| B-003 | all-or-error collection | mixed valid+corrupt rows return only `Err`, never partial values |
| B-004 | redacted typed error mapping | assert field context present and raw marker absent |
| B-005 | unchanged valid conversion/sync paths | existing repository tests plus valid peer coverage |
| B-006 | minimal diff and source inspection | direct-get/member/name-conflict tests remain unchanged and pass |

## 数据流

SQL rows → per-row `data` extraction → strict `LegacyTeam` deserialization → all-or-error `Vec<LegacyTeam>` → existing canonical
sync/query logic。任何 corrupt row 在进入同步前终止整个 operation，调用者收到 storage error，不会观察到 partial canonical state/result。

## 备选方案

- 保留 skip 并返回 skipped count：调用仍可把不完整列表当真，违反 B-001/B-003，拒绝。
- 用默认 `LegacyTeam` 替换坏 row：会伪造 identity/membership/budget 数据，拒绝。
- 只让 public `list` 失败：count/name/user-team 仍 silent degrade，契约不一致，拒绝。
- 自动删除或修复 corrupt row：不可逆且需要产品级恢复策略，超出本 issue。
- 同时收紧 invalid member 行为：扩大兼容风险，留给独立审计项。

## 风险

- Availability: 过去返回 partial success 的查询会在存在坏 row 时失败；这是避免静默数据丢失的预期变化。
- Error exposure: 错误必须有可诊断 field context，但不能包含 raw persisted content。
- Test isolation: corruption 必须通过 parameterized SQL 写入 in-memory SQLite，不能改 migration 或测试基础设施。
- Scope drift: `persist_legacy_team` 仍可能对其他 conversion/name conflict 返回 `None`；本 issue 只修 row JSON decode。

## 测试计划

- [ ] Red: current `TeamRepository::list` 对唯一 corrupt legacy row 返回 `Ok(([], 0))`，证明 silent partial success。
- [ ] Corruption: single corrupt row causes list error。
- [ ] Atomicity: valid peer + corrupt row causes error, no partial list/total。
- [ ] Propagation: count、get_by_name、get_user_teams 同样返回 error。
- [ ] Redaction: error 包含 `um_teams.data`，不包含 raw marker。
- [ ] Regression: existing team repository tests、format、all-target/all-feature check、strict Clippy、full serial tests。

## 回滚方案

不得恢复 warning + skip 或 default row。若部署发现历史损坏，应在独立受审计的 repair/migration 流程中修复数据；查询路径继续
fail closed。若需更细诊断，可增加不含 payload 的 row identifier，但必须另行评估信息暴露与兼容性。
