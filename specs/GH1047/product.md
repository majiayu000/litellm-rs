# Product Spec

## Linked Issue

GH-1047 / #1047

complexity: medium

## 用户问题

authoritative `users` 表中的 `role` 和 `status` 是认证与授权输入。当前实体转换会把未知 role 静默改为
`UserRole::User`，把未知 status 静默改为 `UserStatus::Pending`。更严重的是，写入端支持并保存合法的
`deleted`，读取端却不识别它，同样返回 `Pending`，破坏用户生命周期状态的往返一致性。

ID、username、email 三个 canonical 查询入口都会接收这些合成的默认用户。JWT 认证还把数据库错误和转换错误
统一伪装成 “User not found”，导致损坏数据与基础设施故障不可观察、不可区分。

## 目标

- authoritative canonical user 的 role/status 必须严格解析，非法值显式失败。
- 所有已声明的 `UserStatus`（包括 `Deleted`）必须无损往返。
- ID、username、email 查询都传播同一转换错误，真实缺失仍保持 `Ok(None)`。
- JWT 认证必须区分真实用户缺失与存储/转换故障，后者通过现有 `Result` 错误通道传播。
- 错误提供稳定字段上下文，但不泄露 raw persisted value、password hash 或 credential。
- 有效用户创建、查询、RBAC、active 判定和 legacy bridge 行为保持不变。

## 非目标

- 不增加 schema constraint、migration 或自动修复已有损坏 row。
- 不改变合法 role 集合、权限含义或角色继承。
- 不改变合法 status 的 active/inactive 判定。
- 不修改 legacy `um_users` JSON 格式或其转换规则。
- 不把本 issue 扩展为所有认证错误文案或所有数据库实体转换重构。

## Behavior Invariants

1. B-001 非空 canonical `role` 必须解析为声明的 `UserRole`；未知值返回 typed `GatewayError`，不得合成为 `User`。
2. B-002 非空 canonical `status` 必须精确解析 `active`、`inactive`、`pending`、`suspended`、`deleted`；未知值返回错误，不得合成为 `Pending`。
3. B-003 每个合法 role/status 都保持 entity → domain 转换语义，尤其 `deleted` 必须转换为 `UserStatus::Deleted`。
4. B-004 canonical ID、username、email 查询遇到损坏 row 都返回错误；真实无匹配 row 仍返回 `Ok(None)`，不得回退到 legacy 或返回部分/default 用户。
5. B-005 JWT access-token 认证只对真实 `Ok(None)` 返回 “User not found”；数据库或转换错误必须通过 `Result` 传播，不得伪装为认证拒绝。
6. B-006 转换错误只包含稳定的 entity/field/category 上下文，不包含 raw role/status、username、email、password hash、token 或其他 credential。
7. B-007 valid canonical user 的创建、读取、更新、RBAC、active 判定和 canonical/legacy bridge 保持现有行为。

## 验收标准

- [ ] malformed role 对 ID、username、email 三条 canonical lookup 都显式失败。
- [ ] malformed status 对三条 canonical lookup 都显式失败。
- [ ] `Deleted` 写入后读取仍为 `Deleted`，其余合法 role/status 覆盖回归测试。
- [ ] missing user 仍是 `Ok(None)`，不得与 conversion/storage error 合并。
- [ ] JWT authentication 对 lookup error 返回 `Err`，对真实 missing user 保持现有失败结果。
- [ ] 错误断言证明包含字段名，但不包含 fixture 原文、用户名、邮箱、password hash 或 token。
- [ ] focused SQLite/auth tests、格式、全特性编译、strict Clippy 和全量测试通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-002, B-004；空字符串是非法 persisted value，no row 保持 `None`。 |
| 错误与失败路径 | covered: B-001, B-002, B-004, B-005；转换和 DB 错误完整传播。 |
| 授权/权限 | covered: B-001, B-003, B-005, B-007；role 不再被合成，valid RBAC 不变。 |
| 并发/竞态 | N/A；只改变 row 转换与错误传播，不新增共享状态或写事务。 |
| 重试/幂等 | covered: B-004；重复读取损坏 row 持续失败且不修改数据。 |
| 非法状态转换 | covered: B-002, B-003；未知值不进入 domain，`Deleted` 不再变成 `Pending`。 |
| 兼容/迁移 | covered: B-003, B-007；合法 row 无需迁移，损坏 row 从静默默认变成显式错误。 |
| 降级/回退 | covered: B-001, B-002, B-004, B-005；禁止 default、legacy fallback 和错误伪装。 |
| 证据与审计完整性 | covered: B-006；保留字段上下文且不泄露 persisted secret/data。 |
| 取消/中断 | covered: B-004；读取失败没有写入或部分持久化，可安全重试。 |

## 发布说明

canonical 用户的 role/status 损坏时现在会显式失败；合法 `Deleted` 状态可正确往返，认证不再把存储故障误报为用户不存在。
