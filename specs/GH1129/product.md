# Product Spec

## Linked Issue

GH-1129 / #1129

complexity: high

## 用户问题

部分 provider 的非流式响应在 `usage` 存在但字段缺失、类型错误或无法解析时，
会把对应 token 数默认为 `0`，最终生成看似有效的零 usage。下游因此跳过
“provider 未返回 usage”的显式保护，把成功请求记录成零 token、零费用，预算也
可能不扣减。合法但超过内部范围的计数还会被截断成更小值。

## 目标

- 对受影响的 provider 非流式与明确列出的 Gemini 流式入口使用一致、可测试的
  usage 有效性规则。
- 明确区分合法的单字段零值、字段缺失、字段类型错误和整体零 usage。
- 对部分字段漂移采取 fail closed 行为，不猜测或拼接不可信 usage。
- 以饱和语义处理合法的大整数，并保证 `total_tokens` 与分量一致。
- 让不可信 usage 进入既有的缺失 usage 结算保护，避免静默免费调用。

## 非目标

- 改变模型价格、预算上限、预留算法或 provider 请求协议。
- 猜测未声明的新字段名、字符串数字、浮点数、负数或任意嵌套数字的含义。
- 重写已经具有显式错误处理的 OpenAI SSE usage 解析；标准 chat-completions Gemini
  stream 与 native Gemini SDK SSE 仍经过不严格 Google usage parser，因此这两条
  流式入口在本 Issue 范围内。
- 扩大公开 `Usage` 字段的整数类型或改变公开响应 schema。

## Behavior Invariants

1. **B-001** Azure chat/embed、Azure AI chat/embed、Vertex AI、direct Gemini
   provider client、标准 chat/completions/responses Gemini stream、native Gemini
   SDK unary/SSE shared parser、Bedrock 和 Mistral embedding 的受影响 usage 必须
   遵循同一可信度规则。
2. **B-002** usage 容器不存在时，响应必须表现为 `usage: None`；不得构造默认
   零 usage。
3. **B-003** 每种已声明 provider 格式的必需 token 分量必须存在且为 JSON
   unsigned integer；任一必需分量缺失、为 `null`、字符串、浮点、负数或其他
   类型时，整条 usage 必须 fail closed 为 `None`。
4. **B-004** 合法解析出的单个零分量不得单独导致 usage 被丢弃：chat 的
   prompt-only 或 completion-only 非零 usage 必须保留；embedding 中不适用的
   completion 分量必须保持合法零值。
5. **B-005** 所有适用分量均合法但 prompt 与 completion 都为零时，整条 usage
   必须表现为 `None`，不得进入零费用成功记账路径。
