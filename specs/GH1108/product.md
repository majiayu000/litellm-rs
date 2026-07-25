# Product Spec

## Linked Issue

GH-1108 / #1108

- `complexity: large`
- `spec_approval: pending_maintainer`
- `draft_source: 2026-07-26 current conversation ("implxauto解决所有的issue和prs")`
- `required_approval: maintainer approval bound to the final spec head`

## 用户问题

Gemini Developer API 的静态模型目录落后于 Google 2026-07-21 的正式发布：
`gemini-3.6-flash` 与 `gemini-3.5-flash-lite` 已 GA，但 provider 仍无法准确公开、
校验和计费这两个 exact model ID。与此同时，目录中还混有已退役、未证实或不属于
Developer API 通用 chat 入口的 ID；只追加两个新 ID 会继续把不可兑现的模型支持暴露给
用户。

新模型还收紧了请求契约：`temperature`、`top_p`、`top_k` 不应再发送，最后一个非空
turn 为 `model` 的 prefill 请求必须被拒绝。若 SDK 仍声明这些参数可用、静默删除参数，
或把错误留给上游通用 `400`，用户会得到互相矛盾且难以定位的行为。

本变更需要把“模型存在、仍可调用、价格正确、请求契约可执行”作为一个可审计的
Developer API 行为面，同时保持 Vertex AI、Interactions API 和其他产品入口独立。

## 官方证据基线

本 spec 于 2026-07-26 依据以下 Google 官方页面起草：

- `gemini-3.6-flash` 与 `gemini-3.5-flash-lite` 于 2026-07-21 GA：
  <https://ai.google.dev/gemini-api/docs/changelog>
- exact IDs、1,048,576 input limit、65,536 output limit、采样参数迁移与 prefill 约束：
  <https://ai.google.dev/gemini-api/docs/latest-model>
- 两个模型的 exact model page 与能力证据：
  <https://ai.google.dev/gemini-api/docs/models/gemini-3.6-flash>、
  <https://ai.google.dev/gemini-api/docs/models/gemini-3.5-flash-lite>
- 当前 lifecycle/shutdown 状态：
  <https://ai.google.dev/gemini-api/docs/deprecations>
- Gemini Developer API 定价（paid Standard tier）：
  <https://ai.google.dev/gemini-api/docs/pricing>

证据只证明 Gemini Developer API，不证明 Vertex AI availability。

## 目标

- 在 Gemini Developer API chat catalog 中准确公开两个新 GA exact model ID。
- 对两个模型公开可核验的 context、output、能力和价格事实。
- 让 supported params、请求校验和最终请求体遵循同一个新模型请求契约。
- 在网络前拒绝已废弃采样参数和 model-turn prefill，禁止静默删除或透传。
- 对当前公开 chat catalog 的每个 ID 记录 exact 官方 lifecycle disposition。
- 提供显式 opt-in、凭证安全、结果可审计的 Developer API live smoke。

## 非目标

- 不实现或迁移到 Interactions API、Live API、embedding、image、video 或 managed-agent
  协议。
- 不把 Gemini Developer API 的模型、价格或 availability 推断到 Vertex AI。
- 不实现 GH1112 的共享 Google catalog ownership、Vertex overlay 或认证收敛。
- 不实现 GH1111 的完整 `ToolUse` / `ToolResult` 回路。
- 不实现 GH1113 的 pricing authority、unknown-cost 或 spend/budget 语义收敛。
- 不把 Gemini Developer API paid Standard 的价格推断为 Batch、Flex 或 Priority tier
  价格，也不在本 issue 增加这些 tier 的计费契约。
- 不自动联网刷新目录，不在正常构建、测试或运行时隐式执行 live smoke。

## Behavior Invariants

1. **B-001** Gemini Developer API chat catalog 必须以区分大小写的 exact model ID
   公开 `gemini-3.6-flash` 与 `gemini-3.5-flash-lite`；前后缀、大小写变体或相似名称
   不得命中这两个模型。
2. **B-002** `gemini-3.6-flash` 必须公开 1,048,576 input token limit、65,536 output
   token limit、Gemini Developer API paid Standard tier 每百万 input tokens 1.50 USD、
   每百万 output tokens 7.50 USD；
   `gemini-3.5-flash-lite` 必须公开相同 token limits、0.30 USD input 与 2.50 USD
   output。价格单位必须明确为每百万 tokens，不能与 per-token 值混用；Batch、Flex、
   Priority 或其他 tier 不得复用这些值并宣称已由本 invariant 验证。
