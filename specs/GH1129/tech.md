# Tech Spec

## Linked Issue

GH-1129 / #1129

## Product Spec

[`product.md`](./product.md)

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| 统一响应类型 | `src/core/types/responses/usage.rs` | `Usage` 使用 `u32` 分量；`Usage::new` 直接相加，但本 Issue 的 provider 解析器大多手工构造该类型 | 公开 schema 保持不变；解析边界必须在进入 `Usage` 前完成合法性与范围归一化 |
| 共享 provider 工具 | `src/core/providers/shared.rs` | 已承载跨 provider 的响应解析工具，但尚无严格 token usage helper | 搜索后确认这是复用逻辑的既有位置，不新增平行 utility 文件 |
| Azure | `src/core/providers/azure/chat.rs`, `src/core/providers/azure/embed.rs`, `src/core/providers/azure/chat_tests.rs` | 缺失或错误类型通过 `unwrap_or(0)` 退化为零，`as u32` 可截断 | chat 与 embedding 都在账单入口前产生 `Option<Usage>` |
| Azure AI | `src/core/providers/azure_ai/chat.rs`, `src/core/providers/azure_ai/embed.rs`, `src/core/providers/azure_ai/chat_tests.rs` | 与 Azure 相同；embedding completion 固定为不适用的零 | 需要保留格式差异，同时移除静默 fallback |
| Vertex AI | `src/core/providers/vertex_ai/transformers.rs`, `src/core/providers/vertex_ai/client.rs` | 两条非流式 response parser 都解析 `usageMetadata`/legacy metadata，存在默认零、截断或手写加法；legacy 路径还会把缺失 usage 展开为全零 `Usage` | 两条入口必须委托同一 helper，不能只修 transformer |
| Direct Gemini | `src/core/providers/gemini/client.rs`, `src/server/routes/ai/gemini/spend.rs` | provider client 与 native SDK unary/SSE 使用不同 parser；两者都默认零/直接缩窄，SDK parser 还独立执行有缺口的 no-usage settlement | 两条 parser 都必须使用 endpoint policy；SDK settlement 委托 common helper，不能留下旁路 |
| Bedrock | `src/core/providers/bedrock/transformation.rs` | 多种模型族各自解析，部分路径已有 all-zero → `None`，其他路径仍默认零并直接相加 | 需要统一 OpenAI-compatible、runtime alias、Claude、Converse、Titan 的边界语义 |
| Mistral embedding | `src/core/providers/mistral/embedding.rs` | 缺失或错误字段默认为零并截断 | completion 为不适用零，prompt/total 仍需严格验证 |
| 结算保护 | `src/server/routes/ai/spend.rs`, `src/server/routes/ai/spend_no_usage_tests.rs` | `usage: None` 会进入 `record_reserved_spend_without_usage`；有预留时按预留结算，无预留时显式报错且不记录 spend | provider 层必须把不可信 usage 路由到此既有保护，并补充账单副作用回归证明 |
| 流式解析 | `src/core/providers/base/sse/openai.rs` | 已对 usage 反序列化失败显式报错并返回 `None` | 本 Issue 不修改，作为既有 fail-closed 参考 |

## 设计方案

### 1. 统一严格解析原语

在既有 `src/core/providers/shared.rs` 中增加 crate-private helper，不改变公开 API：

- 只接受 `serde_json::Value::as_u64()` 成功的 JSON unsigned integer。
- 各 provider 先保留合法 raw `u64` 字段；需要相加的字段提升为 `u128`，禁止先
  饱和或缩窄。
- 由可信 raw prompt/completion 构造 `Usage` 时：
  - 两者均为零则返回 `None`；
  - provider total 若该格式声明存在，则必须是合法 unsigned integer，并在
    `u128` 域与该 endpoint 显式声明的 reported-total raw parts 之和一致；通常
    与 billable prompt/completion parts 相同，但 direct Gemini 是明确例外；
    先饱和再比较被禁止；
  - raw-domain 校验通过后，各分量以 `min(value, u32::MAX)` 缩窄，
    `total_tokens` 以 raw 总和饱和到 `u32::MAX`。
