# Product Spec

## Linked Issue

GH-1032 / #1032

complexity: small

## 用户问题

仓库 lockfile 固定 `anyhow 1.0.102`，RustSec `RUSTSEC-2026-0190` 将该版本标记为 unsound；
`cargo audit --deny unsound` 因此失败。上游已在 `1.0.103` 修复，当前依赖约束允许精确升级而无需修改
manifest 或其他 package。

## 目标

- 将解析后的 `anyhow` 版本精确更新到 `1.0.103`。
- 消除 `RUSTSEC-2026-0190`，恢复 `cargo audit --deny unsound` 安全门。
- 保持依赖更新最小化、可复现，并证明所有 feature/target 继续构建和测试。

## 非目标

- 不升级其他落后依赖。
- 不处理 `proc-macro-error2` unmaintained 或 `spin` yanked warning。
- 不修改 `Cargo.toml`、`.cargo/audit.toml`、生产代码或公开 API。
- 不新增 advisory ignore。

## Behavior Invariants

1. B-001 `Cargo.lock` 中解析到的 `anyhow` 版本必须为 `1.0.103`，不得继续包含 `1.0.102`。
2. B-002 lockfile 更新只能改变 `anyhow` package 的 version 与对应 checksum；其他 package 的版本、source、
   checksum 和依赖关系不得变化。
3. B-003 `cargo audit --deny unsound` 必须成功，且输出不得包含 `RUSTSEC-2026-0190`；禁止通过 ignore、
   降低 deny 级别或修改 audit 配置制造成功。
4. B-004 普通 `cargo audit` 必须继续完整报告剩余 unmaintained/yanked warning；本变更不得把既有 warning
   静默隐藏或误称为零告警。
5. B-005 all-target/all-feature check、strict Clippy 和全 feature tests 必须在更新后的 lockfile 上通过。
6. B-006 使用 `--locked` 的构建和测试必须成功，证明提交的 lockfile 可以在不重新解析依赖的情况下复现。

## 验收标准

- [ ] `Cargo.lock` 只包含 `anyhow 1.0.103`，diff 中没有其他 package 更新。
- [ ] `cargo tree -i anyhow --locked --all-features` 显示 `anyhow v1.0.103`。
- [ ] `cargo audit --deny unsound` 成功且不再报告 `RUSTSEC-2026-0190`。
- [ ] `cargo audit` 保留其他基线 warning 的可见性。
- [ ] 格式、check、strict Clippy 与全 feature tests 全部产生当前 head 的成功证据。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | N/A：无运行时输入；Cargo.lock package 必须存在并由 B-001 固定。 |
| 错误与失败路径 | covered: B-003, B-005, B-006；audit、解析、构建或测试失败均阻断交付。 |
| 授权/权限 | N/A：不改变认证、授权或运行时权限。 |
| 并发/竞态 | N/A：lockfile 更新与离线检查无共享运行时状态。 |
| 重试/幂等 | covered: B-001, B-002；相同 precise update 重跑不应产生额外 diff。 |
| 非法状态转换 | covered: B-002；不得借修复 advisory 转为全量依赖刷新。 |
| 兼容/迁移 | covered: B-005, B-006；所有 feature/target 与锁定构建证明兼容。 |
| 降级/回退 | covered: B-003, B-004；禁止 ignore 或隐藏 warning 作为 fallback。 |
| 证据与审计完整性 | covered: B-002, B-003, B-004；diff 与两种 audit 输出共同证明结果。 |
| 取消/中断 | N/A：更新只有一个 lockfile commit；中断后可从干净基线重跑。 |

## 发布说明

依赖安全维护：将传递依赖 `anyhow` 更新到 `1.0.103`，修复 `RUSTSEC-2026-0190`；无运行时 API
或配置变更。