3. **B-003** 一个 Developer API chat model 只有在 exact ID、当前 lifecycle 和通用
   chat 入口均有 Google 官方正证据时才可被公开；retired、shutdown、unverified、
   仅属于其他产品或只有近似名称的条目必须 fail closed 为不公开。
4. **B-004** 每个现有公开 chat model 必须有独立 disposition：
   `available_exact`、`retired`、`shutdown`、`unverified` 或 `other_product`。
   缺失、冲突或过期到已越过 shutdown date 的证据不得降级为“继续沿用旧目录”。
5. **B-005** 对 `gemini-3.6-flash`、`gemini-3.5-flash-lite` 以及显式声明采用同一后续
   契约的模型，supported params 不得包含 `temperature`、`top_p`、`top_k`；
   请求省略字段或显式 JSON `null` 经现有 `Option` DTO 反序列化后均视为 absent，允许
   继续且最终 upstream body 必须省略字段；任何 non-null 值（包括看似默认的数值）
   必须在网络前返回稳定的 OpenAI-compatible invalid-request 错误，不得静默删除、
   忽略或透传。
6. **B-006** 对 B-005 模型，最后一个非空 conversation turn 为 `model`/assistant
   turn 时必须在网络前拒绝；尾部空内容不得改变“最后一个非空 turn”的判定，
   user/tool turn 结尾不得被本 issue 新增的 prefill gate 拒绝。既有 Gemini
   `ToolUse`/`ToolResult` 序列化与完整 tool-loop callability 仍归 GH1111，不是本
   invariant 的通过条件，也不是 GH1108 implementation dependency。
7. **B-007** provider 公布的 supported params、preflight validation 和最终 upstream
   request body 必须一致；任何一层不能重新加入已拒绝字段，也不能绕过 prefill gate。
8. **B-008** 新模型的 pricing/cost lookup 必须返回 B-002 的确定值并保持单位一致；
   这些值只代表 Gemini Developer API paid Standard tier；Batch、Flex、Priority 与
   其他 unknown model/tier 的全局 pricing 行为不得由本 issue 改写、复用或伪装为
   已知零成本。
9. **B-009** catalog 列表必须稳定排序、无重复；同一不可变证据输入在重复或并发读取时
   返回相同 ID、metadata、价格和请求契约。
10. **B-010** live smoke 默认关闭，只能由文档声明的单一显式 opt-in 环境变量开启；
    未提供 opt-in 或凭证时不得联网，普通单元测试、全量测试和应用启动不得隐式触发。
11. **B-011** opt-in live smoke 必须分别记录静态目录、官方 list-models/get-model
    exact 结果与最小调用结果，并把失败分类为闭集
    `{auth, quota, not_found, protocol, network}`；任一必需步骤未执行不得报告整体通过。
12. **B-012** live smoke、错误、Debug/Display、命令回显和持久化 artifact 均不得包含
    API key 或其他 credential；redaction 负例必须使用 sentinel 凭证证明泄漏为零。
13. **B-013** Developer API 证据不得扩大 Vertex AI availability、endpoint、region、
    auth 或 pricing 声明；任何 Vertex 结果只能作为独立信息记录，不能满足本 spec 的
    Developer API 正证据，也不能由本 spec 改变 Vertex 行为。
14. **B-014** live smoke 被取消、中断或部分完成时必须保留已完成步骤与终止原因，
    整体状态为 incomplete/failed；重试不得复用旧凭证输出或把先前部分成功冒充当前
    完整通过。
15. **B-015** 除明确列出的新模型、evidence disposition 和新请求契约外，已有仍受支持
    模型的 exact ID、能力、合法参数、认证、endpoint 与响应转换保持兼容；不因刷新
    意外删除或改变无关模型。
16. **B-016** catalog refresh 必须保留 evidence reviewed-at 与官方 source URL；官方
    页面互相冲突、来源不可访问或模型只存在于非官方二手资料时按 B-003 fail closed，
    不能以 live smoke 单次成功替代 lifecycle/source 记录。
17. **B-017** 两个新模型公开的 `ModelInfo.capabilities` 必须恰为闭合集
    `{ChatCompletion, ChatCompletionStream, ToolCalling, FunctionCalling}`；model feature
    flags 必须恰为闭合集
    `{MultimodalSupport, ToolCalling, FunctionCalling, StreamingSupport, ContextCaching,
    SystemInstructions, JsonMode, SearchGrounding, VideoUnderstanding,
    AudioUnderstanding}`。任何集合外能力均 fail closed 为不广告，尤其包括
    CodeExecution、BatchProcessing、Realtime API/streaming、Computer Use、audio/image
    generation、Live 与 Interactions；`AudioUnderstanding`/`VideoUnderstanding` 不能被
    解释为 generation 能力。

