# Product Spec

## Linked Issue

GH-1107 / #1107

## 用户问题

`litellm-rs` 的 `/v1/responses` 可以处理普通消息、文本、reasoning 和一部分
function-call 输出，但 Codex 的真实会话会在连续回合中发送 call item、call output、
custom/freeform tool、tool-search 与 compaction 等结构。当前入口只接受
`message`，并在 provider 选择前把请求压成 Chat Completions，因此“文本能返回”
不能证明 Codex agent loop 可用。

用户需要的不是另一个统一控制层，而是让现有 gateway 成为可配置、可审计、明确
fail-closed 的 Codex Responses 兼容端点：Codex 继续拥有工具执行权，litellm-rs
负责协议保持、provider 转换、路由、计费和错误语义。

## 目标

- Codex CLI / App 能通过现有 `POST /v1/responses` 使用 litellm-rs 已接入的模型。
- 首批支持消息、function tool loop 与 custom/freeform tool loop 的非流式和流式闭环。
- 对 provider、model 或协议项不支持的组合，在任何上游调用前给出稳定的 4xx 错误。
- 复用现有 router、provider、预算、限流、guardrail、callback 与 lifecycle。
- 用无密钥 conformance fixtures 证明 Anthropic、Gemini、OpenAI-compatible 三类
  adapter 的可用性。
- 提供只读配置示例和 smoke test，不自动修改用户 Codex 配置。

## 非目标

- 新建 Edge、control plane、daemon、GUI、模型市场或另一个 provider registry。
- 整合 Argus、VibeGuard、Remem、Harness 等独立项目。
- 在 gateway 内执行 shell、MCP、web search、computer-use 或其他 Codex 工具。
- 实现 ChatGPT account pool、OAuth 池、配额切换或模型自动发现。
- 自动写入、备份或恢复 `~/.codex/config.toml`。
- 声称全部 provider 支持全部 Codex 扩展。
- 在本规格 PR 中实现生产代码或授予 `ready_to_implement`。

## 支持等级

### Tier 1：完成本 issue 的最低兼容面

- 输入与上下文：`message`、`function_call`、`function_call_output`、
  `custom_tool_call`、`custom_tool_call_output`。
- 工具定义：`function` 与 `custom` freeform tool。
- 输出：message、reasoning、function/custom tool call。
- 传输：同步 JSON 与 Responses SSE。
- Provider 证明：Anthropic、Gemini、OpenAI-compatible。

### Tier 2：识别但不得伪装支持

- `namespace`、`tool_search`、`tool_search_call/output`、
  `additional_tools`。
- `local_shell_call`。
- `compaction`、`compaction_trigger`、`context_compaction`。
- hosted web search、MCP、computer-use、image generation 等 provider-native 工具。

Tier 2 可以在同一 issue 的后续 tranche 中实现；在实现前必须返回明确的
`unsupported_codex_feature`，不得删除、转为空消息或拼成普通文本继续执行。

## Behavior Invariants

1. **B-001** 当请求仅包含现有 `message`、文本、图片、reasoning 配置和 function
   tool 时，启用 Codex 兼容性后不得改变当前成功响应、鉴权、模型限制、计费或
   lifecycle 语义。
2. **B-002** Tier 1 输入项必须保留原始类型、顺序、`id`、`call_id`、
   `name`、`namespace` 和 payload；缺失、为空和显式 `null` 按各字段契约
   区分，不得在解析时静默补成另一个值。
3. **B-003** 每个 `function_call_output` 或 `custom_tool_call_output` 必须
   唯一关联同一会话上下文中的已知 `call_id`；未知、缺失、重复消费或 call
   类型不匹配均在 provider 调用前被拒绝。
4. **B-004** `custom` freeform tool 转换必须可逆：provider 返回的调用必须恢复
   原始 tool kind、名称、namespace 与输入字符串；不得把 custom call 暴露给
   Codex 为不同的 function call。
5. **B-005** 当一次请求含多个并行 call 时，call/output 按 `call_id` 关联，
   output item 按模型产生的稳定顺序返回；并发完成顺序不得造成串线或覆盖。
6. **B-006** Tier 2 或未知 item/tool 必须在上游调用前返回 4xx
   `unsupported_codex_feature`，并指出 feature、model 和 provider context：若 provider
   已选择则给出 selected provider；若请求在 provider 选择前被 wire gate 拒绝，则明确
   返回 `provider=unselected`，不得为补齐错误文本而触发 provider 选择。不得通过
   `serde(other)`、空消息、warning 或普通文本 fallback 表示成功。
7. **B-007** 只有同时满足请求所需 transport、streaming 与 tool capability 的
   provider/model 才可执行；能力声明缺失或不一致时按不支持处理，不尝试未声明
   的降级路径。
8. **B-008** provider 已选择但 capability preflight 失败时，不得发送上游 HTTP
   请求、预留不可回收预算、记录成功 callback 或产生成功 cache entry。
9. **B-009** streaming 对每个 output item 产生合法的 added → delta* → done
   序列；序号单调且引用已创建 item；一个响应恰好产生一个
   `response.completed` 或 `response.failed`，两者互斥。
10. **B-010** 同一 provider 结果的 streaming 聚合值与非 streaming 响应在 item
    类型、顺序、status、`call_id`、tool name/input 和最终文本上等价。
11. **B-011** client disconnect、显式取消、idle timeout、provider timeout 或
    provider error 不得产生伪 `completed`；已经发送的失败/中断证据不得被后续
    retry 改写或删除。
