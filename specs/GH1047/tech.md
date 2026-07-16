# Tech Spec

## Linked Issue

GH-1047 / #1047

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| User entity conversion | `src/storage/database/entities/user.rs` | unknown role → `User`; unknown status and legal `deleted` → `Pending` | B-001/B-002/B-003 root cause |
| Canonical queries | `src/storage/database/seaorm_db/user_ops.rs` | three query methods map an infallible converter and cannot propagate corruption | B-004 propagation gap |
| JWT authentication | `src/auth/system.rs` | `if let Ok(Some(user))` collapses both `Err` and `None` into “User not found” | B-005 observability gap |
| Domain enums | `src/core/models/user/types.rs` | `UserRole::from_str` already validates roles; `UserStatus` declares five variants | canonical validation source |
| Existing DB tests | `src/storage/database/seaorm_db/user_repository_tests.rs` | exercises canonical/legacy bridge but not corrupt role/status or Deleted round-trip | focused regression home |

## 设计方案

1. 将 `user::Model::to_domain_user` 改为 `Result<User>`，复用 `UserRole::from_str`，并通过一个不回显原值的
   field-context mapper 转成 `GatewayError`。status 使用完整显式 match，包含 `deleted`；unknown branch 返回同类错误。
2. 错误消息只报告 `users.role` 或 `users.status` 及 invalid persisted enum category。不得 format `self.role`、
   `self.status`、username、email、password hash、row debug 或 token。
3. ID、username、email 三个 canonical query 使用 `Option::map(...).transpose()`，使 no row 保持 `Ok(None)`，
   present invalid row 变为 `Err`。高层 fallback 只有在 canonical query 确实返回 `None` 时才可继续。
4. `authenticate_jwt` 将 user lookup 改为三分支 match：`Ok(Some(user))` 保留 active 判定；`Ok(None)` 保留
   “User not found”；`Err(error)` 直接返回 `Err(error)`。token verification error 和 inactive account 行为不变。
5. 新建独立 `user_state_corruption_tests.rs`，用 in-memory SQLite migration 插入 valid user，再直接更新 role/status
   制造损坏状态，覆盖三条 lookup、missing row、所有合法 enum、Deleted round-trip和错误脱敏。认证错误传播测试放在
   现有 auth test module 或使用最小 storage fixture；不得通过放宽断言实现。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | fallible role conversion | malformed role DB test; source search confirms no role fallback |
| B-002 | exhaustive status conversion | malformed status DB test; unknown value returns error |
| B-003 | complete enum mapping | table-driven valid role/status tests and explicit Deleted round-trip |
| B-004 | three query transposes | ID/username/email corruption tests plus missing-row regression |
| B-005 | JWT lookup match | auth test distinguishes injected lookup error from `Ok(None)` result |
| B-006 | redacted conversion error | sentinel values and identity/credential fixtures absent from rendered error |
| B-007 | minimal conversion/query/auth diff | existing user repository, auth, RBAC and full test suites |

## 数据流

SeaORM row → strict fallible entity conversion → `Result<Option<User>>` → canonical lookup/auth consumer。只有完整合法的
role/status row 能进入 domain 与 RBAC。无 row 保持 `None`；损坏 row 和数据库故障沿既有 `GatewayError` 通道上抛。
本变更不写回、不清理损坏 row，因而保留故障证据。

## 备选方案

- 未知 role 继续降级为 `User`：虽为权限降级，仍会隐藏 authoritative corruption 并造成用户访问异常，违反 B-001。
- 未知 status 继续降级为 `Pending`，只补 `deleted`：仍把未来/损坏值伪装为合法生命周期状态，违反 B-002。
- 记录 warning 后使用默认值：运行仍携带错误 domain state，且日志可能泄露 raw value，违反 B-001/B-006。
- 只修实体转换、不修 JWT：数据库错误仍被报告为用户不存在，违反 B-005。
- migration 修复历史值：修复策略具有业务含义且可能不可逆，超出本 issue。

## 风险

- Compatibility: 已损坏 rows 从可读默认用户变为显式错误；这是预期 fail-closed 行为。
- Availability: 损坏账户的 lookup/auth 会失败，避免基于合成身份继续运行；其他合法 row 不受影响。
- Security: error construction 不能携带 raw enum、identity、hash 或 token；测试使用唯一 sentinel 证明脱敏。
- Control flow: `Option::transpose` 必须只阻止 invalid present row，不能把真实 missing user 改成错误。
- Bridge: canonical invalid row 不能被 legacy fallback 覆盖，否则仍会隐藏 authoritative corruption。

## 测试计划

- [ ] Red: 在生产改动前加入 corruption/Deleted tests；确认 malformed role/status 和 Deleted round-trip 断言失败。
- [ ] Entity/DB: 三条 canonical lookup 对 malformed role/status 均返回 `Err`。
- [ ] Enum: 所有合法 roles/statuses 精确映射，`Deleted` 显式覆盖。
- [ ] Missing: 不存在的 ID/username/email 保持 `Ok(None)`。
- [ ] Auth: lookup error 从 JWT authentication 返回 `Err`；missing/inactive 保持原结果。
- [ ] Redaction: error 包含 `role`/`status` field context，但不包含 sentinel/raw identity/hash/token。
- [ ] Repository: format、all-target/all-feature check、strict Clippy 和全量 serial tests。

## 回滚方案

不得恢复 permissive enum fallback 或 JWT error swallowing。若历史坏数据导致兼容问题，应提供独立只读诊断/显式修复流程；
紧急 forward-fix 可改善错误分类或管理员提示，但必须继续阻止损坏 row 进入 domain/auth boundary。
