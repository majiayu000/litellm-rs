# Product Spec

## Linked Issue

GH-1038 / #1038

complexity: small

## 用户问题

`Cargo.lock` 锁定了 crates.io 已撤回的 `spin 0.10.0`。该版本通过
`crc-fast -> aws-smithy-checksums -> aws-sdk-s3` 进入完整 feature 依赖图，导致供应链审计持续报告
yanked warning，并降低锁文件的长期构建可靠性。

## 目标

- 将完整 feature 依赖图中的 `spin 0.10.0` 精确更新到同一兼容版本线的 `0.10.1`。
- 消除 `cargo audit` 对 `spin 0.10.0` 的 yanked warning。
- 保持 AWS S3、checksum、应用行为、公开 API 和项目源码不变。
- 以一个 package 的最小 lockfile diff 完成更新。

## 非目标

- 不升级 `crc-fast`、AWS SDK 或其他落后依赖。
- 不重复处理独立的 `spin 0.9.8` package。
- 不修改 `Cargo.toml`、生产代码、测试、CI 或公开 API。
- 不用 audit ignore 隐藏 yanked warning。

## Behavior Invariants

1. B-001 `Cargo.lock` 不再解析到 `spin 0.10.0`，而解析到 `spin 0.10.1`。
2. B-002 lockfile diff 只包含 `spin` package version/checksum，以及 `crc-fast` 既有依赖项对该 package 的
   版本消歧引用；不能更新其他 package 版本或依赖集合。
3. B-003 active feature graph 中反向依赖仍为
   `spin -> crc-fast -> aws-smithy-checksums -> aws-sdk-s3 -> litellm-rs`，AWS S3 feature 与行为不变。
4. B-004 `cargo audit` 不再报告 `spin 0.10.0` yanked warning；其他独立 warning 必须继续可见。
5. B-005 格式、all-target/all-feature 编译、strict Clippy 与全 feature 测试保持通过。
6. B-006 Spec PR 与 Impl PR 必须分离，Impl PR 关联 #1038 且不自动合并。

## 验收标准

- [ ] `Cargo.lock` 只更新 `spin` package version/checksum 和 `crc-fast` 的既有版本消歧引用。
- [ ] `cargo tree -i spin@0.10.1 --locked --all-features` 显示既有 AWS S3 路径。
- [ ] `cargo tree -i spin@0.10.0 --locked --all-features` 不再找到 package。
- [ ] `cargo audit` 不再报告 `spin 0.10.0`，且没有新增 ignore。
- [ ] Rust 格式、编译、Clippy 和全量测试通过。
- [ ] 独立 Spec PR 与 Impl PR 已创建，未自动合并。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | N/A；纯 lockfile 依赖解析变更。 |
| 错误与失败路径 | covered: B-004, B-005；审计或验证失败必须阻断完成声明。 |
| 授权/权限 | N/A；不改变认证或权限代码。 |
| 并发/竞态 | covered: B-003, B-005；同步原语 patch 更新由完整测试验证。 |
| 重试/幂等 | covered: B-001, B-002；对新 package ID 的 precise dry-run 不得产生额外变化。 |
| 非法状态转换 | N/A；不修改应用状态机。 |
| 兼容/迁移 | covered: B-001, B-003；停留在 0.10 兼容版本线，无数据迁移。 |
| 降级/回退 | covered: B-004；不得用 audit ignore 静默放行。 |
| 证据与审计完整性 | covered: B-002, B-004, B-005；diff、依赖树、audit 与完整 Rust 验证缺一不可。 |
| 取消/中断 | covered: B-002；lockfile 更新可从干净分支安全重做。 |

## 发布说明

AWS S3 checksum 依赖链不再锁定已撤回的 `spin 0.10.0`，改为兼容的 `0.10.1`；没有用户可见行为或 API 变化。