6. **B-006** provider 报告的 `total_tokens` 不得替代缺失的 prompt 或 completion
   分量；输出 `total_tokens` 必须由可信原始分量重新计算。Vertex/direct Gemini
   `usageMetadata` 的有效 input 为 `promptTokenCount + toolUsePromptTokenCount`，
   output 为 `candidatesTokenCount + thoughtsTokenCount`，后两个扩展字段缺失时按该
   API 的可选零语义处理。`thoughtsTokenCount` 已包含在公开 `completion_tokens`
   与 output token 费用中，本 Issue 不再把它写入会另行计价的
   `completion_tokens_details.reasoning_tokens`，禁止重复计费；现有 user-visible
   `thinking_usage` breakdown 不参与 pricing，必须保留。
   Vertex reported-total 的四项关系以
   [Google Vertex AI v1 `UsageMetadata`](https://cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/GenerateContentResponse#usagemetadata)
   的公开契约为准：`totalTokenCount` 是 prompt、candidates、tool-use prompt 与
   thoughts 四项之和。Direct Gemini 当前
   [Gemini API `UsageMetadata`](https://ai.google.dev/api/generate-content#v1beta.GenerateContentResponse.UsageMetadata)
   则把 reported `totalTokenCount` 定义为 prompt、thoughts 与 candidates 三项之和，
   不含单列的 tool-use prompt；因此两条 endpoint 必须共享严格字段解析，但使用
   各自明确的 raw-domain reported-total policy。不得把扩展字段误认为已包含在
   前两项中，也不得把 Vertex 的四项公式套到 direct Gemini。
7. **B-007** 对声明包含 total 的 provider 格式（包括 Bedrock Converse），total
   缺失、类型错误或与原始分量之和不一致时，整条 usage 必须 fail closed 为
   `None`；对声明不包含 total 的格式，系统只使用重算值。
8. **B-008** 合法的 `u64` token 数超过 `u32::MAX` 时必须在 raw-domain total
   校验完成后饱和为
   `u32::MAX`，不得回绕、截断为更小值或 panic。
9. **B-009** `total_tokens` 必须使用饱和加法；即使两个分量各自可表示，相加也
   不得溢出为更小值。
10. **B-010** `usage: None` 必须触发完整缺失 usage 结算：provider/model reservation
    与 API-key reservation 各自按自身 `reserved_amount()` 结算；两者同时存在但
    预留额不同时不得复用另一方金额。API-key usage 使用 key reservation 金额，
    key reservation 缺失时才使用 provider reservation 金额；两者都不存在时记录
    显式错误且不得伪造零费用 spend。任一 reservation 不得因另一种 reservation
    缺失而提前返回或泄漏。native Gemini SDK 独立 settlement 路径必须委托同一
    结算 helper，不得恢复 key-only `0.0` 或把 unified amount 复用于 key。native
    Gemini SSE 一旦已收到任意 upstream output，随后即使在 usage 到达前发生 read
    error，也必须把真实 `saw_upstream_output=true` 传入 settlement 并按无 usage
    保护结算；无 upstream output 的 read error 不得伪报为已产生输出。
11. **B-011** 合法、范围内且 total 一致的既有 provider usage，在 Google 扩展字段
    缺失或为零时，其公开 token 值和既有计费结果必须保持兼容。非零扩展字段时，
    公开 input/output 必须包含真实 tool-use/thoughts token；可选
    `cachedContentTokenCount` 必须严格解析、不得超过 raw prompt、饱和后保留到
    cache-read pricing details；direct Gemini user-visible `thinking_usage` 保留，
    但 separately-priced reasoning details 为空。费用按归一化 input/output 各计
    一次，这是从漏计到正确计费的有意修正。
12. **B-012** 解析失败不得仅记录低级别日志后继续生成成功的零 token、零费用
    账单副作用。
13. **B-013** 标准 Gemini/Vertex SSE 必须区分 Missing、Valid 与 Invalid usage：
    Missing 不改变累计 Valid；后续 Invalid 必须清除旧 Valid，后续权威 Valid 可恢复。
    EOF 与 upstream read error 前必须先处理残留 SSE buffer；截断的
    `usageMetadata` 必须成为 Invalid。provider stream 必须在私有 accumulator 中
    保存状态，剥离内容 chunk 上的 usage，仅在终态仍为 Valid 时发送一个合法
    usage-only chunk；终态 Invalid/Missing 不发送 usage。chat、completions 与
    responses 三条标准路由只能收到无内部标记、无伪零 usage 的公开 `ChatChunk`，
    并把 Invalid 的 `None` 交给既有 no-usage reservation settlement。

## 验收标准

- [ ] 每个受影响 provider 格式均覆盖合法 usage、容器缺失、必需字段缺失、
      错误类型、部分字段漂移和全零 usage。
- [ ] prompt-only、completion-only 以及 embedding 合法零 completion 的行为符合
      B-004。
- [ ] `u32::MAX`、`u32::MAX + 1`、`u64::MAX` 和分量相加溢出场景均证明饱和且
      不回绕。
- [ ] 对带 total 的格式覆盖一致、缺失、错误类型和 raw-domain 不一致 total；对不带 total
      的格式证明输出 total 来自重算。
- [ ] Vertex 两条、direct Gemini provider client、标准 chat-completions Gemini
      stream 与 native Gemini SDK unary/SSE shared parser 都覆盖
      malformed/overflow/thoughts/tool-use prompt/cache；
      cost 断言证明 input/output 各计一次且 thoughts 不产生额外 reasoning 费用；
      Bedrock Converse 覆盖必需 `totalTokens`。
- [ ] 下游验证证明不可信 usage 的 provider+key、provider-only、key-only 与无
      reservation 四种路径都终止正确；provider+key 使用不同预留额时各自按自身
      金额结算；common 与 native Gemini SDK settlement 都不记录 `$0` 成功计费或
      遗留 reservation。
- [ ] native Gemini route-level SSE 测试证明：先收到 output、再发生 upstream read
      error 且没有 usage 时，caller 传递真实 output 状态并按各 reservation 自有金额
      结算；error 发生在任何 output 前时不伪造已输出结算。
- [ ] standard Gemini/Vertex stream 覆盖 Valid→Invalid、Valid→Missing、
      Valid→Invalid→Valid、EOF truncated 与 read-error residual buffer；三个标准
      路由最终序列化、direct provider consumer 均永不出现内部 marker/伪零 usage，
      Invalid 后继续使用 no-usage reservation settlement。
- [ ] 未声明字段名和嵌套形状不会被猜测为 usage。
- [ ] 不含 Vertex/direct Gemini 扩展计数的合法非零 usage 兼容性回归测试通过；
      扩展计数非零的 token/cost 修正，以及两条 endpoint 各自的 reported-total
      公式都有精确断言；cached count 保持 cache-read price，`thinking_usage`
      保留而 reasoning cost 为零。

## 边界情况

- chat 可能合法报告 prompt 为零或 completion 为零，但不能缺少该格式规定的字段。
- embedding 没有 completion 计费分量；该零值是格式语义，不是解析 fallback。
- JSON unsigned integer 可能刚好等于或远大于 `u32::MAX`。
- 两个归一化分量都可表示时，它们的和仍可能超过 `u32::MAX`。
- provider 可能提供 total 但遗漏一个分量，或提供与分量和不一致的 total；两种
  情况都不能被当作可信 usage。
- 极少数真实响应可能明确报告全零 usage；在计费边界上仍按无可信 usage 处理，
  以避免静默免费调用。
- 流式响应可能在已发送 candidate/output 后、最终 usage chunk 前断开；是否进入
  no-usage settlement 必须取决于真实 upstream output 状态，而不是终止分支常量。
- 标准 SSE 可能在同一网络读取中包含多个 usage event；Invalid 必须按事件顺序覆盖
  旧 Valid，而不是把一批 chunks 聚合后再猜测最终状态。

## 发布说明

这是计费正确性收紧。过去被默认为零 token、零费用的 malformed、partial 或
all-zero usage 将进入既有缺失 usage 保护。合法非零 usage 无需迁移；若上游
provider 已发生字段漂移，运营日志将显式暴露该问题。Vertex 与 direct Gemini
非流式、标准 chat stream、native SDK unary/SSE 的 tool-use prompt/thoughts token
现在分别计入 input/output 费用一次，并各自使用官方 endpoint-specific
reported-total 公式校验。native Gemini SSE 在已输出后遇到 read error 时不再释放
未记账的 reservation。标准 Gemini/Vertex SSE 的 malformed 或截断后续 metadata
也不再复用较早、较低的累计 usage。
