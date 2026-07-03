# Product Spec

## Linked Issue

GH-843 / #843

## 用户问题

仓库当前追踪了编译产物、一次性生成/修复脚本、scratch 文档、大量没有生产调用点的自制并发原语，以及行为与命名不一致的空壳 feature flags。
这些内容会干扰代码搜索、增大维护面，并让用户以为 `websockets` / `analytics` / `enterprise` 开关会改变实际编译行为。

## 目标

- 版本控制中不再包含编译产物和一次性脚本。
- `src/utils/sync` 只保留有生产调用点或明确公开用途的并发原语；无调用点的薄封装删除或按维护者批复保留。
- `websockets` / `analytics` / `enterprise` feature flags 的行为与文档一致：要么真正 gate 对应模块，要么移除并更新 docs.rs / README / Cargo 注释。
- 清理后 `cargo check --all-features` 与相关测试通过。

## 非目标

- 不清理 `docs/` 下历史审计文档。
- 不重构运行时代码逻辑；本 issue 只处理仓库卫生、公开导出和 feature gate 对齐。
- 不删除仍有生产调用点、外部 API 明确承诺或维护者要求保留的工具。

## Behavior Invariants

1. `git ls-files` 不再列出 `.rlib` 编译产物和 issue 中列出的顶层一次性生成/修复脚本。
2. 删除 `ConcurrentMap` / `ConcurrentVec` / `VersionedMap` 前必须用当前 `origin/main` 的搜索证据证明没有生产调用点；仅测试、doctest、模块自引用不能算生产调用点。
3. 保留 `AtomicValue`，因为 `AppState.config` 仍有生产使用。
4. feature flags 调整后，`cargo check` 在默认 features、`--all-features`、docs.rs feature set 下都可解释；不能留下声明了但无行为的开关。
5. README、模块注释、Cargo feature 注释和 docs.rs metadata 与实际 feature 行为一致。

## 验收标准

- [ ] `git ls-files` 不再包含 `libcache_manager_tests.rlib`、`batch_create_providers.sh`、`create_provider_impls.sh`、`create_providers.sh`、`fix_error_mappers.sh`、`fix_streaming_files.sh`、`STREAMING_FIXES.md`、`plan/2026-02-08_12-09-23-refresh-all-provider-model-catalogs.md`、`src/utils/REORGANIZATION_SUMMARY.md`。
- [ ] `src/utils/sync` 只导出保留的原语；删除项的 tests、docs、public re-exports 同步移除或替换。
- [ ] `websockets` / `analytics` / `enterprise` 不再是空壳 feature：每个 feature 都有真实 gate 或被移除并更新所有引用。
- [ ] `cargo check --all-features` 通过。
- [ ] 默认 feature set 和 docs.rs feature set 的检查命令在 PR body 记录。

## 边界情况

- `deployment/**/*.sh`、`scripts/guards/*.sh`、`scripts/test/*.sh` 是运行/CI 脚本，不属于本 issue 的一次性脚本清理范围。
- `utils::sync::*` 可能是 crate public export；删除前必须在 PR body 明确这是有意的公开 API 清理，或按维护者要求先做 deprecation。
- `enterprise = ["analytics", "vector-db"]` 当前会影响依赖闭包；若移除或改 gate，必须检查 docs.rs metadata 和 feature composition。
- `websockets` 可能应 gate `src/core/realtime`，但不能只 gate 一个 error enum variant 后声称支持 websockets。

## 发布说明

仓库卫生与 feature flag 对齐。若删除 public exports 或 Cargo features，CHANGELOG 需标注为 breaking 或明确 repo 当前不承诺兼容。
