# Tech Spec

## Linked Issue

GH-957 / #957

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Auth method type | `src/auth/types.rs:38-48` | `AuthMethod` 派生 `Debug`，字符串字段被逐值输出 | 凭证泄露根因 |
| Authentication entry | `src/auth/system.rs:61-70` | `debug!("Authenticating request: {:?}", auth_method)` | live 日志触发点 |
| Type tests | `src/auth/types.rs` 的 `#[cfg(test)]` 模块 | 没有凭证格式化安全回归测试 | 需要锁定不变量 |

## 设计方案

1. 从 `AuthMethod` 移除派生 `Debug`，保留 `Clone`。
2. 为 `AuthMethod` 手写 `std::fmt::Debug`：
   - `Jwt(_)` 输出可识别的 `Jwt("[REDACTED]")`；
   - `ApiKey(_)` 输出可识别的 `ApiKey("[REDACTED]")`；
   - `Session(_)` 输出可识别的 `Session("[REDACTED]")`；
   - `None` 输出 `None`。
3. 不读取凭证内容生成 prefix、suffix、hash、长度或其他派生信息。固定占位符是唯一字段输出。
4. 保留 `AuthSystem::authenticate` 的现有日志事件；其 `{:?}` 将自动使用安全实现，避免在调用点重复
   redaction 逻辑。
5. 在 `src/auth/types.rs` 内添加聚焦测试。每个变体至少使用两个不同输入，并断言完整输出精确等于
   预期固定字符串；测试输入覆盖普通 secret、空字符串、Unicode/换行符和 `[REDACTED]`。精确相等与
   不同输入同输出共同排除 prefix、suffix、hash、长度和其他输入派生信息。
6. 用全仓定向搜索检查 `AuthMethod` 内部字段是否被其他日志调用绕过安全 formatter。已确认的独立
   session identifier 日志由 #969 跟踪，不扩展本 issue。

## Implementation Preconditions

1. `product.md` 与 `tech.md` 必须获得维护者的人类安全审查确认；agent 自检不能替代该 gate。
2. Draft PR #954 已修改 `src/auth/types.rs` 并包含同一实现候选。开始实现前必须将 #954 安全地拆分或
   标记 superseded，并确认没有 open PR 文件重叠。
3. 最终 implementation PR 在合并前必须再次取得针对当前 head 的人类 auth/security review。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 JWT 固定输出 | `AuthMethod` custom `Debug` | exact output + two-input equality |
| P2 API key 固定输出 | `AuthMethod` custom `Debug` | exact output + two-input equality |
| P3 session 固定输出 | `AuthMethod` custom `Debug` | exact output + two-input equality |
| P4 变体可区分 | custom `Debug` + tests | variant-name assertions |
| P5 固定占位符且无派生标识 | custom `Debug` | exact/structural output assertions |
| P6 认证行为不变 | 不修改 auth execution | focused auth tests + full suite |

## 数据流

HTTP credential extraction → `AuthMethod` → `AuthSystem::authenticate` debug event → custom `Debug` →
只输出 method kind + `[REDACTED]` → 原有认证验证流程继续执行。

## 备选方案

- 删除认证 debug 日志：能降低泄露面，但失去 method-level 诊断信息，且其他 `Debug` 调用仍可能泄露，拒绝。
- 在日志调用点手工匹配变体：只保护一个调用点，未来其他 `{:?}` 仍不安全，拒绝。
- 记录凭证 prefix/hash：仍产生可关联或可枚举信息，不符合产品不变量，拒绝。
- 引入通用 secret wrapper：范围大于本 issue，后续可单独设计；本次采用最小 custom `Debug` 修复。

## 风险

- Security: 变更降低凭证落日志风险；custom implementation 必须覆盖所有携带 secret 的变体。
- Compatibility: `Debug` 文本不是稳定公共协议，但依赖精确 debug 字符串的测试可能需要同步更新。
- Maintenance: enum 新增变体会触发 non-exhaustive match 编译失败，强制显式决定日志策略。

## 测试计划

- [ ] Unit: JWT/API key/session 分别验证精确固定输出和不同输入得到相同结果。
- [ ] Unit: `None` 可安全格式化。
- [ ] Unit: 控制字符与 Unicode secret 不进入输出。
- [ ] Focused: `cargo test auth::types --all-features`。
- [ ] Repository: format、check、strict Clippy、全量 tests 与 PR scope/overlap guards。

## 回滚方案

不得 revert 到派生字段输出。若 custom formatter 导致紧急兼容问题，安全回退是临时移除
`AuthSystem::authenticate` 的该 debug event，同时保留 custom `Debug`，随后 forward-fix 安全格式；绝不恢复原始
凭证日志。
