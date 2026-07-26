# Tech Spec

## Linked Issue

GH-1131 / #1131

## Product Spec

见 `product.md`（B-001 ～ B-007）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Example config | `config/gateway.yaml.example` | 声称 cache 未接线 | 主要错误文案 |
| Validation comment | `src/config/models/gateway.rs` | 把 warning 循环解释为整个 cache 未接线 | 与真实 warning scope 冲突 |
| Cache construction | `src/server/state.rs` | enabled + ttl 构造 response cache | 文案事实来源 |
| Chat cache | `src/server/routes/ai/{chat.rs,response_cache.rs}` | 非流式 lookup/store，含 budget/store/context bypass | 必须准确说明 |
| Embedding cache | `src/server/routes/ai/{embeddings.rs,response_cache.rs}` | 非流式 lookup/store，无 chat budget bypass | 端点差异 |
| Cache config | `src/config/models/cache.rs` | `semantic_cache` 仍产生 not-implemented warning | 不可误报已接线 |

## 设计方案

1. 只修改 `config/gateway.yaml.example` 的 cache 注释和
   `GatewayConfig::validate` warning 循环前的代码注释，不改字段、默认值或执行逻辑。
2. 文案逐项绑定当前代码：`enabled && ttl > 0`、非流式 chat/embeddings、
   caller-scoped key、chat 的 budget/`store`/context bypass、embeddings 差异、
   `ttl: 0` error/off、semantic cache 未实现。
3. 不把当前静默 bypass 描述成 warning，也不承诺未来行为。若实现与文案在审查时
   不一致，以代码为事实并缩小文案，不顺带改 runtime。
4. 复用现有 config example 解析/验证测试；新增或调整一个 focused 文案 guard，
   防止“response cache unwired”旧断言再次出现，但不对自然语言做脆弱的整段 snapshot。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001/B-002 | example 注释 | 关键事实字符串 + 代码路径人工核对 |
| B-003/B-004 | bypass 文案 | 与 `should_bypass_chat_cache`/embedding functions 对照 |
| B-005 | ttl 文案 | `build_response_cache`/config test |
| B-006 | validation comment | semantic warning 现有测试继续通过 |
| B-007 | diff scope | `git diff` 只有注释/文档，无行为字节变化 |

## 数据流

无运行时数据流变化。配置仍由同一 parser/validator 读取，AppState 仍按当前条件构造
cache，route 仍执行现有 lookup/store/bypass。

## 备选方案

- 删除全部 cache 注释：拒绝，不能向操作员暴露重要副作用和不对称行为。
- 顺便为 bypass 增加日志：超出 docs-only Issue，需独立行为规格。
- 把 semantic cache 也写成已接线：与当前 warning 和 registry 事实冲突。

## 风险

- Security: 文案涉及 caller 隔离，只描述已验证事实，不扩大保证。
- Compatibility: 无运行时变化。
- Performance: 无变化。
- Maintenance: 文案可能随 cache 行为漂移，focused test 只锁定关键否定事实。

## 测试计划

- [ ] Unit tests: cache warning 与 enabled-wired 现有测试。
- [ ] Config tests: example YAML parse/validate。
- [ ] Diff audit: 仅两处注释/文档变化。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、相关测试。

## 回滚方案

直接回滚文案 commit，无数据或配置迁移。若发现某条事实不准确，先修正文案并单独
建立运行时 Issue，不在 docs PR 中静默改变 cache。
