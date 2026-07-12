# Tech Spec

## Linked Issue

GH-961 / #961

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Canonical migration | `src/storage/database/migration/m20240101_000005_create_api_keys_table.rs:8` | 创建 `api_keys`；`fk_api_keys_user_id` 在 `:59-63` 使用 `SetNull` | 新安装的 schema 真值源，也是 SQLite 重建时应复用的表定义 |
| Migration registry | `src/storage/database/migration/mod.rs:18` | 顺序注册 10 个 SeaORM migration | 存量数据库需要追加一个新 migration，不能只修改已应用的旧文件 |
| Entity metadata | `src/storage/database/entities/api_key.rs:61` | relation 在 `:65-71` 声明 `on_delete = "SetNull"` | ORM relation 必须与数据库约束一致 |
| Migration runtime | `src/storage/database/seaorm_db/connection.rs:128` | 启动通过 `Migrator::up` 应用 pending migrations，失败作为 storage error 返回 | 迁移失败必须 fail closed，不能继续使用部分 schema |
| Legacy bootstrap | `deployment/scripts/init-db.sql:51` | 已文档化 Docker/PostgreSQL 初始化入口，API-key owner FK 使用 `CASCADE` | 该入口也必须拒绝删除仍拥有 key 的 user |
| Runtime owner validation | `src/auth/api_key/creation.rs:359` | 非空 owner missing/non-active 时拒绝；`:502`, `:566`, `:610` 已覆盖 missing、inactive、ownerless | schema 修复必须保留 GH958 的认证边界，不修改 verifier |
| Direct canonical delete evidence | `src/storage/database/seaorm_db/user_repository_tests.rs:216` | 测试可直接删除 canonical `users` 记录 | 数据库约束必须覆盖绕过未来应用入口的直接删除 |

## 设计方案

### 决策

采用数据库级 `RESTRICT`：存在 user-owned key 时拒绝删除 owner。`CASCADE` 会在删除 user 时
静默销毁凭证与潜在审计证据；“先 revoke 再删除”需要当前不存在的 canonical 删除服务、事务
边界和 provenance 设计。`RESTRICT` 是不扩展产品 surface 且能立即阻止非法转换的最小方案。

### Fresh schema 与 ORM

1. 保持已发布的原始 migration 输出为 `SetNull`，只将 16 列表定义提取为无行为变化的同模块
   helper；原有索引创建块保持原位。新增 migration 在 fresh 和 upgrade 序列中都执行，因此
   最终 schema 均收敛到 `RESTRICT`，同时不重写历史 migration 语义。
2. SQLite upgrade migration 复用该 table helper，避免复制 16 列 schema。
3. 将最终 SeaORM entity relation 改为 `on_delete = "Restrict"`。
4. 将 legacy bootstrap 的 API-key owner FK 从 `CASCADE` 改为 `RESTRICT`，但不借此声称其
   BIGINT/VARCHAR schema 已与 SeaORM UUID schema 收敛。

`deployment/scripts/init-db.sql` 使用与当前 SeaORM UUID schema 不兼容的旧 BIGINT/VARCHAR
模型；本 issue 只对齐 owner 删除契约，完整部署 schema 收敛仍需独立设计。

### 存量升级 migration

追加 `m20260712_000001_restrict_api_key_owner_deletion`，并在 registry 最后注册：

- PostgreSQL：在 SeaORM 已提供的 migrator transaction 中删除命名约束
  `fk_api_keys_user_id`，随后以同名、同列重新创建 `ON DELETE RESTRICT` 外键。任一步失败会回滚。
- SQLite：repository `Migrator::up/down` 先开启外层 transaction，使 SeaORM 的 schema 操作与
  随后的 `seaql_migrations` insert/delete 同时提交或回滚。表重建在该 transaction 的 savepoint
  中创建临时表，使用结构化 `INSERT ... SELECT` 逐列复制 16 列，删除旧表，将临时表改名为
  `api_keys`，重建四个索引并执行 `PRAGMA foreign_key_check("api_keys")`。全程保持
  `PRAGMA foreign_keys = ON`；dangling owner 或 ledger 写入失败都回滚，不允许静默修复。
- `down` 使用相同后端路径恢复 `SetNull`，使单步 rollback 与 migration ledger 一致。

SQLite 表重建没有入站 `api_keys` 外键：当前 migration/entity 搜索只发现该表对 `users` 的出站
外键，因此 transaction 内 drop/rename 不会触发其他表的级联语义。

### 测试设计

在 `src/storage/database/seaorm_db/api_key_owner_migration_tests.rs` 增加真实 upgrade 测试：只
应用前 10 个 migrations，插入 canonical user、owned key、global key 与无 key user，保存两条
key 的完整 entity model，再应用最后一个 migration。验证：

1. 两条 key 的所有字段逐字段相等；
2. 删除有 key owner 返回数据库错误，user 与 keys 保持不变；
3. 删除无 key user 成功；
4. global key 仍为 `user_id = NULL`；
5. 重复 key hash 仍因唯一索引失败；
6. CI PostgreSQL service 从前 10 个 migration 带数据升级后执行真实 owner delete contract；
7. 两后端单步 `down` 后恢复 `SetNull`，删除 owner 会保留 key 并只把 `user_id` 置空；
8. 注入 SQLite ledger insert/delete 失败后，schema 与 ledger 一起回滚；
9. PostgreSQL builder 与 legacy bootstrap 都包含 `ON DELETE RESTRICT`。

