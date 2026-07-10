# Tech Spec

## Linked Issue

GH-962 / #962

## Product Spec

See `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Canonical request | `src/core/types/chat.rs` | `ChatRequest` 已声明 typed `functions` 与 `function_call` | 字段在 provider 前仍存在 |
| HTTP boundary | `src/server/routes/ai/chat.rs` | transport DTO 已转换并保存两个 legacy 字段 | 证明丢失发生在 outbound serializer |
| OpenAI serializer | `src/core/providers/openai/client.rs` | `transform_chat_request` 手工插入 optional fields，但遗漏两个 legacy 字段 | OpenAI live 请求根因 |
| OpenAI-compatible serializer | `src/core/providers/openai_like/provider.rs` | 独立手工映射同样遗漏两个字段 | catalog/Tier-1 OpenAI-like live 请求根因 |
| Contract tests | `tests/openai_legacy_function_forwarding.rs` | 尚无 mock upstream 对两个 provider outbound body 的共同契约测试 | 需要锁定实际发送边界 |

## 设计方案

1. 保留现有 `ChatRequest` 与 provider-specific JSON transform 结构。
2. 在 OpenAI `transform_chat_request` 的 typed optional parameter 映射中插入 `functions` 与
   `function_call`，复用现有 fallible `serde_json::to_value` 路径。
3. 在 OpenAI-compatible transform 中做同样插入，并沿用该 provider 的
   `OpenAILikeError::serialization` 映射。
4. 两条路径都先写 typed fields，再用 `entry(...).or_insert(...)` 合并 `extra_params`，保持 canonical
   typed 值优先的既有规则。
5. 新增一个独立 integration test 文件，启动本地 mock HTTP upstream，分别通过
   `OpenAIProvider` 与 `OpenAILikeProvider` 的 live `chat_completion` 路径发送请求并捕获 JSON body。
   测试直接构造 canonical `ChatRequest`；字符串形式 `function_call` 只证明 provider contract，
   不声明扩展当前 HTTP DTO 的输入形状。
6. 不修改 `get_supported_openai_params`、capability matrix、transport DTO 或 response transformer；这些
   不属于本 issue 的 outbound field-loss 根因。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| `GH962-P1/P2` | OpenAI request transform | OpenAI mock upstream 精确 body 断言 |
| `GH962-P3` | OpenAI-compatible request transform | OpenAI-compatible mock upstream 精确 body 断言 |
| `GH962-P4` | 两个 transform | 同一请求同时携带 legacy 与 modern fields |
| `GH962-P5` | optional insertion | 无 legacy fields 的 transform 聚焦断言 |
| `GH962-P6` | typed insertion before `extra_params` merge | 同名 extra parameter 冲突测试 |

## 数据流

HTTP `functions` / `function_call` -> canonical `ChatRequest` -> selected OpenAI or OpenAI-compatible
provider -> provider `transform_chat_request` -> outbound JSON body -> upstream `/chat/completions`。

没有新增持久化、缓存、共享状态或外部协议转换。

## 备选方案

- 直接序列化整个 `ChatRequest`：会绕过 model mapping、provider-specific reasoning 处理和现有字段白名单，范围过大，拒绝。
- 把 legacy fields 放回 `extra_params`：继续保留两套来源并允许语义漂移，拒绝。
- 自动转换成现代 tool fields：会改变调用方选择的 OpenAI 协议，且无法保证完全等价，拒绝。
- 只添加 transform unit tests：能覆盖映射函数，但不能证明 live provider 发送边界，保留为补充而不替代 mock upstream contract test。

## 风险

- Security: 不新增 secret 或日志；测试使用固定假 API key，本地 upstream 仅绑定 loopback。
- Compatibility: 以前被静默丢弃的字段现在会到达上游；不支持 legacy contract 的上游可能显式返回 4xx，这是预期修复行为。
- Performance: 仅多序列化两个已存在的 optional JSON 字段，开销与 payload 大小线性一致。
- Maintenance: OpenAI 与 OpenAI-compatible 仍有两套手工 serializer；本 issue 只保持二者契约一致，不做架构合并。

## 测试计划

- [x] Red test: mock upstream 先证明当前两个 live provider body 都缺少 legacy fields。
- [x] Integration: 两个 provider 分别精确断言 `functions`、字符串/对象 `function_call` 与 typed precedence。
- [ ] Focused: `cargo test --test openai_legacy_function_forwarding --all-features --locked`。
- [ ] Repository: `cargo fmt --all -- --check`; `cargo check --all-features --locked`;
  `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked`。

## 回滚方案

该变更无 schema 或数据迁移。若出现 provider compatibility 回归，可回退两个 optional insertion 与对应
contract tests；回退会重新暴露字段静默丢失，因此优先通过 provider-specific 显式校验或错误映射 forward-fix。
