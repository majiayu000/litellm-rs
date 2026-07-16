# Tech Spec

## Linked Issue

GH-1038 / #1038

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Dependency lock | `Cargo.lock` | 锁定已撤回的 `spin 0.10.0` | B-001/B-002 根因与唯一写入范围 |
| Reverse dependency | Cargo resolver | `spin -> crc-fast 1.9.0 -> aws-smithy-checksums 0.64.7 -> aws-sdk-s3 1.131.0` | B-003 兼容边界 |
| Supply-chain audit | `cargo audit` | 报告 `spin 0.10.0` yanked warning | B-004 验收信号 |
| Rust verification | Cargo workspace | S3/checksum 路径依赖既有同步原语行为 | B-005 回归边界 |

## 根因

项目 manifest 没有直接声明 `spin`；`crc-fast` 的 `spin` feature 经 AWS checksum 依赖链启用，Cargo resolver
曾锁定后来被撤回的 `0.10.0`。现有约束允许同一 0.10 版本线的 `0.10.1`，无需升级上游 crate 或 manifest。

## 设计方案

1. 在从最新 `origin/main` 创建且包含 GH1038 Spec 的 Impl 分支执行
   `cargo update -p spin@0.10.0 --precise 0.10.1`。
2. 拒绝任何超出 `Cargo.lock` 的文件变化；lockfile 内只接受 `spin` package version/checksum 和
   `crc-fast` 既有 dependency entry 的版本消歧引用变化，不得改变其他 package。
3. 用正反向依赖查询确认旧版本消失、新版本仍沿既有 AWS S3 checksum 路径解析。
4. 运行 `cargo audit`，确认目标 warning 消失且其他独立 warning 没有被 ignore 或吞掉。
5. 运行完整 Rust 格式、编译、strict Clippy 和全 feature 测试。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | `Cargo.lock` spin package entry | old package ID 查询预期失败；新 package ID 查询成功 |
| B-002 | 单一 lockfile diff | `git diff --stat`; `git diff -- Cargo.lock`; 新 package precise dry-run 无变化 |
| B-003 | Cargo resolved graph | `cargo tree -i spin@0.10.1 --locked --all-features` |
| B-004 | RustSec/crates.io audit | `cargo audit`; 检查无 `spin 0.10.0` 且无新增 ignore |
| B-005 | Workspace | format、all-target/all-feature check、strict Clippy、全 feature tests |
| B-006 | GitHub workflow | 独立 Spec/Impl PR metadata、base/head 与 open state |

## 备选方案

- 升级 `crc-fast` 或 AWS SDK：范围更大且当前无必要，违反 B-002，拒绝。
- 升级到 `spin` 0.11/0.12：跨版本线且超出上游约束，回归面更大，拒绝。
- 在 audit 配置中忽略 yanked warning：隐藏证据而未消除锁定版本，违反 B-004，拒绝。
- 与 `spin 0.9.x` 合并处理：两个 package ID、依赖路径和 PR 都独立，不利于归因，拒绝。
- 全量 `cargo update`：会同时更新大量无关依赖，违反最小范围，拒绝。

## 风险

- Compatibility: `0.10.1` 位于同一 semver 兼容版本线，且 MSRV Rust 1.60 低于项目工具链；仍以全量测试验证。
- Concurrency: `spin` 提供同步原语并被 checksum cache 路径间接使用；需保留完整测试证据。
- Scope drift: 精确 update 仍会合法重写 `crc-fast` 的版本消歧引用；除此以外的 package 变化都应停止实现。
- Audit interpretation: 其他 warning 仍可能存在；只验证目标 warning 消失，不篡改其余证据。

## 测试计划

- [ ] 实现前 `cargo update -p spin@0.10.0 --precise 0.10.1 --dry-run` 只计划单 package 更新。
- [ ] 实现后 `cargo update -p spin@0.10.1 --precise 0.10.1 --dry-run` 成功且无变化。
- [ ] 旧 package ID 查询失败，新版本反向树显示既有 AWS S3 路径。
- [ ] `cargo audit` 不再报告 `spin 0.10.0`，其他 warning 保持可见。
- [ ] `cargo fmt --all -- --check`。
- [ ] `cargo check --all-targets --all-features --locked`。
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings`。
- [ ] `cargo test --all-features --locked -- --test-threads=1`。

## 回滚方案

若 `0.10.1` 引发可复现回归，回滚 Impl PR 的单一 lockfile commit，并为 `crc-fast`/AWS 依赖替代建立新的
独立 issue；不得通过 audit ignore 静默保留 `0.10.0`。
