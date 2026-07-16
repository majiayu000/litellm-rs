# Tech Spec

## Linked Issue

GH-1032 / #1032

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Locked dependency | `Cargo.lock:419` | `anyhow` 固定为 `1.0.102`，带对应 registry checksum | 唯一需要修改的实现区域 |
| Audit policy | `.cargo/audit.toml:3` | 仅 ignore 三个与本 advisory 无关的既有条目 | 必须保持不变，不能用 ignore 绕过 unsound warning |
| Direct manifests | workspace `Cargo.toml` files | 没有直接声明 `anyhow` | 修复不需要新增或修改 manifest dependency |
| Dependency paths | `rustify -> vaultrs`、`tiktoken-rs` | 两条传递路径共同解析到同一个 `anyhow 1.0.102` | lockfile precise update 可同时修复两条路径 |

## 根因

依赖解析发生在 RustSec 修复版本发布前，`Cargo.lock` 因而持续固定 `anyhow 1.0.102`。现有传递依赖
使用兼容 semver 约束，dry-run 已证明 Cargo 可以只把 `anyhow` 更新到 `1.0.103`，无需提升直接依赖或
重解其他 package。

## 设计方案

### 1. Precise lockfile update

在基于最新 `origin/main` 且包含本 Spec 的实现分支执行：

```sh
cargo update -p anyhow --precise 1.0.103
```

预期 `Cargo.lock` 的 `[[package]] name = "anyhow"` 只改变 `version` 与 `checksum`。`source`、依赖路径、
其他 package 和 manifest 均不变。

### 2. Scope proof

- `git diff --name-only` 只能输出 `Cargo.lock`。
- lockfile diff 只能出现 `anyhow` package 的旧/新 version 与 checksum。
- `cargo update -p anyhow --precise 1.0.103 --dry-run` 在更新后必须报告无需额外变更。
- `cargo tree -i anyhow --locked --all-features` 必须显示 `anyhow v1.0.103` 的现有两条传递路径。

### 3. Security and compatibility proof

- `cargo audit --deny unsound` 必须退出 0，且不得新增 `RUSTSEC-2026-0190` ignore。
- 普通 `cargo audit` 仍记录剩余 unmaintained/yanked warning，避免把“unsound 已修复”误报为“无 warning”。
- 使用更新后的 lockfile 运行 all-target/all-feature check、strict Clippy 和全 feature tests；所有 Cargo
  验证使用 `--locked`。

## 影响文件

| Path | Change |
| --- | --- |
| `Cargo.lock` | `anyhow 1.0.102 -> 1.0.103` 及匹配 checksum |

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | `Cargo.lock` anyhow package | `cargo tree -i anyhow --locked --all-features` 显示 `anyhow v1.0.103`；`rg 'version = "1.0.102"' Cargo.lock` 零命中 |
| B-002 | precise lockfile diff | `git diff -- Cargo.lock` 只含 anyhow version/checksum；`git diff --name-only` 只含 `Cargo.lock` |
| B-003 | RustSec audit gate | `cargo audit --deny unsound` 退出 0；`rg 'RUSTSEC-2026-0190' .cargo/audit.toml` 零命中 |
| B-004 | ordinary audit reporting | `cargo audit` 退出 0并继续显示基线 unmaintained/yanked warning |
| B-005 | workspace build/test surface | format、all-target/all-feature check、strict Clippy、全 feature tests |
| B-006 | committed resolution | 所有 check/clippy/test 命令带 `--locked` 并成功 |

## 风险与缓解

- 风险：Cargo 顺带更新其他 package。缓解：使用 `-p` + `--precise`，并以 package-level diff 阻断额外变化。
- 风险：通过 audit ignore 伪造修复。缓解：`.cargo/audit.toml` 不在影响文件中，并显式检查 advisory 零命中。
- 风险：补丁版本改变传递依赖行为。缓解：完整 feature/target 编译、Clippy 与测试矩阵。

## 验证计划

```sh
cargo fmt --all -- --check
cargo tree -i anyhow --locked --all-features
cargo update -p anyhow --precise 1.0.103 --dry-run
cargo audit --deny unsound
cargo audit
cargo check --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
git diff --check origin/main...HEAD
```

## 回滚

若 `1.0.103` 导致兼容问题，回滚实现 commit 并重新打开 #1032；不得通过新增 advisory ignore 或降低
audit deny 级别维持表面绿灯。