- helper 不猜测字段名；各 provider 适配器负责传入其格式声明的精确字段。
- completion 不适用的 embedding 路径显式传入合法常量 `0`，而不是把字段读取失败
  映射成 `0`。
- Google usage 的 optional `cachedContentTokenCount` 存在时也必须是 unsigned raw
  integer，并满足 `cached <= promptTokenCount`；raw 校验后饱和到 `u32::MAX`。
  provider `Usage` 保留 `prompt_tokens_details.cached_tokens/cache_read_tokens`，
  native SDK `PricingUsage` 保留 `cached_tokens`，确保 cache-read pricing 不漂移。

### 2. Provider 字段契约

| Provider format | Required prompt | Required completion | Reported total |
| --- | --- | --- | --- |
| Azure / Azure AI chat | `usage.prompt_tokens` | `usage.completion_tokens` | `usage.total_tokens` 必需且必须一致 |
| Azure / Azure AI embedding | `usage.prompt_tokens` | 不适用，显式 `0`；若响应带 completion，则必须是合法 `0` | `usage.total_tokens` 必需且必须一致 |
| Vertex `usageMetadata` | `promptTokenCount` + optional-zero `toolUsePromptTokenCount` | `candidatesTokenCount` + optional-zero `thoughtsTokenCount`；thoughts 只包含在 completion 总量，不写 `reasoning_tokens`/`thinking_usage` | `totalTokenCount` 必需，必须在 raw `u128` 域等于四项之和 |
| Direct Gemini `usageMetadata`（provider client + native SDK unary/SSE） | `promptTokenCount` + optional-zero `toolUsePromptTokenCount` | `candidatesTokenCount` + optional-zero `thoughtsTokenCount`；不写 separately-priced reasoning details，provider client 保留 user-visible `thinking_usage` | `totalTokenCount` 必需，但按 Gemini API 契约在 raw `u128` 域等于 prompt + candidates + thoughts，不含 tool-use prompt；公开/计费 total 仍由四个 billable parts 重算；optional cached count 严格保留 |
| Vertex legacy `metadata.tokenMetadata` | `inputTokens.totalTokens` | `outputTokens.totalTokens` | 格式不声明 total，统一重算 |
| Bedrock OpenAI-compatible | `prompt_tokens` | `completion_tokens` | `total_tokens` 必需且必须一致 |
| Bedrock runtime aliases | 已识别的 input 字段之一 | 对应格式的 output 字段 | 格式无 total 时统一重算；不得跨不完整 alias 组合拼接 |
| Bedrock Claude / Titan | 各模型族声明的 input 字段 | 各模型族声明的 output 字段 | 格式无 total 时统一重算 |
| Bedrock Converse | `usage.inputTokens` | `usage.outputTokens` | `usage.totalTokens` 必需且 raw-domain 一致 |
| Mistral embedding | `usage.prompt_tokens` | 不适用，显式 `0` | `usage.total_tokens` 必需且必须一致 |

