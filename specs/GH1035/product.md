# Product Spec

## Linked Issue

GH-1035 / #1035

complexity: small

## 用户问题

`Cargo.lock` 锁定了 crates.io 已撤回的 `spin 0.9.8`。该版本通过
`flume -> sqlx-sqlite -> sqlx -> SeaORM` 进入完整 feature 依赖图，导致供应链审计持续报告
yanked warning，并降低从锁文件重建依赖的长期可靠性。

## 目标

- 将完整 feature 依赖图中的 `spin 0.9.8` 精确更新到同一兼容版本线的 `0.9.9`。
- 消除 `cargo audit` 对 `spin 0.9.8` 的 yanked warning。
- 保持依赖图、产品行为、公开 API 和项目源码不变。
- 以最小且可审计的 lockfile diff 完成更新。

## 非目标

- 不升级 `flume`、SQLx、SeaORM 或其他落后依赖。
- 不处理独立的 `spin 0.10.0` yanked warning。
- 不修改 `Cargo.toml`、生产代码、测试、CI 或公开 API。
- 不用 audit ignore 隐藏 yanked warning。

## Behavior Invariants

1. B-001 `Cargo.lock` 不再解析到 `spin 0.9.8`，而解析到 `spin 0.9.9`。
2. B-002 lockfile diff 只包含 `spin` package 的版本/checksum，以及既有依赖项对该 package 的版本消歧引用；
   不能连带更新其他 package 版本或依赖集合。
3. B-003 active feature graph 中 `spin` 的反向依赖仍为 `flume -> sqlx-sqlite -> sqlx -> SeaORM`，应用功能和
   依赖 feature 不变；lockfile 中 `flume` 与 `lazy_static` 的既有 package 引用同步指向 `spin 0.9.9`。
4. B-004 `cargo audit` 不再报告 `spin 0.9.8` yanked warning；其他独立 warning 必须继续可见。
5. B-005 格式、all-target/all-feature 编译、strict Clippy 与全 feature 测试保持通过。
6. B-006 Spec PR 与 Impl PR 必须分离，Impl PR 关联 #1035 且不自动合并。

## 验收标准

- [ ] `Cargo.lock` 只把 `spin 0.9.8` package 更新为 `0.9.9`、更新 checksum，并同步既有依赖项中的
      `spin 0.9.8` 消歧引用；没有其他 package 版本或依赖集合变化。
- [ ] `cargo tree -i spin@0.9.9 --locked --all-features` 显示既有 `flume` 路径。
- [ ] `cargo tree -i spin@0.9.8 --locked --all-features` 不再找到匹配 package。
- [ ] `cargo audit` 不再报告 `spin 0.9.8`，且没有新增 ignore。
- [ ] Rust 格式、编译、Clippy 和全量测试通过。
- [ ] 独立 Spec PR 与 Impl PR 已创建，未自动合并。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | N/A；纯 lockfile 依赖解析变更。 |
| 错误与失败路径 | covered: B-004, B-005；审计或验证失败必须阻断完成声明。 |
| 授权/权限 | N/A；不改变认证或权限代码。 |
| 并发/竞态 | covered: B-003, B-005；同步原语实现更新后由全量测试验证现有并发行为。 |
| 重试/幂等 | covered: B-001, B-002；精确 update 重跑不得产生额外 lockfile 变化。 |
| 非法状态转换 | N/A；不修改应用状态机。 |
| 兼容/迁移 | covered: B-001, B-003；停留在 `spin` 0.9 兼容版本线，无数据迁移。 |
| 降级/回退 | covered: B-004；不得用 audit ignore 静默放行。 |
| 证据与审计完整性 | covered: B-002, B-004, B-005；diff、依赖树、audit 与完整 Rust 验证缺一不可。 |
| 取消/中断 | covered: B-002；lockfile 更新可从干净分支安全重做。 |

## 发布说明

依赖锁文件不再使用已撤回的 `spin 0.9.8`，改为兼容的 `0.9.9`；没有用户可见行为或 API 变化。
