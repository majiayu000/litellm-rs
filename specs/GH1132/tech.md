# Tech Spec

## Linked Issue

GH-1132 / #1132

## Product Spec

见 `product.md`（B-001 ～ B-008）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Dev example | `config/gateway.dev.yaml.example` | `source: null` + `reject`，而 vLLM model 无价格 | 根因 |
| Main example | `config/gateway.yaml.example` | 生产取向的 fail-closed 定价 | 必须保持 |
| Config tests | `src/config/mod.rs`, `tests/integration/config_validation_tests.rs` | 主示例有 schema 验证，dev 只锁定旧 broken 值 | 漂移原因 |
| Pricing loader | `src/core/pricing_service/{mod.rs,loader.rs}` | `embedded://model_prices_extended` 离线加载 | cwd-independent source |
| Unpriced policy | `src/server/routes/ai/spend/unpriced.rs` | `reject` 预调用失败；`allow_unpriced` 使用 fallback | 端到端行为 |

## 设计方案

1. 把开发示例 `pricing.source` 设为既有
   `embedded://model_prices_extended`，保持 `allow_degraded: false`。
2. 仅在开发示例把 `unpriced_model_policy` 设为 `allow_unpriced`，并显式把
   `unpriced_fallback_cost_per_1k_tokens` 设为 `0.0`。注释说明该零价只代表本地
   自托管模型没有供应商成本；生产示例继续 `reject`。
3. 扩展 config schema/conformance test，使两个随仓库发布的示例都从
   `env!("CARGO_MANIFEST_DIR")` 派生稳定路径、完成反序列化和 `Config::validate`。
4. 建立一个共享测试 helper：加载 embedded pricing map，遍历 dev example 所有
   enabled provider models。每个 model 必须在 map 中，或 policy 为
   `allow_unpriced` 且 fallback 明确存在。禁止只断言 enum 值而不验证所有 model。
5. 增加 focused pricing regression：对 dev vLLM model 调用现有 unpriced cost/
   reservation helper，断言得到显式零 fallback 而不是 `model_not_priced`；对一个
   embedded 已知模型断言使用 map price，不走 fallback。测试不启动真实 provider。
6. 更新现有 integration test 的 broken-state 断言，不删除其他 auth/server/provider
   断言，不修改测试基础设施。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001/B-008 | embedded source + manifest path | 非 cwd 启动的 config test，无网络 |
| B-002/B-003 | dev unpriced policy | vLLM focused pricing helper 不返回 reject |
| B-004 | pricing service | embedded known model 使用真实条目 |
| B-005 | main example unchanged | production policy regression |
| B-006 | shared example conformance | 两个 YAML parse/deny unknown/validate |
| B-007 | enabled model iteration | 所有 dev enabled models 覆盖断言 |

## 数据流

dev YAML 由现有 Config loader 读取；PricingService 根据 embedded URI 从编译进二进制
的数据构造 map。请求模型有条目时按条目计费；本地 vLLM 模型无条目时，
`allow_unpriced` 显式选择 `0.0` fallback，预算预留不再返回
`model_not_priced`。无网络或 cwd 依赖。

## 备选方案

- 只把 `source` 设为 embedded 但保留 `reject`：拒绝，示例 vLLM model 仍无价格。
- `source: null` + `allow_unpriced`：能请求但丢失所有已知模型价格，不满足 B-004。
- `allow_degraded: true`：语义是加载失败降级，不能修复有意的 unpriced model。
- 给示例模型伪造非零价格：拒绝，不猜测本地基础设施成本。

## 风险

- Security: 宽松策略必须严格限制在 dev example，测试锁定 production 不变。
- Compatibility: 复制 dev example 的用户会从全拒绝变为显式零价，这是预期修复。
- Performance: embedded map 本已是默认能力，无网络与显著额外成本。
- Maintenance: 新增 dev model 时全模型 test 会要求价格或明确 fallback。

## 测试计划

- [ ] Unit tests: 两个 example parse/validate、enabled model coverage。
- [ ] Pricing tests: vLLM zero fallback、known model embedded price、load failure fail-closed。
- [ ] Integration tests: 保留 auth/server/provider 断言并更新 pricing 事实。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚 dev example 与测试即可，无持久化迁移。若零 fallback 不适合某部署，操作员可在
自己的配置恢复 `reject` 或设置实际价格；生产示例不受影响。
