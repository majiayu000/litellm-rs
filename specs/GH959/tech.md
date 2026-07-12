# Tech Spec

## Linked Issue

GH-959 / #959

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Required boundary |
| --- | --- | --- | --- |
| API-key lookup | `src/auth/api_key/creation.rs` | `find_api_key_cached` trusts a complete Redis snapshot before the database and writes DB hits with a five-minute TTL | replace the authentication lookup with an authoritative database read |
| Live verification | `src/auth/api_key/creation.rs` | active/expiry/owner checks run against the cached snapshot | preserve checks but feed them only the database record |
| Detailed verification | `src/auth/api_key/creation.rs` | same stale-cache exposure through the detailed path | use the same authoritative lookup boundary |
| Mutation cleanup | `src/auth/api_key/management.rs` | revoke deletes only before the DB mutation, leaving a legacy refill window | invalidate before and after commit while keeping upgraded-instance correctness independent of cache |
| Regression tests | `src/auth/api_key/tests.rs` | no test leaves a readable active snapshot after a denied cache delete | add a live Redis ACL test that denies `DEL` while allowing `GET`/`SET` |

## 设计方案

1. 删除认证专用的 `API_KEY_CACHE_TTL` 和 `find_api_key_cached` cache-aside 实现，增加私有 `find_api_key_authoritative`，仅调用 `database.find_api_key_by_hash`。
2. `verify_key` 与 `verify_key_detailed` 保持各自现有返回契约，但都通过 `find_api_key_authoritative` 获取 key；active、expiry、owner 和 last-used 的顺序不变。
3. 保留 `api_key_cache_key` 与 `invalidate_api_key_cache`。`revoke_key` 在数据库 mutation 前后各执行一次 best-effort invalidation，以缩小旧 cache-first 副本的回填窗口；删除失败仍可诊断，但不再位于已升级实例的授权可信边界。
4. 不在数据库错误时读取 Redis。`find_api_key_by_hash(...).await?` 原样传播错误，避免 silent degradation。
5. 在现有 `src/auth/api_key/tests.rs` 中增加 Redis ACL 回归：
   - 通过 `REDIS_URL` 建立管理连接，并创建唯一的受限测试用户；
   - 仅允许该用户对唯一 cache key 执行 `GET`/`SET`，明确拒绝 `DEL`；
   - 写入 active `ApiKey` 快照后调用真实 `revoke_key`；
   - 确认数据库记录 inactive、受限连接仍能读到 active stale snapshot；
   - 确认 live/detailed verification 都从数据库拒绝该 key；
   - 用管理连接清理 key 与 ACL 用户。
6. 本地 Redis 不可达时沿用仓库既有 live-Redis 测试约定跳过；CI 设置 `CI` 与 `REDIS_URL`，Redis 不可达则测试必须失败。协调器在本地隔离 Redis 实例上执行一次非跳过验证。
7. 部署要求：在依赖即时撤销保证前，排空所有仍运行 cache-first 认证代码的旧副本。双 invalidation 不能阻止已经在 DB commit 前读到 active row 的旧请求于第二次删除后回填。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1/P4 | `find_api_key_authoritative` and both callers | code review plus existing DB error propagation |
| P2/P3 | revoke path plus authoritative verification | ACL-denied `DEL` regression with readable stale active snapshot |
| P5 | unchanged live/detailed checks | existing active/inactive/expiry/owner tests and full suite |
| P6 | retained invalidation helpers, removed auth cache read/write | diff inspection and stale snapshot assertion |

## 数据流

`raw_key` -> hash -> authoritative database lookup -> active/expiry checks -> optional owner database lookup -> owner predicate -> reject or update last-used -> verification result。

Redis invalidation remains a side-effect of mutation paths only. There is no Redis-to-authentication edge after this change.

## 受影响文件与规模

- `src/auth/api_key/creation.rs`
- `src/auth/api_key/management.rs`
- `src/auth/api_key/tests.rs`
- `specs/GH959/product.md`
- `specs/GH959/tech.md`
- `specs/GH959/tasks.md`

预计 3 个 code/test 文件、少于 350 行 code diff，满足仓库 scope 限制；现有文件均保持在 800 行以内。

## 备选方案

- 缩短 TTL：仍存在授权窗口，不满足即时撤销，拒绝。
- cache 删除失败时让撤销 API 失败：数据库可能已经提交，且多实例/网络分区仍不能证明所有快照已删除，拒绝。
- 在 Redis 中只缓存 active flag 并做版本校验：仍引入双权威与分布式一致性协议，超过本 issue 的最小安全修复，拒绝。
- 每次认证先读 Redis 再向数据库确认 lifecycle：没有收益且仍增加复杂度，拒绝。

## 风险

- Security: 任何后续优化都不得把完整 Redis key 快照重新引入授权路径。
- Performance: 每次 API-key 验证增加或恢复一次数据库读取；这是即时撤销一致性的明确代价。
- Compatibility: pre/post invalidation 缩小滚动部署窗口，但旧 cache-first 实例必须排空；不能把双删除描述成分布式一致性保证。
- Test isolation: Redis ACL user、key 与密码都使用唯一随机值；场景逻辑通过 finally-style 路径在断言前调用管理连接清理，不依赖受限用户的 `DEL` 权限。
- Error handling: Redis 测试不可用在 CI 中必须失败，数据库错误在生产中必须传播。

## 测试计划

- Red phase: 在隔离 Redis 上运行 ACL-denied deletion 测试，确认旧实现读取 stale active snapshot 而失败。
- Focused: 修复后重复同一测试并运行 API-key 模块测试。
- Deterministic: `cargo fmt --all -- --check`、`git diff --check`、all-features check、strict clippy。
- Repository: `cargo test --all-features --locked -- --test-threads=1`。
- Guards: `bash scripts/guards/check_pr_scope.sh origin/main`、`bash scripts/guards/check_pr_overlap.sh`。
- SpecRail: GH959 packet、implement route、current-head security reviewer、PR gate 与 runtime gate。

## 回滚方案

代码回滚会重新引入 stale-cache 撤销窗口，只能作为安全风险已明确接受的紧急回滚；没有 schema、数据 migration 或 cache 格式回滚步骤。
