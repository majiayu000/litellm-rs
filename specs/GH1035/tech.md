# Tech Spec

## Linked Issue

GH-1035 / #1035

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Dependency lock | `Cargo.lock` | 锁定 `spin 0.9.8`，该版本已被 crates.io 撤回 | B-001/B-002 根因与唯一写入范围 |
| Reverse dependency | Cargo resolver | `spin 0.9.8 -> flume 0.11.1 -> sqlx-sqlite 0.8.6 -> sqlx/SeaORM` | B-003 兼容边界 |
| Supply-chain audit | `cargo audit` | 报告 `spin 0.9.8` yanked warning | B-004 验收信号 |
| Rust verification | Cargo workspace | 当前业务代码依赖现有同步原语行为 | B-005 回归边界 |

## 根因

项目 manifest 没有直接声明 `spin`；Cargo resolver 曾把 `flume` 的兼容依赖锁定到后来被撤回的
`0.9.8`。当前约束允许同一 0.9 版本线的 `0.9.9`，因此无需升级上游 crate 或修改 manifest。

## 设计方案

1. 在从最新 `origin/main` 创建的 Impl 分支执行
   `cargo update -p spin@0.9.8 --precise 0.9.9`。
2. 拒绝任何超出 `Cargo.lock` 的文件变化；在 lockfile 内只接受 `spin` package version/checksum，以及
   `flume`、`lazy_static` 既有 dependency entry 的版本消歧引用变化。不得改变其他 package 版本或依赖集合。
3. 用正反向依赖查询确认旧版本消失、新版本仍沿既有 `flume` 路径解析。
4. 运行 `cargo audit`，确认本 warning 消失且其他独立 warning 没有被 ignore 或吞掉。
5. 运行完整 Rust 格式、编译、strict Clippy 和全 feature 测试。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | `Cargo.lock` spin package entry | `rg -n 'name = "spin"|version = "0\\.9\\.(8|9)"' Cargo.lock`; old-version reverse tree 预期无匹配 |
| B-002 | 单一 lockfile diff | `git diff --stat`; `git diff -- Cargo.lock`; 确认只有 package metadata 与既有版本消歧引用；对 `spin@0.9.9` 执行 precise dry-run 预期无变化 |
| B-003 | Cargo resolved graph | `cargo tree -i spin@0.9.9 --locked --all-features` |
| B-004 | RustSec/crates.io audit | `cargo audit`; 检查无 `spin 0.9.8` 且无新增 ignore |
| B-005 | Workspace | format、check、strict Clippy、全 feature tests |
| B-006 | GitHub workflow | 独立 Spec/Impl PR metadata、base/head 与 open state |

## 备选方案

- 升级 `flume`、SQLx 或 SeaORM：范围更大且当前无必要，违反 B-002，拒绝。
- 升级到 `spin` 0.10/0.12：跨版本线且超出上游约束，回归面更大，拒绝。
- 在 audit 配置中忽略 yanked warning：隐藏证据而未消除锁定版本，违反 B-004，拒绝。
- 全量 `cargo update`：会同时更新大量无关依赖，无法把回归归因到本 issue，拒绝。

## 风险

- Compatibility: `0.9.9` 位于同一 semver 兼容版本线，且 MSRV Rust 1.38 低于项目工具链；仍以全量测试验证。
- Concurrency: `spin` 提供同步原语；虽然本项目经 `flume` 间接使用，必须保留完整测试证据。
- Scope drift: 普通 update 可能更新其他 crate；使用精确 package/version，并区分合法的 dependency
  disambiguation reference 重写与不允许的其他 package 版本/依赖集合变化。
- Audit interpretation: 其他 warning 仍可能令输出非空；只验证目标 warning 消失，不篡改其余证据。

## 测试计划

- [ ] `cargo update -p spin@0.9.8 --precise 0.9.9 --dry-run` 在实现前只计划单 package 更新。
- [ ] 实现后 `cargo update -p spin@0.9.9 --precise 0.9.9 --dry-run` 成功且不产生变化；旧 package ID
      查询则应明确失败。
- [ ] 旧版本反向树查询失败，新版本反向树显示既有 `flume` 路径。
- [ ] `cargo audit` 不再报告 `spin 0.9.8`，其他 warning 保持可见。
- [ ] `cargo fmt --all -- --check`。
- [ ] `cargo check --all-targets --all-features --locked`。
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`。
- [ ] `cargo test --all-features --locked -- --test-threads=1`。

## 回滚方案

若 `0.9.9` 引发可复现回归，回滚 Impl PR 的单一 lockfile commit，并为上游依赖替代方案建立新的独立 issue；
不得通过 audit ignore 将 `0.9.8` 的 yanked warning 静默保留。