Vertex 四项相加不是推断：Google 官方 Vertex AI v1
[`UsageMetadata`](https://cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse#usagemetadata)
明确把 `totalTokenCount` 定义为 `promptTokenCount + candidatesTokenCount +
toolUsePromptTokenCount + thoughtsTokenCount`。因此 helper 必须按四项 raw-domain
总和校验。Direct Gemini 的官方
[`UsageMetadata`](https://ai.google.dev/api/generate-content#v1beta.GenerateContentResponse.UsageMetadata)
则明确把 reported total 写成 prompt + thoughts + candidates；适配器必须向共享
helper 传入 endpoint-specific total parts，不能把 Vertex 四项公式套用过去。
两者的 billable input/output 都分别纳入 tool-use prompt/thoughts。

usage 容器缺失直接返回 `None`。容器存在时，任一必需字段缺失或类型错误都使用 `?`
传播为 `None`；不得保留另一半字段形成 partial usage。

### 3. Total 一致性

内部账单只信任通过 raw-domain 校验的 prompt/completion 分量。reported total 不参与
计费，只用于检测 provider schema 或数据漂移。比较必须在 `u128` 域完成：两个不同的
超大值即使都会缩窄为 `u32::MAX` 也不能被误判为一致。校验后才缩窄；所有调用方移除
普通 `+` 和 provider total 直赋值，统一使用 helper 的饱和结果。

Google `thoughtsTokenCount` 已被合入 `completion_tokens`，因此 pricing 只按标准
output token 单价计费一次。本 Issue 明确保持
`completion_tokens_details.reasoning_tokens=None`；当前 pricing authority 会在
completion cost 之外对 reasoning tokens 再计一笔，填充该 details 会重复收费。
Direct Gemini 现有 `thinking_usage` 只是 user-visible breakdown，当前
`PricingUsage::from` 不读取它，因此保留并断言兼容；不得把“防重复计费”扩大成删除
该公开信息。

### 4. Billing 副作用

转换器返回 `None` 后进入 `record_reserved_spend_without_usage`。该函数需做最小
修正以覆盖所有 reservation 组合：

1. 有 `UnifiedBudgetReservation` 时以其自身 `reserved_amount()` 结算 provider/model，
   避免免费调用；
2. 有 key budget reservation 时独立读取并以其自身 `reserved_amount()` 结算；
   两者同时存在且金额不同时不得复用 unified cost；
3. API-key usage 的零 token cost 优先使用 key reservation 自身金额；key reservation
   缺失时才使用 unified reserved amount。只有 key reservation 时也必须结算并记录，
   不得因 unified reservation 为 `None` 提前返回；
4. 只有 provider reservation 时保持现有 provider/model 与 API-key usage 行为；
5. 两种预留都没有时记录 error，且不伪造成功 spend。

native Gemini SDK 的 `settle_gemini_reserved_spend_without_usage` 不再维护平行算法：
把 common helper 调整为 crate-visible 内部入口并委托它，确保 unary/SSE 的
provider+key、provider-only、key-only、neither 与不同金额矩阵完全相同。

在 `spend_no_usage_tests.rs` 扩充下游测试，固定这些副作用，避免未来 provider 修复
再次被结算层弱化。

## Planned Change Manifest

| File | Planned change | Product invariants |
| --- | --- | --- |
| `src/core/providers/shared.rs` | 增加严格 unsigned token 读取、饱和 `u64 → u32`、all-zero 拒绝、reported-total 校验和饱和 total 重算 helper 及单元测试 | B-003–B-009 |
| `src/core/providers/azure/chat.rs` | 用共享 helper 替换三个默认零/截断读取 | B-001–B-009, B-011 |
| `src/core/providers/azure/chat_tests.rs` | 增加合法零分量、missing/type drift、全零、total mismatch、范围边界 fixture | B-003–B-009, B-011 |
| `src/core/providers/azure/embed.rs` | 严格读取 prompt/total，显式处理 embedding completion=0 | B-001–B-009 |
| `src/core/providers/azure_ai/chat.rs` | 应用与 Azure chat 相同的严格契约 | B-001–B-009, B-011 |
| `src/core/providers/azure_ai/chat_tests.rs` | 覆盖 Azure AI chat 的 drift、total、范围与兼容性 | B-003–B-009, B-011 |
| `src/core/providers/azure_ai/embed.rs` | 严格读取 prompt/total，completion 仅按格式语义为零 | B-001–B-009 |
| `src/core/providers/vertex_ai/transformers.rs` | 分别收紧 `usageMetadata` 与 legacy `tokenMetadata`，移除缺失 usage 的默认 `Usage`，统一调用共享 helper、重算 total，并扩充本文件测试 | B-001–B-009, B-011 |
| `src/core/providers/vertex_ai/transformers/split_tests.rs` | 覆盖 transformer 的 usageMetadata/legacy 合法、malformed、total、扩展字段与范围 fixture，避免继续膨胀已接近上限的实现文件 | B-001–B-009, B-011 |
| `src/core/providers/vertex_ai/client.rs` | trait response parser 的 `usageMetadata` 委托同一共享 helper，移除 `unwrap_or(0) as u32`/普通加法；覆盖扩展计数、malformed/total 与 exact token 输出 | B-001–B-009, B-011 |
| `src/core/providers/vertex_ai/client_tests.rs` | 覆盖 trait response parser 的扩展计数、malformed、all-zero、total mismatch 与范围 fixture | B-001–B-009, B-011 |
| `src/core/providers/gemini/client.rs` | direct Gemini 非流式 `usageMetadata` 委托共享严格 helper，但使用 Gemini-specific prompt+candidates+thoughts reported-total policy；移除默认零/直接缩窄与 separately-priced reasoning details，并在 inline tests 覆盖扩展计数、malformed/total、范围与 exact token 输出 | B-001–B-009, B-011 |
| `src/server/routes/ai/gemini/spend.rs` | native Gemini SDK unary/SSE `gemini_usage_metadata` 委托同一 strict direct-Gemini policy，保留 cached pricing、清空 reasoning cost；no-usage settlement 委托 common helper，并扩充 inline parser/四组合/不同金额测试 | B-001–B-012 |
| `src/core/providers/bedrock/transformation.rs` | 收敛所有模型族 usage parser，禁止 partial alias 拼接，移除默认零和普通加法，并扩充本文件测试 | B-001–B-009, B-011 |
| `src/core/providers/mistral/embedding.rs` | 严格读取 prompt/total、显式 completion=0，并扩充本文件测试 | B-001–B-009, B-011 |
| `src/core/pricing_service/tests.rs` | 用归一化 Vertex effective usage 做 exact cost 断言，证明 thoughts 只进入 output cost 且 reasoning cost 为零 | B-006, B-011 |
| `src/server/routes/ai/spend.rs` | 修正 no-usage key-only reservation 的提前返回，各 reservation 按自身金额恰好结算一次，为 API-key usage 选择 key-first fallback cost，并把 helper 暴露为 native Gemini SDK 可复用的 crate-internal 入口 | B-010, B-012 |
| `src/server/routes/ai/spend_no_usage_tests.rs` | 扩充无 usage 的 reservation、不同 provider/key 预留额、key budget、无 reservation 账单副作用测试 | B-010, B-012 |

不计划修改公开 `Usage` schema、已经 fail-closed 的 OpenAI SSE usage 解析或
价格/预算配置；native Gemini unary/SSE shared parser 按 manifest 在范围内。
若实现发现必须修改 manifest 外文件，应停止并回到 spec review。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | 所有 manifest 中 provider parser（含两条 Vertex、direct Gemini provider client 与 native SDK unary/SSE shared parser） | 每个 provider 的定向 parser test；审查不再存在受影响路径的 `unwrap_or(0) as u32` |
| B-002 | provider 容器入口 | 缺失 usage / usageMetadata / tokenMetadata fixture 返回 `None` |
| B-003 | shared helper 与每种字段映射 | missing、`null`、string、float、negative、object/array fixture；partial 字段返回 `None` |
| B-004 | chat 与 embedding 格式策略 | prompt=0/completion>0、prompt>0/completion=0、embedding completion=0 fixture |
| B-005 | shared builder | prompt=0/completion=0 返回 `None`，所有 provider 至少一条集成 fixture |
| B-006 | total 重算 | total 不能补齐缺失分量；无 total 格式得到分量饱和和 |
| B-007 | endpoint-specific reported-total validator | raw `u128` 域 total 缺失、错误类型、不一致均返回 `None`；一致 total 保留；Vertex 四项、direct Gemini 三项与 Converse 专项 fixture |
| B-008 | token conversion helper | `u32::MAX`、`u32::MAX + 1`、`u64::MAX` |
| B-009 | total builder | `u32::MAX + 1` 与 `u32::MAX + u32::MAX` 均得到 `u32::MAX` |
| B-010 | common + native Gemini SDK no-usage tests | 两条入口都验证 provider+key（含不同金额）、provider-only、key-only、neither 的 spend、API-key usage 与各 reservation exactly-once/self-amount settlement |
| B-011 | provider happy-path + Google pricing fixtures | 扩展字段零/缺失保持既有值和费用；非零时 effective input/output 各计一次，cached 保持 cache-read price，thinking breakdown 保留且 reasoning cost 为零 |
| B-012 | provider → spend handoff | malformed provider fixture 返回 `None`，下游 no-usage 测试证明不产生 `$0` 成功 spend |

## 数据流

1. provider 非流式响应被解析为 JSON。
2. 对应适配器按格式选择精确 usage 容器和必需字段。
3. 共享 helper 验证 JSON unsigned integer，在 raw `u128` 域重算并校验 reported
   total、拒绝 partial/all-zero usage，然后执行饱和转换。
4. 可信数据生成 `Some(Usage)`；不可信或缺失数据生成 `None`。
5. `Some(Usage)` 进入现有定价与实际 usage 结算；`None` 进入现有 reserved
   no-usage 结算与 error 日志。
6. 不新增持久化字段或外部调用。

## 备选方案

- **在每个 provider 内复制修复**：改动直观，但容易再次出现字段、溢出和 total
  语义漂移，拒绝。
- **仅在 spend 层把 `Some(0,0,0)` 转成 `None`**：能挡住 all-zero，但无法识别
  partial drift、截断和 total 不一致，也会丢失 provider 诊断上下文，拒绝。
- **错误类型直接让成功响应失败**：最严格，但改变用户可见请求成功语义；当前选择
  保留响应并通过既有 no-usage 结算 fail closed。

## 风险

- Security: 计费与预算属于高风险边界；任一 fallback-to-zero 都可能恢复免费调用。
  必须人工审查 provider 字段映射和所有账单副作用测试。
- Compatibility: malformed、partial、all-zero 或 total 不一致的响应将从
  `Some(Usage)` 变为 `None`；这是有意收紧。Vertex 扩展字段缺失/零的合法非零响应
  与公开 schema 不变；扩展字段非零时修正过去漏计的 token/cost。
- Performance: 每个响应增加常数级字段检查、`try_from` 和一次饱和加法，无额外 I/O
  或分配，影响可忽略。
- Maintenance: provider 字段策略必须保留在显式表驱动/局部映射中；不得让共享
  helper 猜测任意 alias。

## 测试计划

- [ ] Unit tests: shared helper 的类型、边界、all-zero、total consistency 全分支；
      每个 provider 格式的 happy path 与 drift fixture。
- [ ] Integration tests: provider parser 输出 `None` 后复用
      `spend_no_usage_tests.rs` 验证四种 reservation 组合和 key usage 副作用。
- [ ] Regression tests: 现有 Azure、Azure AI、Vertex AI、direct Gemini provider
      client/native SDK unary+SSE、Bedrock、Mistral 相关测试。
- [ ] Pricing tests: Vertex/direct Gemini tool-use prompt/thoughts 非零时，
      input/output cost 由四个 billable parts 精确计算，`reasoning_cost == 0`
      且 total 不重复；reported total 分别覆盖 Vertex 四项和 direct Gemini 三项；
      cached token 保持 cache-read price，provider client `thinking_usage` 保留。
- [ ] Static checks: `cargo fmt --check`、`cargo check`、
      `cargo clippy --all-targets -- -D warnings`。
- [ ] Full verification: `cargo test`；关键 usage helper 与结算分支要求 100% 覆盖，
      新增代码整体至少 80% line coverage。
- [ ] Manual verification: 使用一份合法 usage 与一份 partial/malformed usage 的
      provider fixture，确认前者正常计费，后者产生显式 no-usage error 且不记录
      `$0` 成功 spend。

## Security / Compatibility / Performance / Rollback

- **Security**：实现不得吞掉 malformed usage；review 必须检查所有列出的转换点、
  `as u32`、普通 token 加法和账单副作用。
- **Compatibility**：不改变外部 JSON schema 或 provider 请求；Vertex 扩展字段
  缺失/零的合法 usage 保持兼容，扩展字段非零时有意修正过去漏计的 token/cost；
  其他行为变化只收紧不可信响应。
- **Performance**：保持 O(1) CPU、O(1) 内存，不引入网络、锁或持久化。
- **Rollback**：代码可按单一实现提交整体 revert。若上线后某 provider 因真实格式
  差异大量进入 `None`，先通过日志确认字段事实，再补充明确格式映射；不得回退为
  `unwrap_or(0)`。紧急回滚时预算风险会恢复，必须同步告警并限制相关 provider。

## 回滚方案

回滚仅撤销严格 provider usage normalization 和对应测试，不迁移数据。回滚前需确认
是否会重新暴露零费用调用；若风险不可接受，应临时停用受影响 provider 或收紧其预算，
而不是恢复静默 fallback。Spec approval、实现、PR final review 和 merge 仍需各自的人类
门禁。
