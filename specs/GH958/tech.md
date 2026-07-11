# Tech Spec

## Linked Issue

GH-958 / #958

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Required boundary |
| --- | --- | --- | --- |
| Live verification | `src/auth/api_key/creation.rs` | 查询 optional owner 后直接成功，missing/non-active owner 均未拒绝 | 在 last-used 更新前调用共享 owner predicate |
| Detailed verification | `src/auth/api_key/creation.rs` | 只拒绝返回的 inactive owner；missing owner 被当成 ownerless | 使用同一 predicate 并保留详细 invalid reason |
| Authentication caller | `src/auth/system.rs` | `verify_key -> None` 映射为通用 `Invalid API key` | 保持不变，避免 owner existence disclosure |
| Owner model | `src/core/models/user/types.rs` | `is_active()` 仅对 `UserStatus::Active` 返回 true | 复用该方法，不复制状态枚举 |
| Schema | `src/storage/database/migration/m20240101_000005_create_api_keys_table.rs` | owner FK 使用 `ON DELETE SET NULL` | 不改；provenance/delete semantics 留给 #961 |

## 设计方案

1. 在 `creation.rs` 增加私有纯函数 `api_key_owner_invalid_reason(api_key, user)`：
   - `api_key.user_id == None` 返回 `None`，表示 ownerless key 不需要 owner；
   - `user_id != None` 且 `user == None` 返回 missing-owner reason；
   - user 存在但 `!user.is_active()` 返回 inactive-owner reason；
   - active user 返回 `None`。
2. `verify_key` 与 `verify_key_detailed` 保留现有 key lookup、active、expiry 与 owner repository query，但在 `update_last_used` 前都调用该函数。
3. live 入口只把 invalid owner 映射成 `Ok(None)`，不把具体 reason 写入公开认证结果。detailed 入口把同一 reason 写入 `invalid_reason`，用于内部诊断与测试。
4. repository 的 `Err` 继续由 `?` 传播；helper 只处理成功查询后的 `Option<User>`，不吞掉基础设施错误。
5. 保留 `find_api_key_cached` 和 cache invalidation 行为，避免混入 #959。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1/P3 | shared owner predicate | active 与 inactive user matrix |
| P2/P5 | both verifier call sites | missing owner 通过 live/detailed 双入口均拒绝 |
| P4 | ownerless branch | ownerless key 双入口成功且 user 为 `None` |
| P6 | existing `find_user_by_id(...).await?` boundary | code review plus existing storage-error propagation contract |
| P7 | existing `AuthSystem::authenticate_api_key` mapping | no caller change; live verifier returns `None` for both owner failures |

## 数据流

`raw_key` -> hash -> existing key lookup -> key active/expiry checks -> optional owner DB lookup -> shared owner predicate -> reject or update last-used -> verification result。

不新增持久化或网络调用。user-owned key 仍执行一次现有 owner DB lookup；ownerless key 不查询 owner。

## 受影响文件与规模

- `src/auth/api_key/creation.rs`
- `specs/GH958/product.md`
- `specs/GH958/tech.md`
- `specs/GH958/tasks.md`

预计 1 个 code/test 文件、少于 250 行 code diff，满足仓库 scope 限制。

## 备选方案

- 让 `verify_key` 直接调用 `verify_key_detailed`：会连带合并 key lookup/debug/result 构造，超过 owner 判定的最小范围，拒绝。
- 把 missing owner 当作 ownerless：违反 `user_id = Some(id)` 的数据契约，拒绝。
- 在本 issue 修改 FK 或删除流程：需要 #961 的维护者决策，拒绝。
- 同时移除 Redis 认证缓存：属于 #959，拒绝。

## 风险

- Security: 必须 fail closed，且 live 客户端不能区分 missing 与 inactive owner。
- Compatibility: 仅此前错误成功的 orphaned/non-active-owner keys 会被拒绝。
- Data integrity: `ON DELETE SET NULL` 后已成为 ownerless 的记录不在本 issue 可识别范围内。
- Performance: 不增加查询；只增加一个纯函数分支。
- Maintenance: 两条入口仍各自构造结果，但 owner 规则只有一个真值源。

## 测试计划

- Unit/in-memory storage: missing owner、inactive owner、ownerless key 的 live/detailed matrix。
- Deterministic: `cargo fmt --all -- --check`、`git diff --check`、all-features check、strict clippy。
- Repository: `cargo test --all-features --locked -- --test-threads=1`。
- Guards: `bash scripts/guards/check_pr_scope.sh origin/main`、`bash scripts/guards/check_pr_overlap.sh`。
- SpecRail: GH958 packet、implement route、current-head reviewer、PR gate 与 runtime gate。

## 回滚方案

回滚 GH958 PR 即恢复旧 owner 判定。没有 schema、数据 migration 或 cache 格式变更。