12. **B-012** retry/fallback 不得复用已部分发送的 stream、重复 tool call、
    重复结算或重复 terminal event；stream 开始后的 retry 规则沿用现有 fail-closed
    约束。
13. **B-013** `previous_response_id` 合并上下文时必须保留 Tier 1 call 与 call
    output，不得像当前实现一样只恢复 assistant 文本；owner 不匹配仍返回 not
    found，不泄露响应是否存在。
14. **B-014** `store=false` 不持久化 Codex items；`store=true` 或默认存储必须
    保留后续回合所需的 Tier 1 item，并继续受现有 owner、TTL、容量和取消规则约束。
15. **B-015** 空 input、空 Tier 1 payload、非法 JSON、非法角色或不完整图片继续
    返回确定性 4xx；“没有模型输出 item”不得自动伪造一个成功的空 assistant
    message。
16. **B-016** Codex 兼容请求不能绕过现有 API-key model allowlist、token limit、
    guardrail、rate limit、budget、cache isolation 或 callback 生命周期。
17. **B-017** 对外错误不得包含 API key、Authorization header、provider 原始响应
    body 或用户工具输出全文；错误 code 稳定，message 只包含诊断所需的
    feature/provider/model。
18. **B-018** conformance fixture 必须固定 Codex 协议来源 commit，并同时覆盖
    正例和 schema 合法但业务不支持的负例；只验证 serde 成功不算完成。
19. **B-019** 文档只提供 `wire_api="responses"` 的手动配置与环境变量方式；
    不写入用户文件、不要求硬编码密钥，并提供恢复到原 provider 的步骤。
20. **B-020** 本功能不得建立第二套 router/provider selection、工具执行器或后台
    控制服务；所有执行仍通过当前 canonical runtime，功能关闭或回滚后通用
    Responses 与 Chat Completions 保持可用。

## 验收标准

- [ ] 无密钥 fixture 完成“用户消息 → tool call → Codex 回送 output → 最终文本”的
      两回合闭环，Anthropic、Gemini、OpenAI-compatible 三类 adapter 均有同步与
      streaming 证明。
- [ ] function 与 custom/freeform 两种 tool loop 都验证 call correlation、并行
      calls 和 reversible mapping。
- [ ] 每个 Tier 2 类型至少有一个 schema 合法的负例，证明在上游请求计数仍为零时
      返回 `unsupported_codex_feature`。
- [ ] event-sequence validator 覆盖正常、空 delta、并行 calls、provider error、
      timeout、disconnect 与取消。
- [ ] `previous_response_id` fixture 证明 call/output 进入下一回合且跨 owner
      不可见；`store=false` fixture 证明不持久化。
- [ ] 现有 Responses、Chat Completions、reasoning、budget、callback 和 lifecycle
      regression suite 保持通过。
- [ ] 新增关键转换/验证路径分支覆盖 100%，新增代码总体 line coverage 至少 80%。
- [ ] 文档 smoke test 能在不写用户配置、不提交密钥的前提下完成。
- [ ] 全量 `cargo fmt --check`、`cargo check`、strict Clippy 与 `cargo test`
      通过。

## 边界检查

| 边界类别 | 判定 |
| --- | --- |
| Empty / missing input | covered: B-002、B-015。缺失、空、null 与无输出分别定义，不能补成成功。 |
| Error and failure paths | covered: B-006、B-008、B-011、B-017。unsupported、timeout、disconnect、provider error 都有显式结果。 |
| Authorization / permission | covered: B-013、B-014、B-016。owner isolation 与既有 API-key 权限不得绕过。 |
| Concurrency / race / ordering | covered: B-005、B-009、B-012。并行 call、SSE 顺序与 retry 互斥被固定。 |
| Retry / repetition / idempotency | covered: B-003、B-012。重复 output 与部分 stream retry 都不能重复执行或结算。 |
| Illegal state transitions | covered: B-009、B-011。failed/cancelled 不能转 completed，terminal 恰好一次。 |
| Compatibility / migration | covered: B-001、B-010、B-019、B-020。现有请求、双传输和配置恢复均有契约。 |
| Degradation / fallback | covered: B-006、B-007、B-008。未声明能力不得看起来成功。 |
| Evidence and audit integrity | covered: B-018。来源 commit、正负例和 upstream request count 都是必要证据。 |
| Cancellation / interruption / partial completion | covered: B-011、B-012、B-014。取消、断连和部分发送不会产生伪完成或重复副作用。 |

## 边界情况

- 一个请求同时含 function 与 custom calls：分别保持类型，不能因同名合并。
- 两个 call 使用相同 `call_id`：整个请求失败，不能“最后一个覆盖前一个”。
- output 只引用 `previous_response_id` 中的 call：在 owner 匹配且 call 可见时合法。
- provider 支持 tool calling 但不支持 streaming tool delta：stream 请求必须失败，
  不能先发文本再中止。
- provider 返回 tool call 后又返回文本：保持 provider 顺序并满足统一 event sequence。
- 未识别的未来 Codex item：保留 type 供错误诊断，但不得将完整敏感 payload写日志。
- background + streaming：继续沿用当前明确拒绝规则，不在本需求中放宽。

## 发布说明

这是 opt-in 使用场景的协议扩展，不改变默认 endpoint 或现有 provider 配置。发布说明
需列出 Tier 1 支持矩阵、Tier 2 fail-closed 列表、Codex 来源 commit、配置示例和已知
限制。若兼容性回归，可回滚该功能提交；不得通过重新启用 silent fallback 回滚。
