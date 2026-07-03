# Tech Spec

## Linked Issue

GH-843 / #843

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| 追踪产物/一次性文件 | `libcache_manager_tests.rlib`、顶层 `batch_create_providers.sh`、`create_provider_impls.sh`、`create_providers.sh`、`fix_error_mappers.sh`、`fix_streaming_files.sh`、`STREAMING_FIXES.md`、`plan/2026-02-08_12-09-23-refresh-all-provider-model-catalogs.md`、`src/utils/REORGANIZATION_SUMMARY.md` | `git ls-files` 仍追踪 | 清理对象 |
| 合法脚本 | `deployment/**/*.sh`、`scripts/guards/*.sh`、`scripts/test/*.sh` | 仍为运行/CI/guard 脚本 | 不应误删 |
| sync exports | `src/utils/sync/mod.rs`、`src/utils/mod.rs` | 公开 re-export `AtomicValue`、`ConcurrentMap`、`ConcurrentVec`、`VersionedMap` | 删除需要同步 export/docs |
| production use | `src/server/state.rs:13,30,69` | 仅 `AtomicValue<Config>` 有生产使用 | 保留 `AtomicValue` |
| sync implementation | `src/utils/sync/{concurrent_map.rs,concurrent_vec.rs,versioned_map.rs}` 与对应 tests | 大量薄封装和测试/示例自引用 | 候选删除对象 |
| feature flags | `Cargo.toml:198-203`、`src/server/routes/health.rs:534-538`、`src/utils/error/canonical.rs:261`、`src/core/mod.rs:7,35` | `websockets` / `analytics` / `enterprise` 声明与实际模块 gate 不一致 | 需要 gate 或移除 |

## 设计方案

1. **产物与一次性文件清理**
   - 删除 `libcache_manager_tests.rlib`，并在 `.gitignore` 加入适当规则（例如 `*.rlib` 或更窄路径）防止再次提交编译产物。
   - 删除 issue 中列出的顶层一次性脚本与 scratch 文档；不要删除 `deployment/` 和 `scripts/guards/` 下仍被文档/CI 使用的脚本。
   - PR body 附 `git ls-files` 检查输出，证明目标文件不再追踪。

2. **`utils/sync` 调用点审计**
   - 先运行生产调用点搜索：
     `rg -n "ConcurrentMap|ConcurrentVec|VersionedMap|AtomicValue" src --glob '*.rs' --glob '!src/utils/sync/**'`。
   - 若只有 `AtomicValue` 有生产调用点，则删除 `concurrent_map.rs`、`concurrent_vec.rs`、`versioned_map.rs` 及对应 tests，更新 `src/utils/sync/mod.rs` 与 `src/utils/mod.rs` re-export。
   - 若发现生产调用点，先替换为标准库 / `DashMap` / `Arc<RwLock<_>>` / 现有类型，再删除；不能留下 broken public export。
   - 由于这些类型当前是 public export，PR body 需要明确破坏性影响，或按维护者批复改为 deprecate-first。

3. **feature flag 对齐**
   - 对每个 feature 做二选一决策：
     - `websockets`: gate 实际 realtime/websocket 模块、routes、依赖和 canonical error variant；或删除 feature 并移除 docs.rs / `full` 引用。
     - `analytics`: gate `src/core/analytics` 及 health feature reporting；或删除 feature 并更新 `enterprise` composition。
     - `enterprise`: 如果只是 meta-feature，文档必须说清楚其包含项；否则必须 gate enterprise-only 模块/行为。
   - `health` 路由的 feature 列表必须来自真实 compile-time behavior，不可只展示空壳 feature。
   - docs.rs metadata、`full` feature、README/Cargo 注释同步更新。

4. **验证矩阵**
   - 默认 features: `cargo check`
   - all features: `cargo check --all-features`
   - docs.rs feature set: `env DOCS_RS=1 cargo doc --no-deps --features "postgres sqlite redis s3 metrics tracing websockets analytics providers-extra providers-extended"`，如果 features 改名/删除，命令同步调整。
   - 静态检查：目标文件不在 `git ls-files`；空 feature 搜索无残留误导。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 产物/一次性文件不追踪 | git index + `.gitignore` | `git ls-files <target paths>` 无输出 |
| P2 删除前有调用点证据 | `src/utils/sync` | `rg` production-use 搜索输出进 PR body |
| P3 `AtomicValue` 保留 | `state.rs` + sync exports | `cargo check --all-features` |
| P4 feature 行为一致 | `Cargo.toml` + gated modules + health | 默认/all/docs.rs 检查 + health tests |
| P5 docs 一致 | README/Cargo/module docs | `rg -n "websockets|analytics|enterprise|ConcurrentMap|ConcurrentVec|VersionedMap"` 人工核对 |

## 风险

- Compatibility: 删除 public exports 或 features 可能影响外部用户；本 repo AGENTS 允许优先清洁架构，但 PR body 必须显式说明。
- Build matrix: feature composition 改动可能影响 docs.rs 和 `full`。必须跑默认、all、docs.rs 三类检查。
- Scope creep: 不要在同一 PR 中顺手重构 analytics/realtime 行为；feature gate 只做启停边界对齐。

## 测试计划

- [ ] `git ls-files libcache_manager_tests.rlib batch_create_providers.sh create_provider_impls.sh create_providers.sh fix_error_mappers.sh fix_streaming_files.sh STREAMING_FIXES.md plan/2026-02-08_12-09-23-refresh-all-provider-model-catalogs.md src/utils/REORGANIZATION_SUMMARY.md`
- [ ] `rg -n "ConcurrentMap|ConcurrentVec|VersionedMap|AtomicValue" src --glob '*.rs' --glob '!src/utils/sync/**'`
- [ ] `cargo check`
- [ ] `cargo check --all-features`
- [ ] docs.rs compatibility command adjusted to final feature set.
- [ ] `cargo test --all-features` before merge if implementation touches Rust source.

## 回滚方案

按切片 revert：产物/脚本删除、`utils/sync` 删除、feature flag 对齐应分提交实现。若 feature flag 改动影响下游，可先 revert feature 切片而保留产物清理。
