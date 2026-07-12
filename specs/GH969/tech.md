# Tech Spec

## Linked Issue

GH-969 / #969

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Standard logout | `src/auth/user_management.rs:73-85` | JWT claims 中的 `session_id` 被插值到 info log | B-001 根因 |
| OAuth middleware | `src/auth/oauth/middleware.rs:207-220` | invalid/expired branch 将 `sid` 插值到 warn log | B-002 根因 |
| OAuth logout | `src/auth/oauth/handlers.rs:516-523` | delete success branch 将 `sid` 插值到 debug log | B-003 根因 |
| Existing PII guard | `scripts/guards/check_log_pii.sh:1-52` | 只扫描 raw request/response body，未覆盖 session identifier | B-006/B-007 prevention gap |
| PR lint workflow | `.github/workflows/ci.yml:41-66` | 运行多个 guard，但未运行 log PII guard | PR 回归未自动阻断 |
| Main lint workflow | `.github/workflows/ci-main-full.yml:42-55` | main full lint 未运行 log PII guard | merge 后回归未自动阻断 |

## 设计方案

1. 将三个日志分别改为静态文本：`Session invalidated`、`Invalid or expired session`、`Session deleted`。
   不绑定、不 hash、不读取 session 值来生成日志字段；保留原日志级别和所在 control-flow branch。
2. 扩展 `scripts/guards/check_log_pii.sh`：
   - 保留现有 raw-body scan 与 `LITELLM_LOG_PII_BASELINE_MAX` 行为；
   - 新增独立 multiline PCRE scan，查找 production `src/**/*.rs` 的 log macro 调用中对完整变量名
     `session_id`、`session_token` 或 `sid` 的直接引用；
   - 使用独立 `LITELLM_LOG_SESSION_IDENTIFIER_BASELINE_MAX`，默认且 CI 固定为 0；raw-body override 不得
     影响 session scan；
   - `rg` 缺失、session count 超阈值或 raw-body count 超阈值都非零退出。
3. 在 `.github/workflows/ci.yml` 和 `.github/workflows/ci-main-full.yml` 的 lint job 中新增
   `Log PII guard` step，调用同一脚本；不新建 workflow/job，不改变 check names 或权限。
4. 不修改 session store、JWT、middleware decision、handler response 或 redirect 构造。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | `src/auth/user_management.rs` static info message | base guard 红灯包含该 path；实现后 guard 为 0；diff 确认 branch/level 不变 |
| B-002 | `src/auth/oauth/middleware.rs` static warn message | base guard 红灯包含该 path；实现后 guard 为 0；既有 middleware/full tests |
| B-003 | `src/auth/oauth/handlers.rs` static debug message | base guard 红灯包含该 path；实现后 guard 为 0；既有 OAuth/full tests |
| B-004 | 三个 static messages | `rg` 禁止三个 log calls 引用 `session_id|session_token|sid`，人工确认无 hash/prefix/length 替代 |
| B-005 | 三个调用点的最小 diff | `git diff --unified=40 origin/main...HEAD` + `cargo test --all-features --locked -- --test-threads=1` |
| B-006 | `check_log_pii.sh` + two lint workflows | base red count=3；post-fix count=0；workflow `rg` 显示两个 exact step；PR Lint check 成功 |
| B-007 | guard 的独立 session baseline/pattern | 运行 body override 负例仍因 session 命中失败；实现后默认两类 count 均为 0；全日志 disposition |

## 数据流

Session credential 从 header/token/claims 进入既有验证或删除逻辑；日志 branch 只发射固定事件文本，不再把
credential 传给 formatter。CI 从 checkout source 运行 `check_log_pii.sh`，两类 scan 独立计数并 fail closed。
不新增持久化、网络调用或用户可见响应字段。

## 备选方案

- hash/prefix session ID：仍产生可跨请求关联值，违反 B-004，拒绝。
- 删除三条日志：安全但丢失事件/结果诊断，静态文本已能同时满足诊断和脱敏，拒绝。
- 只运行一次 `rg`、不扩展 CI guard：能证明当前 diff，不能阻断回归，违反 B-006，拒绝。
- runtime tracing capture tests：三个 control-flow path 需要复杂全局 subscriber/session-store fixture，并有并发测试干扰；
  source guard 对“变量直接进入 log macro”这一根因更小、更确定，拒绝本次引入。

## 风险

- Security: PCRE 必须以完整变量边界匹配，不能依赖日志 label；否则改文案可绕过。
- Compatibility: 日志消费者若解析旧的 identifier 字段会失去该字段，这是预期安全变化；事件文本保留。
- Performance: 三个静态日志减少格式化；source guard 只在 CI/本地运行。
- Maintenance: 新 credential 变量名不在闭集时需扩展 guard；全日志 disposition 用于发现此类缺口。

## 测试计划

- [ ] Red: 只改 guard 后运行 `bash scripts/guards/check_log_pii.sh`，精确报告三个 path 并非零退出。
- [ ] Negative isolation: 设置高 raw-body baseline 仍不能放行三个 session hits。
- [ ] Green: 三处日志改静态文本后，默认 guard 报 body=0/session=0 并成功。
- [ ] Audit: 搜索 auth/server 全部 session/token/sid 相关 log macros，并逐项分类 credential、metadata、error 或 protocol。
- [ ] Repository: format、all-feature check、strict Clippy、全量 serial tests、scope/overlap 和 CI checks。

## 回滚方案

不得恢复 session identifier 日志。若 guard 产生误报，forward-fix 完整变量边界或限定 production path，同时保留
三条静态日志；紧急情况下可暂时从 CI 移除 guard step，但不能回滚 production redaction，并必须单独跟踪 guard 修复。
