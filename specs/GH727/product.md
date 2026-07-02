# Product Spec

## Linked Issue

GH-727 / #727

## 用户问题

`origin/main@5ef936c47c57` 仍有 33 个 Rust 文件超过 U-16 的 800 行硬上限。前几个
tranche 已经证明“小 PR、单文件族、保持行为不变”的方式可行，但现在的最大文件已经从测试或
类型模块转到运行时 repository 代码，需要按真实职责拆分，不能只为了降行数移动代码。

本轮目标继续执行完整的大文件解耦计划：每个 PR 仍然小而可审，但所有 tranche 都必须服从同一套
架构边界，避免制造新的耦合、重导出混乱或行为漂移。

## 全量目标

- 把当前 33 个 over-800 Rust 文件逐步拆到 U-16 范围内。
- 每个 tranche 只拥有一个文件或一个紧密文件家族。
- 拆分必须沿现有架构边界进行：测试按行为域拆、类型按领域 DTO/状态/配置拆、运行时代码按
  provider/route/repository/validator/adapter 职责拆。
- 对 public API 类型文件使用 facade + `pub use` 保持现有导入路径兼容。
- 对运行时代码保留现有错误语义；不得用 warning、fallback 或 silently ignore 代替错误。
- #727 只在最后一次全量扫描确认没有 over-800 Rust 文件后才允许使用 closing keyword。

## 解耦分层

| Lane | 文件类型 | 代表文件 | 拆分策略 |
| --- | --- | --- | --- |
| A | Test-only suites | router tests, utils/event tests, provider test files, integration route tests | 保持原测试断言和模块发现路径，按行为域拆成 child test modules。 |
| B | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | 建立 `types/` 子模块，root 继续 `pub use` 原有类型，禁止字段/别名重命名。 |
| C | Runtime orchestrators | `vertex_ai/client.rs`, `unified_provider.rs`, repositories, validators, integrations | 抽出 request mapping、response mapping、operation handlers、storage helpers 或 error mapper，保留外层入口和 trait surface。 |
| D | Shared utilities | config/net/sync helpers | 按功能域拆 util module，避免新增全局 prelude 或 Any-like public API。 |

## 本 tranche 目标

- 拆分 `src/storage/database/seaorm_db/team_repository.rs`，它当前 932 行，是 #727 当前最大的运行时 repository 文件。
- 保留 `SeaOrmTeamRepository` 的 public 构造入口和 `TeamRepository` trait 行为不变。
- 将 legacy/core 数据转换移动到 `team_repository/conversions.rs`。
- 将 canonical `teams` / `team_members` JSON SQL helpers 移动到 `team_repository/canonical.rs`。
- 将 legacy `um_teams` 同步和 legacy user team membership 更新移动到 `team_repository/legacy_sync.rs`。
- 将 `TeamRepository for SeaOrmTeamRepository` 的 trait implementation 移动到 `team_repository/repository_impl.rs`。
- 保持所有 SQL 参数化、JSON serde、legacy sync、delete transaction、membership cleanup 和 error propagation 语义不变。
- 所有新增或修改后的 Rust 文件低于 800 行。

## 非目标

- 不修改 SeaORM migrations、表结构或 stored JSON schema。
- 不修改 `TeamRepository` trait、`SeaOrmTeamRepository::new` 签名或外部导入路径。
- 不改变 canonical/legacy 双写和 backfill 行为。
- 不新增 warning-only fallback，也不吞掉已有错误。
- 不在本 PR 中处理其余 32 个大文件。
- 不关闭 #727。

## Behavior Invariants

1. `super::team_repository::SeaOrmTeamRepository` remains the repository type used by existing tests and callers.
2. `TeamRepository` operations keep their existing behavior for create/get/get_by_name/update/delete/list/count/member operations.
3. Legacy `um_teams` rows remain visible through the canonical repository and continue to backfill canonical team/member rows.
4. Canonical updates still sync back to the legacy user-management tables and user team membership lists.
5. SQL construction continues to use SeaORM `Statement::from_sql_and_values` and backend-specific placeholders.
6. Every touched repository file must be below U-16's 800-line ceiling.
7. `cargo test storage::database::seaorm_db::team_repository_tests --lib --all-features` must pass.

## 验收标准

- [ ] `team_repository.rs` becomes a small root module that declares focused child modules and keeps `SeaOrmTeamRepository::new`。
- [ ] Conversion helpers move to `team_repository/conversions.rs` without changing serialized fields or role mappings。
- [ ] Canonical SQL helpers move to `team_repository/canonical.rs` without changing statements, placeholders, or error propagation。
- [ ] Legacy synchronization helpers move to `team_repository/legacy_sync.rs` without changing backfill or membership cleanup behavior。
- [ ] The trait implementation moves to `team_repository/repository_impl.rs` without changing method signatures or method bodies。
- [ ] All touched repository files are below U-16's 800-line ceiling。
- [ ] Focused SeaORM team repository tests 通过。
- [ ] `cargo fmt --all -- --check` 和 `cargo check --all-features --locked` 通过。
- [ ] PR body 明确该 PR 是 #727 的 partial tranche，使用 `Refs #727`，不自动关闭 tracker issue。

## 发布说明

No runtime behavior change. This is a SeaORM team repository responsibility split for U-16 compliance.
