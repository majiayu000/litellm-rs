# Tech Spec

## Linked Issue

GH-1059 / #1059

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| SeaORM delete | `src/storage/database/seaorm_db/team_repository/repository_impl.rs` | deletes members/teams/um_teams, discards all ExecResults, always commits | root cause |
| In-memory delete | `src/core/teams/repository.rs` | missing map entry returns NotFound before member cleanup | parity reference |
| Compatibility tests | `src/storage/database/seaorm_db/team_repository_tests.rs` | covers bridged and legacy-only deletes, not missing/rollback | regression surface |

## 设计方案

1. 保持现有 pre-read：收集 canonical/legacy member user IDs，供成功 commit 后清理 legacy user membership。
2. 在现有 transaction 中继续先删除 `team_members`，保存 canonical `teams` 与 legacy `um_teams` delete 的两个
   `ExecResult`。
3. 若两个 `rows_affected()` 都为 `0`，显式 `rollback().await`；rollback 成功后返回
   `GatewayError::NotFound(format!("Team {} not found", id))`。不得执行 transaction 后 user cleanup。
4. 若任一 count 大于 `0`，按现有顺序 commit，再对收集的 user IDs 执行 `remove_legacy_user_team`。
5. SQL execute、commit 与 rollback failure 继续用 `GatewayError::from`，真实 infrastructure error 优先于 NotFound。
6. 增加 missing regression；为证明 rollback，参数化插入 orphan `team_members` row 和引用 missing ID 的 legacy user，
   delete 后断言 member 与 user membership 均保留。现有 legacy-only/bridged tests 覆盖成功分支。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | dual affected-row check + rollback | missing delete returns NotFound |
| B-002 | transaction rollback | orphan canonical member remains readable |
| B-003 | early return before post-commit loop | legacy user still references missing team ID |
| B-004 | either-row existence predicate | existing canonical/bridged/legacy-only delete tests |
| B-005 | unchanged post-commit cleanup | existing canonical membership deletion test |
| B-006 | execute/rollback/commit mappings | source inspection, check/clippy/full tests |
| B-007 | write-boundary result contract | missing regression and in-memory comparison |

## 数据流

Pre-read member IDs → transaction delete members → delete canonical team + delete legacy team → inspect both counts；both zero →
rollback → NotFound；either nonzero → commit → legacy user-membership cleanup → success。

## 备选方案

- 只检查 canonical count：会破坏受支持的 legacy-only deletion，拒绝。
- delete 前 SELECT：增加 round trip、仍有 TOCTOU，且需同时查两表，拒绝。
- both zero 时 commit member cleanup 再 NotFound：失败请求产生副作用，违反 B-002，拒绝。
- missing delete 继续幂等成功：与现有 repository contract/API 预期不一致，拒绝。
- 本次增加 foreign keys/cascade：需要 migration 与跨 backend rollout，超出 issue。

## 风险

- Transaction control: missing branch 必须显式 rollback，不能依赖 drop timing。
- Compatibility: legacy-only delete 必须以 `um_teams` affected row 判定为存在。
- Side effects: post-commit user cleanup 只能在存在 team 且 commit 成功后运行。
- Orphan data: regression 仅证明 missing request 不清理 orphan；不定义 orphan repair policy。

## 测试计划

- [ ] Red: current SeaORM repository delete unknown UUID 返回 `Ok(())`。
- [ ] Missing: fixed path returns `GatewayError::NotFound`。
- [ ] Rollback: orphan member row remains after missing delete。
- [ ] No external side effect: legacy user membership remains after missing delete。
- [ ] Success compatibility: bridged/canonical/legacy-only delete tests pass。
- [ ] Repository: complete team repository module、format、all-target/all-feature check、strict Clippy、full serial tests。

## 回滚方案

不得恢复 unconditional commit/success。若 backend affected-row behavior 异常，应增加 backend-specific regression；若 rollback 失败，继续返回真实
storage error，不得吞掉错误后返回 NotFound。
