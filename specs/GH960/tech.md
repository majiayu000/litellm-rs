# Tech Spec

## Linked Issue

GH-960 / #960

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Required boundary |
| --- | --- | --- | --- |
| API-key auth core | `src/auth/system.rs` | `verify_key` errors become failed `AuthResult` with formatted internal detail | preserve `Ok(None)` invalid semantics but propagate `Err` |
| Shared public contract | `src/auth/mod.rs` | no common generic authentication-service message | define one crate-internal constant |
| Middleware mapping | `src/server/middleware/auth.rs` | propagated auth errors are formatted into `ErrorInternalServerError`, exposing detail | log original error and return generic 500 through existing envelope helper |
| Keys direct mapping | `src/server/routes/keys/access.rs` | propagated auth errors are logged but returned as generic 401 | log original error and return generic 500 |
| Core regression | `src/auth/tests.rs` | no real storage failure proves `AuthSystem` preserves `Err` | close the in-memory DB pool and assert API-key auth returns storage error |

## 设计方案

1. 在 `crate::auth` 定义 `pub(crate)` 固定消息 `Authentication service temporarily unavailable`，仅供服务端映射复用，不新增 public API。
2. `AuthSystem::authenticate_api_key` 保留成功与 `Ok(None)` 分支，将 verifier 的 `Err(error)` 直接返回。该层不构造 HTTP 状态或公开消息。
3. auth middleware 的 `Err` 分支：
   - 释放已有 auth rate-limit reservation，保持现有资源语义；
   - 不调用 failed-credential `record_failure`，避免 outage 触发 lockout/429；
   - 以 `error!(error = %error, ...)` 在服务端记录完整错误；
   - 调用私有 `authentication_unavailable_response`，用共享消息构造 500；
   - OpenAI-compatible path 继续经 `middleware_gateway_error_response` 生成 OpenAI envelope，其他路径使用 Actix 500。
4. keys access 的 `Err` 分支同样记录完整错误，并调用私有 helper，以 `KeyErrorResponse::internal` 与共享消息构造 500 `ApiResponse`。
5. 不让 response helper 接收原始 error 或任意 message，结构上阻止内部详情误传。
6. 回归测试：
   - real in-memory SQLite pool `close_by_ref` 后调用 API-key authentication，旧代码应返回 `Ok(AuthResult)`，新代码必须返回 `GatewayError::Storage`；
   - middleware 关闭 DB 后连续请求仍返回 500、包含共享通用消息且不包含 storage/Redis sentinel，不转成 429；
   - keys direct auth 先证明 invalid credential 为 401，再关闭 DB 证明同一路径返回通用 500；
   - 现有 invalid API-key middleware 回归继续证明 401 与 `Invalid API key`。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | unchanged `Ok(None)` branch | existing invalid API-key middleware 401 regression |
| P2/P7 | `AuthSystem::authenticate_api_key` | closed database pool returns `Err(GatewayError::Storage)` |
| P3/P6 | middleware private response helper and integration path | helper 500/generic body plus repeated closed-DB requests never become 429 |
| P4 | keys direct authentication path | invalid credential 401 followed by closed-DB generic 500 |
| P5 | crate-internal shared constant | both helpers import the same constant; code review and body assertions |

## 数据流

Invalid credential: `verify_key -> Ok(None)` -> failed `AuthResult` -> caller 401。

Infrastructure failure: `verify_key -> Err(GatewayError)` -> `AuthSystem::authenticate -> Err` -> caller logs full error -> fixed generic 500 response。

## 受影响文件与规模

- `src/auth/mod.rs`
- `src/auth/system.rs`
- `src/auth/tests.rs`
- `src/server/middleware/auth.rs`
- `src/server/routes/keys/access.rs`
- `tests/integration/auth_middleware_tests_parts/rejection_rate_limit.rs`
- `specs/GH960/product.md`
- `specs/GH960/tech.md`
- `specs/GH960/tasks.md`

预计 6 个 code/test 文件、少于 300 行 code diff，满足仓库 scope 限制；所有文件保持在 800 行以内。

## 备选方案

- 在 `AuthResult` 增加 error kind：已有 `Result<AuthResult, GatewayError>` 已能表达基础设施失败，增加第二套分类会重复建模，拒绝。
- 直接返回 `GatewayError::error_response()`：当前通用 error serializer 会使用原始 `to_string()`，仍可能泄露 storage detail，拒绝。
- 所有认证失败统一 500：破坏无效凭证 401 contract，拒绝。
- 同时修正 JWT database/error disclosure：不在 issue 的 API-key 证据与验收范围，留作独立工作，拒绝。

## 风险

- Security: 原始 error 只能进入服务端日志，不能传给任一 response constructor。
- Compatibility: 依赖“数据库故障也返回 401”的客户端将看到 500；这是预期纠正。
- Observability: 两个 HTTP 调用点必须保留 error-level full-detail log。
- Availability: 基础设施故障不计入 credential lockout；gateway 全局 rate limit 若独立启用仍保持其现有语义。
- Envelope: middleware OpenAI path 与 keys `ApiResponse` 结构不同，只共享状态/消息 contract，不强行统一 schema。
- Testing: 关闭 in-memory pool 的测试必须确认真实 query 失败，而不是构造 synthetic `GatewayError`。

## 测试计划

- Red phase: closed-pool AuthSystem test 先证明旧代码把 storage error 折叠为 `AuthResult`。
- Focused: AuthSystem propagation、middleware generic 500、keys generic 500、existing invalid-key 401。
- Deterministic: `cargo fmt --all -- --check`、`git diff --check`、all-features check、strict clippy。
- Repository: `cargo test --all-features --locked -- --test-threads=1`。
- Guards: `bash scripts/guards/check_pr_scope.sh origin/main`、`bash scripts/guards/check_pr_overlap.sh`。
- SpecRail: GH960 packet、implement route、current-head security reviewer、PR gate 与 runtime gate。

## 回滚方案

回滚 GH960 PR 即恢复旧 401/error-detail 行为；没有 schema、数据 migration 或 cache 格式变更。