现有 GH958 测试继续证明 missing/inactive owner fail closed 与 global key 可认证，不修改其通过
禁用外键构造历史 orphan 的专用 fixture。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | 新 migration 外键 + upgrade test 的 direct user delete | `cargo test owner_restrict_upgrade_preserves_keys_and_blocks_owner_delete --all-features --locked` |
| B-002 | 新增 final migration、entity relation | 同一 upgrade test 断言 owned model 的 `user_id` 与整行不变 |
| B-003 | `RESTRICT` 仅约束被引用 user | 同一 upgrade test 删除无 key user 并断言 row absent |
| B-004 | nullable列保持不变，无关 user delete | 同一 upgrade test 逐字段比较 global model；`cargo test live_and_detailed_verification_accept_ownerless_key --all-features --locked` |
| B-005 | PostgreSQL constraint replacement + SQLite transactional rebuild | 两后端 upgrade test 都从前 10 个 migration 带数据升级并比较完整 entity models；PostgreSQL SQL builder test |
| B-006 | PostgreSQL migrator transaction + SQLite outer transaction/savepoint | dangling owner 与 ledger insert/delete failure fixtures 都验证 schema/ledger 回滚 |
| B-007 | database foreign key serializes insert/delete commit | `cargo test owner_restrict_upgrade_preserves_keys_and_blocks_owner_delete --all-features --locked` + 外键约束 review |
| B-008 | verifier 保持不变 | `cargo test live_and_detailed_verification_reject_missing_owner --all-features --locked` 与 `cargo test live_and_detailed_verification_reject_inactive_owner --all-features --locked` |
| B-009 | migration 复制 `NULL` 不推断 provenance | upgrade test 断言 global model 完整相等 |
| B-010 | SeaORM migration ledger + 删除失败无副作用 | 连续两次 `db.migrate()`，第二次无 pending migration；重复 owner delete 均失败且 models 不变 |

## 数据流

正常删除：`DELETE users(id)` -> 数据库检查 `api_keys.user_id` -> 有引用则整个 statement 失败，
无引用则提交。runtime verifier 无新增查询。

升级：SQLite outer transaction -> migration ledger -> 后端分支 -> 替换外键/表 -> 复制原值 ->
重建索引 -> 写入 migration ledger -> commit。任何错误通过 `DbErr` 传播并回滚，启动不得报告成功。

## 备选方案

- `CASCADE`：会隐式删除 key，扩大 user deletion 的副作用并丢失审计对象，拒绝。
- 应用层“先查 key 再删 user”：直接 SQL 可绕过，且 check/delete 有竞态，拒绝。
- trigger 覆盖旧 `SetNull`：有效行为可限制删除，但 fresh/upgrade 的 FK 元数据仍分叉，拒绝。
- SQLite 关闭 foreign keys 后重建：pool connection 与中断恢复风险更高，也会掩盖 dangling data，
  拒绝。
- 显式 revoke transaction：长期可选，但需要新的删除 API、审计 contract 和更大产品决策，超出
  GH961。

## 风险

- Security: 所有路径必须 fail closed；不得把 migration error 降级为可用的旧 schema。
- Compatibility: 以前可成功删除有 key user 的直接 SQL 现在失败；调用者需先显式处理 keys。
- Data integrity: SQLite 重建必须复制全部 16 列并恢复 unique/non-unique indexes。
- Performance: PostgreSQL 持有短时 schema lock；SQLite 对 `api_keys` 做一次全表复制，升级期间
  需要写锁，但请求路径无新增成本。
- Maintenance: 原始表 builder 成为 initial 与 rebuild 的单一列定义；新增列时仍需同步复制列
  清单和新 migration 的紧凑索引清单，完整 model/index 测试会检测漏项。

## 测试计划

- [ ] Unit: PostgreSQL foreign-key statement 包含命名约束与 `ON DELETE RESTRICT`。
- [ ] Integration: CI `DATABASE_URL` 指向的 PostgreSQL service 在独立 test schema 中完成带数据
  升级、限制删除、global key 保留与单步 down 验证；无该环境变量的本地运行明确跳过外部
  服务 fixture。
- [ ] Integration: SQLite fresh 与 pre-GH961 schema 带数据升级，覆盖保数据、限制删除、global
  key、无 key user、索引、重复 migrate 与单步 down。
- [ ] Negative: dangling non-null owner 以及 ledger insert/delete failure 都使 schema 与 ledger 回滚。
- [ ] Regression: PostgreSQL-only feature run 不执行 SQLite fixture；legacy bootstrap FK 为 `RESTRICT`。
- [ ] Regression: GH958 missing/inactive/ownerless verifier tests。
- [ ] Repository: `cargo fmt --all -- --check`、`cargo check --all-targets --all-features --locked`、
  strict clippy、全量 test、scope/overlap guards。

## 回滚方案

单步执行 repository `Migrator::down`：PostgreSQL 原子重建 `SetNull` 外键；SQLite 在包含 ledger
delete 的外层 transaction 内重建为 `SetNull` 并保留数据/索引。回滚会恢复旧风险，只作为代码
回滚兼容机制，不是正常 user 删除流程。