## 验收标准

- [ ] 两个新 exact model ID 的 Developer API metadata、limits、能力闭合集与 paid
      Standard 价格符合 B-001/B-002/B-017，并有 registry 与 cost 行为测试；Batch/Flex/
      Priority 不在该价格断言范围内。
- [ ] 当前 Developer chat catalog 的每个公开 ID 都有 B-004 disposition；shutdown、
      unverified 与 other-product fixture 不被公开。
- [ ] 新契约模型的 supported params 与最终请求体均不含三项废弃采样参数；省略或显式
      JSON `null` 视为 absent，任何 non-null 输入在网络前得到稳定错误。
- [ ] model-turn prefill 的空/非空、user/tool 结尾不被新增 gate 拒绝，以及
      direct-client/provider 入口均由无网络 fixture 覆盖；完整 tool-loop callability
      明确不作为 GH1108 acceptance。
- [ ] catalog 重复/并发读取稳定排序且 metadata/price/contract 一致。
- [ ] opt-in live smoke 对 list/get/minimal-call 三层结果产生 typed artifact，失败分类
      闭合，取消/中断不产生伪成功。
- [ ] sentinel 凭证在 stdout、stderr、错误、Debug/Display、日志和 artifact 中均无
      明文命中。
- [ ] Vertex AI、Interactions API、GH1111 tool loop 与 GH1113 pricing authority
      acceptance 均未被本实现顺带修改或宣称完成。
- [ ] 新增关键 catalog/request-validation 分支覆盖 100%，新增代码总体 line coverage
      至少 80%。
- [ ] `cargo fmt --check`、`cargo check`、strict Clippy、`cargo test` 与 SpecRail gates
      全部通过。

## 边界检查

| 边界类别 | 判定 |
| --- | --- |
| Empty / missing input | covered: B-006、B-010、B-011。空尾 turn、缺 opt-in/credential、缺 smoke step 都有确定结果。 |
| Error and failure paths | covered: B-003、B-005、B-006、B-011、B-016。未知证据、非法参数、prefill 和 live failures 均 fail closed。 |
| Authorization / permission | covered: B-010、B-012、B-013。Developer credential 只用于 opt-in Developer 请求，不扩大 Vertex 权限。 |
| Concurrency / race / ordering | covered: B-009。目录与契约为稳定不可变快照。 |
| Retry / repetition / idempotency | covered: B-009、B-014。重复目录读取幂等，smoke retry 不复用部分成功。 |
| Illegal state transitions | covered: B-003、B-004、B-011。无证据/部分 smoke 不能变为 advertised/passed。 |
| Compatibility / migration | covered: B-005、B-007、B-015、B-017。明确行为收紧与新模型能力闭合集，其余模型与入口保持兼容。 |
| Degradation / fallback | covered: B-003、B-005、B-016、B-017。旧目录、silent drop、二手资料、近似 ID 和未兑现能力均不能冒充成功。 |
| Evidence and audit integrity | covered: B-004、B-011、B-012、B-016、B-017。每个公开 ID、能力与 smoke 结论都绑定可审计证据。 |
| Cancellation / interruption / partial completion | covered: B-014。中断 smoke 保留部分事实但整体不通过。 |

## 边界情况

- `gemini-3.6-flash-preview`、`Gemini-3.6-Flash` 与
  `foo-gemini-3.6-flash` 均不能命中 stable exact ID。
- 省略废弃采样字段或显式提供 JSON `null` 在现有 `Option` DTO 中均视为 absent，最终
  upstream body 省略该字段；默认或非默认的任何 non-null 数值均按 B-005 拒绝，
  “值看起来无害”不能恢复支持。
- 多个尾部空 turn 之后仍以最近的非空 `model` turn 判定 prefill。
- list-models 出现但 lifecycle 页面未提供通用 chat 正证据时保持不公开，并记录冲突。
- live minimal call 成功但静态价格/limits 不匹配时 smoke 整体失败，不能用连通性覆盖
  metadata 漂移。

## 发布说明

这是 Developer API catalog refresh 与请求行为收紧。发布说明必须列出新增的两个 GA
exact IDs、价格/limits、三项采样参数拒绝和 model-turn prefill 拒绝，并列出停止公开的
旧 ID 及 disposition。必须明确 Vertex AI 未随本变更更新，live smoke 仍为显式 opt-in，
且回滚 binary 前应先移除只被新版本识别的模型配置。
